use clap::Parser;
use cgroup_stats_cli::cli::Args;

fn main() {
    let args = Args::parse();
    println!("{}", args.path);
}
