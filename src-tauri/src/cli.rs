use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "assfonts",
    version,
    about = "ASS font utility rewritten in Rust",
    subcommand_negates_reqs = true,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub run: RunOptions,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Build(BuildOptions),
}

#[derive(Debug, Args, Clone, serde::Deserialize)]
#[cfg_attr(feature = "gui", derive(specta::Type))]
pub struct BuildOptions {
    #[arg(short = 'f', long = "fontpath", required = true, num_args = 1.., help = "Path to font files")]
    pub fontpaths: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long = "output",
        default_value = ".",
        help = "Directory to write fonts.index.json"
    )]
    pub output: PathBuf,
}

#[derive(Debug, Args, Clone, serde::Deserialize)]
#[cfg_attr(feature = "gui", derive(specta::Type))]
pub struct RunOptions {
    #[arg(short = 'i', long = "input", required = true, num_args = 1.., help = "Input ASS files or directories")]
    pub inputs: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long = "output",
        default_value = ".",
        help = "Output directory"
    )]
    pub output: PathBuf,

    #[arg(short = 'f', long = "fontpath", num_args = 1.., help = "Path to font files")]
    pub fontpaths: Option<Vec<PathBuf>>,

    #[arg(
        short = 'd',
        long = "dbpath",
        default_value = ".",
        help = "Directory containing fonts.index.json"
    )]
    pub dbpath: PathBuf,

    #[arg(
        short = 's',
        long = "strict",
        action = ArgAction::Set,
        default_value_t = true,
        value_name = "BOOL",
        help = "Fail on font usage errors"
    )]
    pub strict: bool,

    #[arg(
        long = "allow-missing-sample",
        help = "Allow missing character samples"
    )]
    pub allow_missing_sample: bool,

    #[arg(long = "allow-missing-fonts", help = "Allow missing fonts")]
    pub allow_missing_fonts: bool,

    #[arg(
        short,
        long = "report",
        help = "Write run-report.json after processing"
    )]
    pub report: bool,

    #[arg(long = "force", help = "Overwrite existing output files")]
    pub force: bool,
}
