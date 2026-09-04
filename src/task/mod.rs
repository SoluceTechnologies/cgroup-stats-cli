pub mod format;
pub mod metrics;

use crate::cli::{Selection, normalize_path};
use cgroups_rs::fs::{Cgroup, hierarchies};
use metrics::{Cpu, Io, Memory, Pids};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Stats {
    pub path: String,
    pub cpu: Option<Result<Cpu, String>>,
    pub memory: Option<Result<Memory, String>>,
    pub pids: Option<Result<Pids, String>>,
    pub io: Option<Result<Io, String>>,
}

pub fn collect(path: &str, sel: Selection, interval: f64) -> Result<Stats, Box<dyn Error>> {
    // Duration::from_secs_f64 panics on negative, NaN, infinite AND overflowing
    // values. try_from_secs_f64 rejects that set, but unlike the CLI's
    // positive_secs parser it accepts zero, so `filter` adds back the
    // "positive" half of "positive, finite". Binding the result here means
    // the sleep below cannot panic. `collect` is public, so it must not
    // assume the CLI validated first.
    let sample_window = Duration::try_from_secs_f64(interval)
        .ok()
        .filter(|_| interval > 0.0)
        .ok_or_else(|| {
            format!("interval must be a positive, finite number of seconds, got {interval}")
        })?;

    if !hierarchies::is_cgroup2_unified_mode() {
        // is_cgroup2_unified_mode() statfs's /sys/fs/cgroup and returns false on
        // ANY error, so "this host is v1" and "there is no cgroupfs here" look
        // identical to it. Tell them apart before blaming v1.
        return Err(if Path::new("/sys/fs/cgroup").is_dir() {
            "cgroup v1 not supported (unified/v2 only)"
        } else {
            "no cgroup filesystem mounted at /sys/fs/cgroup"
        }
        .into());
    }

    let root = hierarchies::auto().root();
    let rel = normalize_path(path, &root);
    let dir: PathBuf = root.join(&rel);
    if !dir.is_dir() || !dir.join("cgroup.controllers").exists() {
        return Err(format!("cgroup not found: {}", dir.display()).into());
    }
    let cg = Cgroup::load(hierarchies::auto(), &rel);

    // Take the first sample of every delta metric before sleeping, so both
    // counters span the same window.
    let cpu_before = sel.cpu.then(|| metrics::read_cpu_usage(&dir));
    let io_before = sel.io.then(|| metrics::read_io(&dir));

    let elapsed = if sel.needs_sampling() {
        let t = Instant::now();
        std::thread::sleep(sample_window);
        // Measured, not requested: sleep overshoots under load, and using the
        // requested interval would inflate the reported rates.
        t.elapsed().as_secs_f64()
    } else {
        0.0
    };

    let cpu = match cpu_before {
        None => None,
        Some(Err(e)) => Some(Err(e)),
        Some(Ok(before)) => Some(
            metrics::read_cpu_usage(&dir)
                .and_then(|after| metrics::collect_cpu(&dir, before, after, elapsed)),
        ),
    };

    let io =
        match io_before {
            None => None,
            Some(Err(e)) => Some(Err(e)),
            Some(Ok(before)) => Some(metrics::read_io(&dir).map(|after| {
                metrics::collect_io(&metrics::sys_dev_block(), &before, &after, elapsed)
            })),
        };

    Ok(Stats {
        path: if rel.is_empty() { "/".into() } else { rel },
        cpu,
        memory: sel.mem.then(|| metrics::collect_memory(&cg, &dir)),
        pids: sel.pids.then(|| metrics::collect_pids(&cg, &dir)),
        io,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Selection;
    use std::time::Instant;

    fn v2() -> bool {
        cgroups_rs::fs::hierarchies::is_cgroup2_unified_mode()
    }

    const ALL: Selection = Selection {
        cpu: true,
        mem: true,
        pids: true,
        io: true,
    };
    const MEM: Selection = Selection {
        cpu: false,
        mem: true,
        pids: false,
        io: false,
    };

    #[test]
    fn memory_only_does_not_sleep() {
        if !v2() {
            eprintln!("skipped: host is not cgroup v2");
            return;
        }
        let t = Instant::now();
        collect("", MEM, 5.0).unwrap();
        assert!(
            t.elapsed().as_secs_f64() < 1.0,
            "a memory-only run must skip the sampling sleep, took {:?}",
            t.elapsed()
        );
    }

    #[test]
    fn root_cgroup_reports_memory_unavailable_not_zero() {
        // Regression guard. The root cgroup has no memory.current; without the
        // existence precheck cgroups-rs reports a confident 0 / unlimited.
        if !v2() {
            eprintln!("skipped: host is not cgroup v2");
            return;
        }
        let s = collect("", ALL, 0.05).unwrap();
        assert!(
            matches!(s.memory, Some(Err(_))),
            "expected memory unavailable at the root, got {:?}",
            s.memory
        );
    }

    #[test]
    fn unselected_metrics_are_none() {
        if !v2() {
            eprintln!("skipped: host is not cgroup v2");
            return;
        }
        let s = collect("", MEM, 0.05).unwrap();
        assert!(s.cpu.is_none() && s.pids.is_none() && s.io.is_none());
        assert!(s.memory.is_some());
    }

    #[test]
    fn a_missing_cgroup_is_a_fatal_error() {
        if !v2() {
            eprintln!("skipped: host is not cgroup v2");
            return;
        }
        let e = collect("definitely/not/a/real/cgroup", ALL, 0.05).unwrap_err();
        assert!(e.to_string().contains("cgroup not found"), "got: {e}");
    }

    #[test]
    fn a_non_finite_or_non_positive_interval_is_an_error_not_a_panic() {
        // No v2() guard: the interval check runs before any host inspection,
        // so this test is meaningful on every machine.
        for bad in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            1e20,
            f64::MAX,
        ] {
            assert!(
                collect("", ALL, bad).is_err(),
                "interval {bad} should be rejected, not panic"
            );
        }
    }
}
