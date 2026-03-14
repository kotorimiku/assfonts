mod ass;
mod cff_fix;
mod cli;
mod commands;
mod embed;
mod error;
mod font;
mod subset;

use clap::Parser;

use crate::{
    cli::{Cli, Commands},
    commands::{run_build, run_process},
};

fn main() -> Result<(), color_eyre::Report> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Build(options)) => run_build(options),
        None => run_process(cli.run),
    }
}
