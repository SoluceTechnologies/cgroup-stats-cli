use cgroups_rs::fs::MaxValue;

pub fn parse_flat_key(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next()) {
            (Some(found), Some(value)) if found == key => value.parse().ok(),
            _ => None,
        }
    })
}

pub fn parse_cpu_max(text: &str) -> Option<(u64, u64)> {
    let mut fields = text.split_whitespace();
    let quota = fields.next()?;
    let period = fields.next()?.parse().ok()?;
    if period == 0 {
        return None;
    }
    Some((quota.parse().ok()?, period))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIo {
    pub device: String,
    pub rbytes: u64,
    pub wbytes: u64,
}

pub fn parse_io_stat(text: &str) -> Vec<RawIo> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let device = fields.next()?;
            if !device.contains(':') {
                return None;
            }
            let mut rbytes = None;
            let mut wbytes = None;
            for field in fields {
                match field.split_once('=') {
                    Some(("rbytes", value)) => rbytes = value.parse().ok(),
                    Some(("wbytes", value)) => wbytes = value.parse().ok(),
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

pub fn rate(before: u64, after: u64, elapsed_secs: f64) -> f64 {
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 {
        return 0.0;
    }
    after.saturating_sub(before) as f64 / elapsed_secs
}

pub fn max_value_to_option(value: Option<MaxValue>) -> Option<u64> {
    match value {
        None | Some(MaxValue::Max) => None,
        Some(MaxValue::Value(bytes)) if bytes < 0 => None,
        Some(MaxValue::Value(bytes)) => Some(bytes as u64),
    }
}
