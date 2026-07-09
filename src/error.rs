use std::path::PathBuf;

use color_eyre::eyre;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AssfontsError>;

#[derive(Debug, Error)]
pub enum AssfontsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("font path does not exist: {0}")]
    MissingFontPath(PathBuf),

    #[error("no font source available: provide --fontpath or a valid fonts.db in {0}")]
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

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("other error: {0}")]
    Other(#[from] eyre::Report),
}

impl AssfontsError {
    #[track_caller]
    pub fn into_report(self) -> color_eyre::eyre::Report {
        match self {
            AssfontsError::Other(report) => report,
            err => color_eyre::eyre::Report::new(err),
        }
    }
}

#[macro_export]
macro_rules! bail {
    ($fmt:literal, $($arg:tt)*) => {
        return Err($crate::error::AssfontsError::Other(::color_eyre::eyre::eyre!($fmt, $($arg)*)))
    };
    ($msg:literal) => {
        return Err($crate::error::AssfontsError::Other(::color_eyre::eyre::eyre!($msg)))
    };
    ($error:expr) => {
        return Err(::std::convert::Into::into($error))
    };
}
