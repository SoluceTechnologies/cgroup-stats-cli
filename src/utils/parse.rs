use cgroups_rs::fs::MaxValue;

/// Read one value out of a flat-keyed cgroup file (`key value` per line), the
/// format used by `cpu.stat`, `memory.stat` and friends.
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
/// literal `max`. Returns `None` for unlimited, for unparseable content, and
/// for a zero period, which would otherwise hand the caller a divide-by-zero.
pub fn parse_cpu_max(text: &str) -> Option<(u64, u64)> {
    let mut it = text.split_whitespace();
    let quota = it.next()?;
    let period = it.next()?.parse().ok()?;
    if period == 0 {
        return None;
    }
    Some((quota.parse().ok()?, period))
}

/// One device's counters as they appear in `io.stat`.
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

/// Saturating: a cgroup recreated between samples resets its counters, which
/// would otherwise underflow. The guard is written as `!is_finite()` rather
/// than `<= 0.0` because the latter is false for NaN.
pub fn rate(before: u64, after: u64, elapsed_secs: f64) -> f64 {
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 {
        return 0.0;
    }
    after.saturating_sub(before) as f64 / elapsed_secs
}

pub fn max_value_to_option(v: Option<MaxValue>) -> Option<u64> {
    match v {
        None | Some(MaxValue::Max) => None,
        Some(MaxValue::Value(n)) if n < 0 => None,
        Some(MaxValue::Value(n)) => Some(n as u64),
    }
}
