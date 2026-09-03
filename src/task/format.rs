use crate::task::Stats;
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
            Ok(i) if i.devices.is_empty() => writeln!(o, "IO:   no activity").unwrap(),
            Ok(i) => {
                for (n, d) in i.devices.iter().enumerate() {
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
            cpu: Some(Ok(Cpu { used_cores: 0.53, max_cores: Some(2.0) })),
            memory: Some(Ok(Memory { current: 1_288_490_188, max: Some(4_294_967_296) })),
            pids: Some(Ok(Pids { current: 42, max: Some(512) })),
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
        s.memory = Some(Ok(Memory { current: 8192, max: None }));
        s.cpu = Some(Ok(Cpu { used_cores: 0.1, max_cores: None }));
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
}
