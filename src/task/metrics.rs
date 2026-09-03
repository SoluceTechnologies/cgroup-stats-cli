/// Read one value out of a flat-keyed cgroup file (`key value` per line),
/// the format used by `cpu.stat`, `memory.stat` and friends.
pub fn parse_flat_key(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        match (it.next(), it.next()) {
            (Some(k), Some(v)) if k == key => v.parse().ok(),
            _ => None,
        }
    })
}

/// Parse `cpu.max`, which holds `<quota> <period>` where quota may be the
/// literal `max`. Returns `None` for unlimited or unparseable content.
pub fn parse_cpu_max(text: &str) -> Option<(u64, u64)> {
    let mut it = text.split_whitespace();
    let quota = it.next()?;
    let period = it.next()?.parse().ok()?;
    if period == 0 {
        return None;
    }
    Some((quota.parse().ok()?, period))
}

/// One device's counters from `io.stat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIo {
    pub device: String,
    pub rbytes: u64,
    pub wbytes: u64,
}

/// Parse `io.stat`: `<major>:<minor> key=value key=value ...`.
///
/// Unlike the parser in cgroups-rs this does not require exactly seven fields,
/// so lines carrying iocost keys survive, and it keeps the device as text
/// rather than parsing the minor into an `i16` that overflows past 32767.
pub fn parse_io_stat(text: &str) -> Vec<RawIo> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let device = it.next()?;
            if !device.contains(':') {
                return None;
            }
            let mut rbytes = None;
            let mut wbytes = None;
            for field in it {
                match field.split_once('=') {
                    Some(("rbytes", v)) => rbytes = v.parse().ok(),
                    Some(("wbytes", v)) => wbytes = v.parse().ok(),
                    _ => {}
                }
            }
            Some(RawIo {
                device: device.to_string(),
                rbytes: rbytes?,
                wbytes: wbytes?,
            })
        })
        .collect()
}

/// Per-second rate between two counter samples.
///
/// Saturating: a cgroup recreated between samples resets its counters, which
/// would otherwise underflow. Zero elapsed time yields zero, not infinity.
pub fn rate(before: u64, after: u64, elapsed_secs: f64) -> f64 {
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 {
        return 0.0;
    }
    after.saturating_sub(before) as f64 / elapsed_secs
}

