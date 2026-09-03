use crate::task::Stats;
use crate::task::metrics::{Io, IoDevice};
use comfy_table::{Table, presets::UTF8_FULL};
use serde_json::{Map, Value, json};
use std::fmt::Write;

/// Format bytes with IEC units, matching `numfmt --to=iec`.
pub fn iec(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    // The unit is chosen before the value is rounded, so a mantissa just under
    // the threshold rounds back up to 1024.0 and prints as "1024.0K" instead of
    // "1.0M". Re-check against the rounded value.
    if i < UNITS.len() - 1 && (v * 10.0).round() / 10.0 >= 1024.0 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes}B")
    } else {
        format!("{v:.1}{}", UNITS[i])
    }
}

fn limit(v: Option<u64>) -> String {
    v.map_or("unlimited".to_string(), iec)
}

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

/// Devices that moved data during the sampling window.
///
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::metrics::{Cpu, Io, IoDevice, Memory, Pids};

    fn stats() -> Stats {
        Stats {
            path: "system.slice/foo.service".into(),
            cpu: Some(Ok(Cpu {
                used_cores: 0.53,
                max_cores: Some(2.0),
            })),
            memory: Some(Ok(Memory {
                current: 1_288_490_188,
                max: Some(4_294_967_296),
            })),
            pids: Some(Ok(Pids {
                current: 42,
                max: Some(512),
            })),
            io: Some(Ok(Io {
                devices: vec![IoDevice {
                    device: "sda".into(),
                    read_bytes_per_sec: 1_258_291.0,
                    write_bytes_per_sec: 348_160.0,
                }],
            })),
        }
    }

    #[test]
    fn iec_boundaries() {
        assert_eq!(iec(0), "0B");
        assert_eq!(iec(1023), "1023B");
        assert_eq!(iec(1024), "1.0K");
        assert_eq!(iec(1536), "1.5K");
        assert_eq!(iec(1024 * 1024), "1.0M");
        assert_eq!(iec(4 * 1024 * 1024 * 1024), "4.0G");
        assert_eq!(iec(2 * 1024_u64.pow(4)), "2.0T");
        // A value one byte below a unit boundary must not round up into a
        // "1024.0" mantissa of the smaller unit.
        assert_eq!(iec(1_048_575), "1.0M");
        assert_eq!(iec(1_073_741_823), "1.0G");
        assert_eq!(iec(1_099_511_627_775), "1.0T");
        // The ladder stops at the last unit rather than indexing past it.
        assert_eq!(iec(u64::MAX), "16384.0P");
    }

    #[test]
    fn human_renders_every_metric() {
        let out = human(&stats());
        // Pin the full line, not loose substrings: checking "1.2G" and "4.0G"
        // independently would pass even if current and max were swapped.
        assert!(out.contains("RAM:  1.2G / 4.0G"), "{out}");
        assert!(out.contains("CPU:  0.53 / 2.00 cores"), "{out}");
        assert!(out.contains("PIDs: 42 / 512"), "{out}");
        assert!(out.contains("sda  r 1.2M/s  w 340.0K/s"), "{out}");
    }

    #[test]
    fn unlimited_renders_as_the_word_unlimited() {
        let mut s = stats();
        s.memory = Some(Ok(Memory {
            current: 8192,
            max: None,
        }));
        s.cpu = Some(Ok(Cpu {
            used_cores: 0.1,
            max_cores: None,
        }));
        let out = human(&s);
        assert!(out.contains("8.0K / unlimited"), "{out}");
        assert!(out.contains("0.10 / unlimited cores"), "{out}");
    }

    #[test]
    fn unavailable_metrics_say_why() {
        let mut s = stats();
        s.memory = Some(Err("memory.current not present".into()));
        let out = human(&s);
        assert!(out.contains("n/a"), "{out}");
        assert!(out.contains("memory.current not present"), "{out}");
    }

    #[test]
    fn unrequested_metrics_are_absent_entirely() {
        let mut s = stats();
        s.pids = None;
        s.io = None;
        let out = human(&s);
        assert!(!out.contains("PIDs"), "{out}");
        assert!(!out.contains("IO"), "{out}");
    }

    #[test]
    fn io_with_no_devices_says_so_rather_than_printing_nothing() {
        let mut s = stats();
        s.io = Some(Ok(Io { devices: vec![] }));
        let out = human(&s);
        assert!(out.contains("IO:"), "{out}");
        assert!(out.contains("no activity"), "{out}");
    }

    #[test]
    fn json_uses_raw_values_and_null_for_unlimited() {
        let mut s = stats();
        s.memory = Some(Ok(Memory {
            current: 8192,
            max: None,
        }));
        let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
        assert_eq!(v["memory"]["current"], 8192);
        assert!(v["memory"]["max"].is_null());
        assert_eq!(v["pids"]["current"], 42);
    }

    #[test]
    fn json_omits_unrequested_metrics() {
        let mut s = stats();
        s.io = None;
        let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
        assert!(
            v.get("io").is_none(),
            "unrequested metrics must be absent: {v}"
        );
        assert!(v.get("memory").is_some());
    }

    #[test]
    fn json_is_valid_when_a_metric_is_unavailable() {
        let mut s = stats();
        s.memory = Some(Err("memory.current not present".into()));
        let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
        assert!(
            v.get("memory").is_none(),
            "unavailable metrics are omitted: {v}"
        );
    }

    #[test]
    fn table_contains_the_values() {
        let out = table(&stats());
        assert!(out.contains("RAM"), "{out}");
        assert!(out.contains("1.2G"), "{out}");
        assert!(out.contains("sda"), "{out}");
    }

    #[test]
    fn table_marks_an_unavailable_metric_and_says_why() {
        let mut s = stats();
        s.memory = Some(Err("memory.current not present".into()));
        let out = table(&s);
        assert!(out.contains("n/a"), "{out}");
        assert!(out.contains("memory.current not present"), "{out}");
    }

    #[test]
    fn table_omits_unrequested_metrics_entirely() {
        let mut s = stats();
        s.pids = None;
        s.io = None;
        let out = table(&s);
        assert!(!out.contains("PIDs"), "{out}");
        assert!(!out.contains("IO"), "{out}");
    }

    #[test]
    fn both_renderers_hide_idle_devices_but_json_keeps_them() {
        use crate::task::metrics::IoDevice;
        let mut s = stats();
        s.io = Some(Ok(Io {
            devices: vec![
                IoDevice {
                    device: "loop0".into(),
                    read_bytes_per_sec: 0.0,
                    write_bytes_per_sec: 0.0,
                },
                IoDevice {
                    device: "nvme0n1".into(),
                    read_bytes_per_sec: 2048.0,
                    write_bytes_per_sec: 0.0,
                },
            ],
        }));
        let h = human(&s);
        assert!(h.contains("nvme0n1"), "{h}");
        assert!(
            !h.contains("loop0"),
            "idle device must not appear in human output: {h}"
        );

        let t = table(&s);
        assert!(t.contains("nvme0n1"), "{t}");
        assert!(
            !t.contains("loop0"),
            "idle device must not appear in table output: {t}"
        );

        // JSON is the fidelity layer and keeps every device.
        let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
        let devs = v["io"]["devices"].as_array().unwrap();
        assert_eq!(devs.len(), 2, "json must keep idle devices: {v}");
    }

    #[test]
    fn all_devices_idle_renders_as_no_activity() {
        use crate::task::metrics::IoDevice;
        let mut s = stats();
        s.io = Some(Ok(Io {
            devices: vec![IoDevice {
                device: "loop0".into(),
                read_bytes_per_sec: 0.0,
                write_bytes_per_sec: 0.0,
            }],
        }));
        assert!(human(&s).contains("no activity"), "{}", human(&s));
        assert!(table(&s).contains("no activity"), "{}", table(&s));
    }
}
