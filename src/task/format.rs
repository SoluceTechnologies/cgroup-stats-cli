use crate::task::Stats;
use crate::task::metrics::{Io, IoDevice};
use crate::utils::bytes::{iec, limit};
use comfy_table::{Table, presets::UTF8_FULL};
use serde_json::{Map, Value, json};
use std::fmt::Write;

/// Only readings appear. A metric that was not requested, or was requested but
/// unavailable, is omitted — a consumer checking for a key is a cleaner
/// contract than a sentinel value.
pub fn json(s: &Stats) -> String {
    let mut m = Map::new();
    m.insert("path".into(), json!(s.path));
    if let Some(Ok(v)) = &s.cpu {
        m.insert("cpu".into(), serde_json::to_value(v).unwrap());
    }
    if let Some(Ok(v)) = &s.memory {
        m.insert("memory".into(), serde_json::to_value(v).unwrap());
    }
    if let Some(Ok(v)) = &s.pids {
        m.insert("pids".into(), serde_json::to_value(v).unwrap());
    }
    if let Some(Ok(v)) = &s.io {
        m.insert("io".into(), serde_json::to_value(v).unwrap());
    }
    serde_json::to_string_pretty(&Value::Object(m)).unwrap()
}

pub fn table(s: &Stats) -> String {
    let mut t = Table::new();
    t.load_preset(UTF8_FULL)
        .set_header(vec!["Metric", "Current", "Limit"]);

    if let Some(m) = &s.memory {
        match m {
            Ok(m) => t.add_row(vec!["RAM".into(), iec(m.current), limit(m.max)]),
            Err(e) => t.add_row(vec!["RAM".into(), "n/a".into(), e.clone()]),
        };
    }
    if let Some(c) = &s.cpu {
        match c {
            Ok(c) => t.add_row(vec![
                "CPU (cores)".into(),
                format!("{:.2}", c.used_cores),
                c.max_cores
                    .map_or("unlimited".into(), |v| format!("{v:.2}")),
            ]),
            Err(e) => t.add_row(vec!["CPU (cores)".into(), "n/a".into(), e.clone()]),
        };
    }
    if let Some(p) = &s.pids {
        match p {
            Ok(p) => t.add_row(vec![
                "PIDs".into(),
                p.current.to_string(),
                p.max.map_or("unlimited".into(), |v| v.to_string()),
            ]),
            Err(e) => t.add_row(vec!["PIDs".into(), "n/a".into(), e.clone()]),
        };
    }
    if let Some(i) = &s.io {
        match i {
            Ok(i) => {
                let active = active_devices(i);
                if active.is_empty() {
                    t.add_row(vec!["IO", "no activity", ""]);
                } else {
                    for d in active {
                        t.add_row(vec![
                            format!("IO {}", d.device),
                            format!("r {}/s", iec(d.read_bytes_per_sec as u64)),
                            format!("w {}/s", iec(d.write_bytes_per_sec as u64)),
                        ]);
                    }
                }
            }
            Err(e) => {
                t.add_row(vec!["IO".into(), "n/a".into(), e.clone()]);
            }
        };
    }
    t.to_string()
}

/// A leaf cgroup's `io.stat` lists only devices that cgroup has touched, but
/// the root's enumerates every block device on the host — dozens of unbroken
/// zero rows with any real traffic buried among them. The human-facing
/// renderers exist to be glanceable and a rate view's job is showing what
/// moves, so they filter. `json` keeps every device.
fn active_devices(io: &Io) -> Vec<&IoDevice> {
    io.devices
        .iter()
        .filter(|d| d.read_bytes_per_sec > 0.0 || d.write_bytes_per_sec > 0.0)
        .collect()
}

pub fn human(s: &Stats) -> String {
    let mut o = String::new();

    if let Some(m) = &s.memory {
        match m {
            Ok(m) => writeln!(o, "RAM:  {} / {}", iec(m.current), limit(m.max)).unwrap(),
            Err(e) => writeln!(o, "RAM:  n/a ({e})").unwrap(),
        }
    }
    if let Some(c) = &s.cpu {
        match c {
            Ok(c) => {
                let max = c
                    .max_cores
                    .map_or("unlimited".to_string(), |v| format!("{v:.2}"));
                writeln!(o, "CPU:  {:.2} / {} cores", c.used_cores, max).unwrap()
            }
            Err(e) => writeln!(o, "CPU:  n/a ({e})").unwrap(),
        }
    }
    if let Some(p) = &s.pids {
        match p {
            Ok(p) => {
                let max = p.max.map_or("unlimited".to_string(), |v| v.to_string());
                writeln!(o, "PIDs: {} / {}", p.current, max).unwrap()
            }
            Err(e) => writeln!(o, "PIDs: n/a ({e})").unwrap(),
        }
    }
    if let Some(i) = &s.io {
        match i {
            Ok(i) => {
                let active = active_devices(i);
                if active.is_empty() {
                    writeln!(o, "IO:   no activity").unwrap()
                } else {
                    for (n, d) in active.iter().enumerate() {
                        let label = if n == 0 { "IO:  " } else { "     " };
                        writeln!(
                            o,
                            "{label} {}  r {}/s  w {}/s",
                            d.device,
                            iec(d.read_bytes_per_sec as u64),
                            iec(d.write_bytes_per_sec as u64)
                        )
                        .unwrap()
                    }
                }
            }
            Err(e) => writeln!(o, "IO:   n/a ({e})").unwrap(),
        }
    }
    o
}
