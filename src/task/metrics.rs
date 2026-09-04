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
    /// `memory.high`: the kernel reclaims aggressively and stalls the cgroup
    /// above this, but does not OOM-kill. Distinct from `max`, and commonly the
    /// only limit systemd sets.
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

pub fn collect_memory(cg: &Cgroup, dir: &Path) -> Result<Memory, String> {
    // memory.high is required alongside the others so get_mem()'s habit of
    // mapping a failed read onto MaxValue::Max cannot masquerade as "no limit".
    require(dir, &["memory.current", "memory.high", "memory.max"])?;
    let c: &MemController = cg
        .controller_of()
        .ok_or_else(|| "memory controller unavailable".to_string())?;
    let set = c.get_mem().map_err(|e| e.to_string())?;
    Ok(Memory {
        current: c.memory_stat().usage_in_bytes,
        high: max_value_to_option(set.high),
        max: max_value_to_option(set.max),
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

pub fn collect_cpu(dir: &Path, before: u64, after: u64, elapsed: f64) -> Result<Cpu, String> {
    require(dir, &["cpu.stat"])?;

    let max_cores = match std::fs::read_to_string(dir.join("cpu.max")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("cpu.max: {e}")),
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
        .map(|a| {
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
