use clap::{Parser, ValueEnum};
use std::path::Path;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Human,
    Json,
    Table,
}

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = env!("CARGO_PKG_DESCRIPTION"),
)]
pub struct Args {
    /// cgroup path, absolute or relative to the hierarchy root
    #[arg(short = 'p', long)]
    pub path: String,

    /// Show CPU statistics
    #[arg(short = 'c', long)]
    pub cpu: bool,

    /// Show memory statistics
    #[arg(short = 'm', long)]
    pub mem: bool,

    /// Show PID statistics
    #[arg(short = 'P', long)]
    pub pids: bool,

    /// Show block IO statistics
    #[arg(short = 'b', long)]
    pub io: bool,

    /// Sampling window in seconds for CPU and IO deltas
    #[arg(short = 'i', long, default_value_t = 1.0, value_parser = positive_secs)]
    pub interval: f64,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = Format::Human)]
    pub format: Format,
}

fn positive_secs(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(format!("interval must be a positive number of seconds, got `{s}`"))
    }
}

/// Which metrics to collect. Distinct from `Args` so the collector does not
/// have to re-derive the "no flags means all" rule.
#[derive(Copy, Clone, Debug)]
pub struct Selection {
    pub cpu: bool,
    pub mem: bool,
    pub pids: bool,
    pub io: bool,
}

impl Selection {
    /// CPU and IO are monotonic counters and need two samples around a sleep.
    /// Memory and PIDs do not, so a memory-only run must not sleep at all.
    pub fn needs_sampling(&self) -> bool {
        self.cpu || self.io
    }
}

impl Args {
    pub fn selection(&self) -> Selection {
        if self.cpu || self.mem || self.pids || self.io {
            Selection { cpu: self.cpu, mem: self.mem, pids: self.pids, io: self.io }
        } else {
            Selection { cpu: true, mem: true, pids: true, io: true }
        }
    }
}

/// Strip the hierarchy mount point so both `/sys/fs/cgroup/a/b` and `a/b`
/// resolve to the `a/b` that `Cgroup::load` expects. Uses the root reported by
/// the running hierarchy rather than a hardcoded prefix, so non-standard mount
/// points work unchanged.
pub fn normalize_path(path: &str, root: &Path) -> String {
    let p = Path::new(path);
    let rel = p.strip_prefix(root).unwrap_or(p);
    rel.to_string_lossy().trim_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::path::Path;

    #[test]
    fn clap_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn absolute_and_relative_paths_normalize_the_same() {
        let root = Path::new("/sys/fs/cgroup");
        assert_eq!(normalize_path("/sys/fs/cgroup/a/b", root), "a/b");
        assert_eq!(normalize_path("a/b", root), "a/b");
    }

    #[test]
    fn normalize_handles_trailing_and_leading_slashes() {
        let root = Path::new("/sys/fs/cgroup");
        assert_eq!(normalize_path("/sys/fs/cgroup/a/b/", root), "a/b");
        assert_eq!(normalize_path("/a/b", root), "a/b");
        assert_eq!(normalize_path("/sys/fs/cgroup", root), "");
        assert_eq!(normalize_path("/sys/fs/cgroup/", root), "");
    }

    #[test]
    fn normalize_respects_a_non_standard_mount_point() {
        let root = Path::new("/mnt/cg2");
        assert_eq!(normalize_path("/mnt/cg2/svc.slice", root), "svc.slice");
    }

    #[test]
    fn no_metric_flags_selects_everything() {
        let a = Args::parse_from(["x", "--path", "/sys/fs/cgroup"]);
        let s = a.selection();
        assert!(s.cpu && s.mem && s.pids && s.io);
    }

    #[test]
    fn explicit_flags_select_only_those() {
        let a = Args::parse_from(["x", "--path", "p", "--cpu", "--mem"]);
        let s = a.selection();
        assert!(s.cpu && s.mem);
        assert!(!s.pids && !s.io);
    }

    #[test]
    fn sampling_needed_only_for_delta_metrics() {
        let mem_only = Args::parse_from(["x", "--path", "p", "--mem"]).selection();
        assert!(!mem_only.needs_sampling());

        let with_cpu = Args::parse_from(["x", "--path", "p", "--cpu"]).selection();
        assert!(with_cpu.needs_sampling());

        let with_io = Args::parse_from(["x", "--path", "p", "--io"]).selection();
        assert!(with_io.needs_sampling());

        let pids_only = Args::parse_from(["x", "--path", "p", "--pids"]).selection();
        assert!(!pids_only.needs_sampling());
    }

    #[test]
    fn interval_must_be_positive() {
        assert!(Args::try_parse_from(["x", "--path", "p", "-i", "0"]).is_err());
        assert!(Args::try_parse_from(["x", "--path", "p", "-i", "-1"]).is_err());
        assert!(Args::try_parse_from(["x", "--path", "p", "-i", "0.5"]).is_ok());
    }

    #[test]
    fn normalize_does_not_strip_a_sibling_that_merely_shares_a_prefix() {
        let root = Path::new("/sys/fs/cgroup");
        // These are siblings of the root, not children. Stripping the text
        // prefix would silently turn them into bogus child paths.
        assert_eq!(normalize_path("/sys/fs/cgroup-old/svc.slice", root), "sys/fs/cgroup-old/svc.slice");
        assert_eq!(normalize_path("/sys/fs/cgroup2/foo", root), "sys/fs/cgroup2/foo");
    }

    #[test]
    fn interval_rejects_non_finite_values() {
        for bad in ["nan", "NaN", "inf", "infinity", "-inf"] {
            assert!(
                Args::try_parse_from(["x", "--path", "p", "-i", bad]).is_err(),
                "{bad} should be rejected"
            );
        }
    }
}
