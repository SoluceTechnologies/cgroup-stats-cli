use cgroup_stats_cli::cli::{Args, Format};
use cgroup_stats_cli::task::{collect, format};
use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = Args::parse();
    let stats = match collect(&args.path, args.selection(), args.interval) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    print!(
        "{}",
        match args.format {
            Format::Human => format::human(&stats),
            Format::Json => format::json(&stats) + "\n",
            Format::Table => format::table(&stats) + "\n",
        }
    );
    ExitCode::SUCCESS
}
