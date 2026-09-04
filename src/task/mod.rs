pub mod format;
pub mod metrics;

use crate::cli::Selection;
use crate::utils::cgfile::normalize_path;
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
    // from_secs_f64 panics on negative, NaN, infinite AND overflowing values.
    // try_from_secs_f64 rejects that set but accepts zero, so `filter` adds the
    // "positive" half back. `collect` is public and cannot assume the CLI
    // validated first.
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

    let io = match io_before {
        None => None,
        Some(Err(e)) => Some(Err(e)),
        Some(Ok(before)) => Some(metrics::read_io(&dir).map(|after| {
            metrics::collect_io(
                &crate::utils::cgfile::sys_dev_block(),
                &before,
                &after,
                elapsed,
            )
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
