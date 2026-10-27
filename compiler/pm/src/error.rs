use std::{
    fmt,
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestOrigin {
    pub manifest: PathBuf,
    pub span: Range<usize>,
}

#[derive(Debug)]
pub struct PmError {
    pub message: String,
    pub origins: Vec<ManifestOrigin>,
}

pub type PmResult<T> = Result<T, PmError>;

impl PmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            origins: Vec::new(),
        }
    }

    pub fn at(mut self, origin: ManifestOrigin) -> Self {
        self.origins.push(origin);
        self
    }

    pub(crate) fn io(path: &Path, error: std::io::Error) -> Self {
        Self::new(format!("{}: {error}", path.display()))
    }
}

impl fmt::Display for PmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        for origin in &self.origins {
            write!(
                f,
                "\n  at {} (bytes {}..{})",
                origin.manifest.display(),
                origin.span.start,
                origin.span.end
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for PmError {}
