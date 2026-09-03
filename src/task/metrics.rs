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
            Some(RawIo { device: device.to_string(), rbytes: rbytes?, wbytes: wbytes? })
        })
        .collect()
}

/// Per-second rate between two counter samples.
///
/// Saturating: a cgroup recreated between samples resets its counters, which
/// would otherwise underflow. Zero elapsed time yields zero, not infinity.
pub fn rate(before: u64, after: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        return 0.0;
    }
    after.saturating_sub(before) as f64 / elapsed_secs
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
}
