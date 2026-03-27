use std::sync::Arc;

use assfonts_lib::{
    cli::{Cli, Commands},
    error::Result,
    processing::{run_build, run_process},
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