use cgroups_rs::fs::memory::MemController;
use cgroups_rs::fs::pid::PidController;
use cgroups_rs::fs::{Cgroup, MaxValue};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Cpu {
    pub used_cores: f64,
    pub max_cores: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Memory {
    pub current: u64,
    pub max: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pids {
    pub current: u64,
    pub max: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct IoDevice {
    pub device: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Io {
    pub devices: Vec<IoDevice>,
}

/// Fail early when a metric's backing files are absent.
///
/// This exists because cgroups-rs cannot be asked whether a read succeeded:
/// `memory_stat()` returns `usage_in_bytes == 0` for a missing `memory.current`,
/// and `get_mem()` maps a failed read of `memory.max` onto `MaxValue::Max`,
/// which is identical to a genuine "unlimited". Without this precheck the root
/// cgroup reports `0 / unlimited` instead of being reported unavailable.
pub fn require(dir: &Path, files: &[&str]) -> Result<(), String> {
    for f in files {
        if !dir.join(f).exists() {
            return Err(format!("{f} not present"));
        }
    }
    Ok(())
}

/// `MaxValue::Max` and any negative value both mean "no limit".
pub fn max_value_to_option(v: Option<MaxValue>) -> Option<u64> {
    match v {
        None | Some(MaxValue::Max) => None,
        Some(MaxValue::Value(n)) if n < 0 => None,
        Some(MaxValue::Value(n)) => Some(n as u64),
    }
}

fn read(dir: &Path, file: &str) -> Result<String, String> {
    std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))
}

pub fn collect_memory(cg: &Cgroup, dir: &Path) -> Result<Memory, String> {
    require(dir, &["memory.current", "memory.max"])?;
    let c: &MemController = cg
        .controller_of()
        .ok_or_else(|| "memory controller unavailable".to_string())?;
    let max = c.get_mem().map_err(|e| e.to_string())?.max;
    Ok(Memory {
        current: c.memory_stat().usage_in_bytes,
        max: max_value_to_option(max),
    })
}

pub fn collect_pids(cg: &Cgroup, dir: &Path) -> Result<Pids, String> {
    require(dir, &["pids.current", "pids.max"])?;
    let c: &PidController = cg
        .controller_of()
        .ok_or_else(|| "pids controller unavailable".to_string())?;
    Ok(Pids {
        current: c.get_pid_current().map_err(|e| e.to_string())?,
        max: max_value_to_option(Some(c.get_pid_max().map_err(|e| e.to_string())?)),
    })
}

/// One `usage_usec` sample. Read directly rather than through
/// `CpuController::cpu()`, which returns an empty string on a failed read.
pub fn read_cpu_usage(dir: &Path) -> Result<u64, String> {
    let text = read(dir, "cpu.stat")?;
    parse_flat_key(&text, "usage_usec").ok_or_else(|| "cpu.stat has no usage_usec".to_string())
}

/// `cpu.max` is absent on the root cgroup, which is not an error: usage is
/// still meaningful, the limit is simply unknown and reported as unlimited.
pub fn collect_cpu(dir: &Path, before: u64, after: u64, elapsed: f64) -> Result<Cpu, String> {
    require(dir, &["cpu.stat"])?;
    let max_cores = read(dir, "cpu.max")
        .ok()
        .and_then(|t| parse_cpu_max(&t))
        .map(|(q, p)| q as f64 / p as f64);
    Ok(Cpu {
        // usage_usec is microseconds of CPU time; dividing by elapsed seconds
        // and 1e6 gives cores in use.
        used_cores: rate(before, after, elapsed) / 1_000_000.0,
        max_cores,
    })
}

pub fn read_io(dir: &Path) -> Result<Vec<RawIo>, String> {
    require(dir, &["io.stat"])?;
    Ok(parse_io_stat(&read(dir, "io.stat")?))
}

/// Resolve `major:minor` to a kernel device name via `/sys/dev/block`.
/// Falls back to the raw `major:minor`; naming never fails the metric.
pub fn device_name(sys_dev_block: &Path, dev: &str) -> String {
    std::fs::read_link(sys_dev_block.join(dev))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| dev.to_string())
}

pub fn collect_io(sys_dev_block: &Path, before: &[RawIo], after: &[RawIo], elapsed: f64) -> Io {
    let devices = after
        .iter()
        .map(|a| {
            // A device absent from the first sample started at zero.
            let b = before.iter().find(|b| b.device == a.device);
            let (br, bw) = b.map_or((0, 0), |b| (b.rbytes, b.wbytes));
            IoDevice {
                device: device_name(sys_dev_block, &a.device),
                read_bytes_per_sec: rate(br, a.rbytes, elapsed),
                write_bytes_per_sec: rate(bw, a.wbytes, elapsed),
            }
        })
        .collect();
    Io { devices }
}

/// The conventional location for block device symlinks, injected so tests can
/// point at a directory that does not contain them.
pub fn sys_dev_block() -> PathBuf {
    PathBuf::from("/sys/dev/block")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real content from /sys/fs/cgroup/cpu.stat on a 6.x kernel.
    const CPU_STAT: &str = "\
usage_usec 38354643000
user_usec 29454747000
system_usec 8899896000
nice_usec 47780000
core_sched.force_idle_usec 0
";

    #[test]
    fn reads_a_key_that_is_first() {
        assert_eq!(parse_flat_key(CPU_STAT, "usage_usec"), Some(38354643000));
    }

    #[test]
    fn reads_a_key_that_is_not_first() {
        assert_eq!(parse_flat_key(CPU_STAT, "system_usec"), Some(8899896000));
        assert_eq!(parse_flat_key(CPU_STAT, "nice_usec"), Some(47780000));
    }

    #[test]
    fn missing_key_is_none_not_a_panic() {
        assert_eq!(parse_flat_key(CPU_STAT, "nope"), None);
        assert_eq!(parse_flat_key("", "usage_usec"), None);
    }

    #[test]
    fn key_match_is_exact_not_a_prefix() {
        // `usage_usec` must not be found by searching for `usage`, and
        // `core_sched.force_idle_usec` must not satisfy a search for `usec`.
        assert_eq!(parse_flat_key(CPU_STAT, "usage"), None);
        assert_eq!(parse_flat_key(CPU_STAT, "usec"), None);
    }

    #[test]
    fn cpu_max_with_a_quota_gives_quota_and_period() {
        assert_eq!(parse_cpu_max("200000 100000\n"), Some((200000, 100000)));
    }

    #[test]
    fn cpu_max_unlimited_is_none() {
        // The literal value read from /sys/fs/cgroup/<leaf>/cpu.max on this host.
        assert_eq!(parse_cpu_max("max 100000\n"), None);
    }

    #[test]
    fn cpu_max_malformed_is_none() {
        assert_eq!(parse_cpu_max(""), None);
        assert_eq!(parse_cpu_max("200000"), None);
        assert_eq!(parse_cpu_max("abc def"), None);
    }

    #[test]
    fn cpu_max_with_a_zero_period_is_none() {
        // A parseable but nonsensical period would hand the caller a
        // divide-by-zero. Reject it here rather than trusting every caller.
        assert_eq!(parse_cpu_max("200000 0\n"), None);
    }

    #[test]
    fn io_stat_parses_multiple_devices() {
        let s = "\
7:55 rbytes=14336 wbytes=0 rios=11 wios=0 dbytes=0 dios=0
259:0 rbytes=966656 wbytes=512 rios=17 wios=3 dbytes=0 dios=0
";
        let v = parse_io_stat(s);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].device, "7:55");
        assert_eq!(v[0].rbytes, 14336);
        assert_eq!(v[1].device, "259:0");
        assert_eq!(v[1].wbytes, 512);
    }

    #[test]
    fn io_stat_keeps_lines_carrying_extra_iocost_keys() {
        // The case cgroups-rs drops: its parser filters to exactly 7 fields,
        // so an iocost-enabled kernel yields an empty device list there.
        let s = "8:0 rbytes=180224 wbytes=0 rios=3 wios=0 dbytes=0 dios=0 \
cost.usage=123 cost.wait=0 cost.indebt=0 cost.indelay=0\n";
        let v = parse_io_stat(s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rbytes, 180224);
    }

    #[test]
    fn io_stat_handles_a_large_minor() {
        // cgroups-rs parses the minor as i16 and panics above 32767.
        let s = "253:1048575 rbytes=1 wbytes=2 rios=0 wios=0 dbytes=0 dios=0\n";
        let v = parse_io_stat(s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].device, "253:1048575");
    }

    #[test]
    fn io_stat_empty_is_an_empty_list_not_an_error() {
        assert!(parse_io_stat("").is_empty());
        assert!(parse_io_stat("\n\n").is_empty());
    }

    #[test]
    fn io_stat_skips_junk_lines_without_panicking() {
        let s = "garbage\n8:0 rbytes=5 wbytes=6\nalso junk\n";
        let v = parse_io_stat(s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rbytes, 5);
    }

    #[test]
    fn rate_divides_the_delta_by_elapsed_time() {
        assert_eq!(rate(0, 1000, 2.0), 500.0);
        assert_eq!(rate(1_000_000, 3_000_000, 1.0), 2_000_000.0);
    }

    #[test]
    fn rate_saturates_when_the_counter_resets() {
        // A cgroup recreated between samples resets its counters. That must be
        // a zero, not an underflow panic or a wrapped-around huge number.
        assert_eq!(rate(5000, 10, 1.0), 0.0);
    }

    #[test]
    fn rate_of_zero_elapsed_is_zero_not_infinity() {
        assert_eq!(rate(0, 100, 0.0), 0.0);
    }

    #[test]
    fn rate_of_non_finite_elapsed_is_zero() {
        // `elapsed <= 0.0` is false for NaN, so the guard must be written as
        // the negation of the positive case to catch it.
        assert_eq!(rate(0, 100, f64::NAN), 0.0);
        assert_eq!(rate(0, 100, -1.0), 0.0);
    }

    use std::fs;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cgstats-test-{name}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn require_passes_when_every_file_is_present() {
        let d = tmpdir("require-ok");
        fs::write(d.join("a"), "1").unwrap();
        fs::write(d.join("b"), "2").unwrap();
        assert!(require(&d, &["a", "b"]).is_ok());
    }

    #[test]
    fn require_names_the_missing_file() {
        let d = tmpdir("require-missing");
        fs::write(d.join("a"), "1").unwrap();
        let err = require(&d, &["a", "b"]).unwrap_err();
        assert!(
            err.contains("b"),
            "error should name the missing file, got: {err}"
        );
    }

    #[test]
    fn max_value_max_and_negative_are_unlimited() {
        assert_eq!(max_value_to_option(Some(MaxValue::Max)), None);
        assert_eq!(max_value_to_option(Some(MaxValue::Value(-1))), None);
        assert_eq!(max_value_to_option(None), None);
        assert_eq!(max_value_to_option(Some(MaxValue::Value(4096))), Some(4096));
    }

    #[test]
    fn cpu_usage_comes_from_the_stat_file() {
        let d = tmpdir("cpu-usage");
        fs::write(d.join("cpu.stat"), CPU_STAT).unwrap();
        assert_eq!(read_cpu_usage(&d).unwrap(), 38354643000);
    }

    #[test]
    fn cpu_usage_missing_file_is_an_error() {
        let d = tmpdir("cpu-usage-missing");
        assert!(read_cpu_usage(&d).is_err());
    }

    #[test]
    fn cpu_usage_present_but_keyless_is_an_error() {
        let d = tmpdir("cpu-usage-keyless");
        fs::write(d.join("cpu.stat"), "user_usec 5\n").unwrap();
        assert!(read_cpu_usage(&d).is_err());
    }

    #[test]
    fn collect_cpu_computes_cores_from_the_delta() {
        let d = tmpdir("cpu-cores");
        fs::write(d.join("cpu.stat"), CPU_STAT).unwrap();
        fs::write(d.join("cpu.max"), "200000 100000\n").unwrap();
        // Half a core-second of usage over one wall second.
        let c = collect_cpu(&d, 1_000_000, 1_500_000, 1.0).unwrap();
        assert!((c.used_cores - 0.5).abs() < 1e-9, "got {}", c.used_cores);
        assert_eq!(c.max_cores, Some(2.0));
    }

    #[test]
    fn collect_cpu_reports_unlimited_when_cpu_max_is_max() {
        let d = tmpdir("cpu-unlimited");
        fs::write(d.join("cpu.stat"), CPU_STAT).unwrap();
        fs::write(d.join("cpu.max"), "max 100000\n").unwrap();
        assert_eq!(collect_cpu(&d, 0, 0, 1.0).unwrap().max_cores, None);
    }

    #[test]
    fn collect_cpu_without_cpu_max_still_reports_usage() {
        // The root cgroup has cpu.stat but no cpu.max. Usage is still valid.
        let d = tmpdir("cpu-no-max");
        fs::write(d.join("cpu.stat"), CPU_STAT).unwrap();
        let c = collect_cpu(&d, 0, 1_000_000, 1.0).unwrap();
        assert!((c.used_cores - 1.0).abs() < 1e-9);
        assert_eq!(c.max_cores, None);
    }

    #[test]
    fn collect_io_pairs_devices_across_samples() {
        let d = tmpdir("io-pair");
        let before = parse_io_stat("8:0 rbytes=1000 wbytes=2000 rios=0 wios=0 dbytes=0 dios=0\n");
        let after = parse_io_stat("8:0 rbytes=3000 wbytes=2500 rios=0 wios=0 dbytes=0 dios=0\n");
        let io = collect_io(&d, &before, &after, 2.0);
        assert_eq!(io.devices.len(), 1);
        assert_eq!(io.devices[0].read_bytes_per_sec, 1000.0);
        assert_eq!(io.devices[0].write_bytes_per_sec, 250.0);
    }

    #[test]
    fn collect_io_treats_a_device_new_in_the_second_sample_as_starting_at_zero() {
        let d = tmpdir("io-new-dev");
        let before = parse_io_stat("");
        let after = parse_io_stat("8:0 rbytes=500 wbytes=0 rios=0 wios=0 dbytes=0 dios=0\n");
        let io = collect_io(&d, &before, &after, 1.0);
        assert_eq!(io.devices.len(), 1);
        assert_eq!(io.devices[0].read_bytes_per_sec, 500.0);
    }

    #[test]
    fn device_name_falls_back_to_major_minor_when_unresolvable() {
        let d = tmpdir("devname");
        assert_eq!(device_name(&d, "8:0"), "8:0");
    }
}
