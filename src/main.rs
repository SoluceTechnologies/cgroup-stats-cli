mod cli;

use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};


fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(xxxxx),
        None => {
            Cli::command()
                .print_help()
                .expect("An error occurred while printing help");
        }
    }
}
