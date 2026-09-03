use clap::Parser;

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
}
