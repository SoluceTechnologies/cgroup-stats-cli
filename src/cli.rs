use clap::{Parser, ValueEnum};
use std::time::Duration;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Human,
    Json,
    Table,
}

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = env!("CARGO_PKG_DESCRIPTION"),
)]
pub struct Args {
    /// cgroup path, absolute or relative to the hierarchy root
    #[arg(short = 'p', long)]
    pub path: String,

    /// Show CPU statistics
    #[arg(short = 'c', long)]
    pub cpu: bool,

    /// Show memory statistics
    #[arg(short = 'm', long)]
    pub mem: bool,

    /// Show PID statistics
    #[arg(short = 'P', long)]
    pub pids: bool,

    /// Show block IO statistics
    #[arg(short = 'b', long)]
    pub io: bool,

    /// Sampling window in seconds for CPU and IO deltas
    #[arg(
        short = 'i',
        long,
        default_value_t = 1.0,
        value_parser = positive_secs,
        value_name = "SECS"
    )]
    pub interval: f64,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = Format::Human)]
    pub format: Format,
}

fn positive_secs(raw: &str) -> Result<f64, String> {
    let seconds: f64 = raw
        .parse()
        .map_err(|_| format!("`{raw}` is not a number"))?;
    if seconds > 0.0 && Duration::try_from_secs_f64(seconds).is_ok() {
        Ok(seconds)
    } else {
        Err(format!(
            "interval must be a positive, representable number of seconds, got `{raw}`"
        ))
    }
}

/// Separate from `Args` so the collector need not re-derive "no flags means all".
#[derive(Copy, Clone, Debug)]
pub struct Selection {
    pub cpu: bool,
    pub mem: bool,
    pub pids: bool,
    pub io: bool,
}

impl Selection {
    /// CPU and IO are monotonic counters and need two samples around a sleep.
    /// Memory and PIDs do not, so a memory-only run must not sleep at all.
    pub fn needs_sampling(&self) -> bool {
        self.cpu || self.io
    }
}

impl Args {
    pub fn selection(&self) -> Selection {
        if self.cpu || self.mem || self.pids || self.io {
            Selection {
                cpu: self.cpu,
                mem: self.mem,
                pids: self.pids,
                io: self.io,
            }
        } else {
            Selection {
                cpu: true,
                mem: true,
                pids: true,
                io: true,
            }
        }
    }
}
