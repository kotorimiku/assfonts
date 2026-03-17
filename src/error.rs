use std::path::PathBuf;

use color_eyre::eyre;
use thiserror::Error;

pub type Result<T> = eyre::Result<T>;

#[derive(Debug, Error)]
pub enum AssfontsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("font path does not exist: {0}")]
    MissingFontPath(PathBuf),

    #[error("no font source available: provide --fontpath or a valid fonts.index.json in {0}")]
    MissingFontSource(PathBuf),

    #[error("output directory is invalid: {0}")]
    InvalidOutputDir(PathBuf),

    #[error("font processing error: {0}")]
    Font(String),

    #[error("missing required fonts: {1:?}, referenced in {0}")]
    MissingFonts(PathBuf, Vec<String>),

    #[error("font subsetting error: {0}")]
    Subset(#[from] allsorts::subset::SubsetError),

    #[error("parse error: {0}")]
    ParseError(#[from] allsorts::error::ParseError),

    #[error("read/write error: {0}")]
    ReadWriteError(#[from] allsorts::error::ReadWriteError),

    #[error("file already exists: {0}")]
    FileExists(PathBuf),

    #[error("process was interrupted")]
    Interrupted(),
}

#[macro_export]
macro_rules! bail {
    ($error:expr) => {
        return Err(::std::convert::Into::into($error))
    };
}
