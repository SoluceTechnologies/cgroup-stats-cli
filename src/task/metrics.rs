use crate::utils::cgfile::{device_name, read, require};
use crate::utils::parse::{
    RawIo, max_value_to_option, parse_cpu_max, parse_flat_key, parse_io_stat, rate,
};
use cgroups_rs::fs::Cgroup;
use cgroups_rs::fs::memory::MemController;
use cgroups_rs::fs::pid::PidController;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Cpu {
    pub used_cores: f64,
    pub max_cores: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Memory {
    pub current: u64,
    pub high: Option<u64>,
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

pub fn collect_memory(cgroup: &Cgroup, dir: &Path) -> Result<Memory, String> {
    require(dir, &["memory.current", "memory.high", "memory.max"])?;
    let controller: &MemController = cgroup
        .controller_of()
        .ok_or_else(|| "memory controller unavailable".to_string())?;
    let limits = controller.get_mem().map_err(|err| err.to_string())?;
    Ok(Memory {
        current: controller.memory_stat().usage_in_bytes,
        high: max_value_to_option(limits.high),
        max: max_value_to_option(limits.max),
    })
}

pub fn collect_pids(cgroup: &Cgroup, dir: &Path) -> Result<Pids, String> {
    require(dir, &["pids.current", "pids.max"])?;
    let controller: &PidController = cgroup
        .controller_of()
        .ok_or_else(|| "pids controller unavailable".to_string())?;
    Ok(Pids {
        current: controller
            .get_pid_current()
            .map_err(|err| err.to_string())?,
        max: max_value_to_option(Some(
            controller.get_pid_max().map_err(|err| err.to_string())?,
        )),
    })
}

pub fn read_cpu_usage(dir: &Path) -> Result<u64, String> {
    let text = read(dir, "cpu.stat")?;
    parse_flat_key(&text, "usage_usec").ok_or_else(|| "cpu.stat has no usage_usec".to_string())
}

pub fn collect_cpu(dir: &Path, before: u64, after: u64, elapsed: f64) -> Result<Cpu, String> {
    require(dir, &["cpu.stat"])?;

    let max_cores = match std::fs::read_to_string(dir.join("cpu.max")) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(format!("cpu.max: {err}")),
        Ok(text) => match parse_cpu_max(&text) {
            Some((quota, period)) => Some(quota as f64 / period as f64),

            None if text.split_whitespace().next() == Some("max") => None,
            None => return Err(format!("cpu.max is unparseable: {:?}", text.trim())),
        },
    };
    Ok(Cpu {
        used_cores: rate(before, after, elapsed) / 1_000_000.0,
        max_cores,
    })
}

pub fn read_io(dir: &Path) -> Result<Vec<RawIo>, String> {
    require(dir, &["io.stat"])?;
    Ok(parse_io_stat(&read(dir, "io.stat")?))
}

pub fn collect_io(sys_dev_block: &Path, before: &[RawIo], after: &[RawIo], elapsed: f64) -> Io {
    let devices = after
        .iter()
        .map(|current| {
            let previous = before
                .iter()
                .find(|candidate| candidate.device == current.device);
            let (previous_read, previous_write) =
                previous.map_or((0, 0), |device| (device.rbytes, device.wbytes));
            IoDevice {
                device: device_name(sys_dev_block, &current.device),
                read_bytes_per_sec: rate(previous_read, current.rbytes, elapsed),
                write_bytes_per_sec: rate(previous_write, current.wbytes, elapsed),
            }
        })
        .collect();
    Io { devices }
}
