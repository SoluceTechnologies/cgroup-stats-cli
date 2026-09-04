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
    let sample_window = Duration::try_from_secs_f64(interval)
        .ok()
        .filter(|_| interval > 0.0)
        .ok_or_else(|| {
            format!("interval must be a positive, finite number of seconds, got {interval}")
        })?;

    if !hierarchies::is_cgroup2_unified_mode() {
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
    let cgroup = Cgroup::load(hierarchies::auto(), &rel);

    let cpu_before = sel.cpu.then(|| metrics::read_cpu_usage(&dir));
    let io_before = sel.io.then(|| metrics::read_io(&dir));

    let elapsed = if sel.needs_sampling() {
        let started = Instant::now();
        std::thread::sleep(sample_window);
        started.elapsed().as_secs_f64()
    } else {
        0.0
    };

    let cpu = match cpu_before {
        None => None,
        Some(Err(err)) => Some(Err(err)),
        Some(Ok(before)) => Some(
            metrics::read_cpu_usage(&dir)
                .and_then(|after| metrics::collect_cpu(&dir, before, after, elapsed)),
        ),
    };

    let io = match io_before {
        None => None,
        Some(Err(err)) => Some(Err(err)),
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
        memory: sel.mem.then(|| metrics::collect_memory(&cgroup, &dir)),
        pids: sel.pids.then(|| metrics::collect_pids(&cgroup, &dir)),
        io,
    })
}
