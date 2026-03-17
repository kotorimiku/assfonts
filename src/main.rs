use std::sync::Arc;

use assfonts::{
    cli::{Cli, Commands},
    commands::{run_build, run_process},
    error::Result,
};
use clap::Parser;

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Build(options)) => run_build(options),
        None => run_process(
            cli.run,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            |_| {},
        ),
    }
}
