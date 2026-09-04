use crate::task::Stats;
use crate::task::metrics::{Io, IoDevice, Memory};
use crate::utils::bytes::{iec, limit};
use comfy_table::{Table, presets::UTF8_FULL};
use serde_json::{Map, Value, json};
use std::fmt::Write;

fn memory_limits(memory: &Memory) -> String {
    match (memory.high, memory.max) {
        (None, max) => limit(max),
        (Some(high), None) => format!("{} high", iec(high)),
        (Some(high), Some(max)) => format!("{} high / {} max", iec(high), iec(max)),
    }
}

pub fn json(stats: &Stats) -> String {
    let mut object = Map::new();
    object.insert("path".into(), json!(stats.path));
    if let Some(Ok(cpu)) = &stats.cpu {
        object.insert("cpu".into(), serde_json::to_value(cpu).unwrap());
    }
    if let Some(Ok(memory)) = &stats.memory {
        object.insert("memory".into(), serde_json::to_value(memory).unwrap());
    }
    if let Some(Ok(pids)) = &stats.pids {
        object.insert("pids".into(), serde_json::to_value(pids).unwrap());
    }
    if let Some(Ok(io)) = &stats.io {
        object.insert("io".into(), serde_json::to_value(io).unwrap());
    }
    serde_json::to_string_pretty(&Value::Object(object)).unwrap()
}

pub fn table(stats: &Stats) -> String {
    let mut grid = Table::new();
    grid.load_preset(UTF8_FULL)
        .set_header(vec!["Metric", "Current", "Limit"]);

    if let Some(memory) = &stats.memory {
        match memory {
            Ok(reading) => grid.add_row(vec![
                "RAM".into(),
                iec(reading.current),
                memory_limits(reading),
            ]),
            Err(reason) => grid.add_row(vec!["RAM".into(), "n/a".into(), reason.clone()]),
        };
    }
    if let Some(cpu) = &stats.cpu {
        match cpu {
            Ok(reading) => grid.add_row(vec![
                "CPU (cores)".into(),
                format!("{:.2}", reading.used_cores),
                reading
                    .max_cores
                    .map_or("unlimited".into(), |cores| format!("{cores:.2}")),
            ]),
            Err(reason) => grid.add_row(vec!["CPU (cores)".into(), "n/a".into(), reason.clone()]),
        };
    }
    if let Some(pids) = &stats.pids {
        match pids {
            Ok(reading) => grid.add_row(vec![
                "PIDs".into(),
                reading.current.to_string(),
                reading
                    .max
                    .map_or("unlimited".into(), |count| count.to_string()),
            ]),
            Err(reason) => grid.add_row(vec!["PIDs".into(), "n/a".into(), reason.clone()]),
        };
    }
    if let Some(io) = &stats.io {
        match io {
            Ok(reading) => {
                let active = active_devices(reading);
                if active.is_empty() {
                    grid.add_row(vec!["IO", "no activity", ""]);
                } else {
                    for device in active {
                        grid.add_row(vec![
                            format!("IO {}", device.device),
                            format!("r {}/s", iec(device.read_bytes_per_sec as u64)),
                            format!("w {}/s", iec(device.write_bytes_per_sec as u64)),
                        ]);
                    }
                }
            }
            Err(reason) => {
                grid.add_row(vec!["IO".into(), "n/a".into(), reason.clone()]);
            }
        };
    }
    grid.to_string()
}

fn active_devices(io: &Io) -> Vec<&IoDevice> {
    io.devices
        .iter()
        .filter(|device| device.read_bytes_per_sec > 0.0 || device.write_bytes_per_sec > 0.0)
        .collect()
}

pub fn human(stats: &Stats) -> String {
    let mut out = String::new();

    if let Some(memory) = &stats.memory {
        match memory {
            Ok(reading) => writeln!(
                out,
                "RAM:  {} / {}",
                iec(reading.current),
                memory_limits(reading)
            )
            .unwrap(),
            Err(reason) => writeln!(out, "RAM:  n/a ({reason})").unwrap(),
        }
    }
    if let Some(cpu) = &stats.cpu {
        match cpu {
            Ok(reading) => {
                let max = reading
                    .max_cores
                    .map_or("unlimited".to_string(), |cores| format!("{cores:.2}"));
                writeln!(out, "CPU:  {:.2} / {} cores", reading.used_cores, max).unwrap()
            }
            Err(reason) => writeln!(out, "CPU:  n/a ({reason})").unwrap(),
        }
    }
    if let Some(pids) = &stats.pids {
        match pids {
            Ok(reading) => {
                let max = reading
                    .max
                    .map_or("unlimited".to_string(), |count| count.to_string());
                writeln!(out, "PIDs: {} / {}", reading.current, max).unwrap()
            }
            Err(reason) => writeln!(out, "PIDs: n/a ({reason})").unwrap(),
        }
    }
    if let Some(io) = &stats.io {
        match io {
            Ok(reading) => {
                let active = active_devices(reading);
                if active.is_empty() {
                    writeln!(out, "IO:   no activity").unwrap()
                } else {
                    for (position, device) in active.iter().enumerate() {
                        let label = if position == 0 { "IO:  " } else { "     " };
                        writeln!(
                            out,
                            "{label} {}  r {}/s  w {}/s",
                            device.device,
                            iec(device.read_bytes_per_sec as u64),
                            iec(device.write_bytes_per_sec as u64)
                        )
                        .unwrap()
                    }
                }
            }
            Err(reason) => writeln!(out, "IO:   n/a ({reason})").unwrap(),
        }
    }
    out
}
