use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheIoOperation {
    CreateDirectory,
    Open,
    Inspect,
    Lock,
    Read,
    Write,
    Publish,
}

#[derive(Debug)]
pub enum CompileCacheStoreError {
    Io {
        operation: CacheIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    Encode(scoop_wire::cbor::EncodeError),
    Decode(scoop_wire::WireError),
    WrongLock,
}

impl CompileCacheStoreError {
    pub(super) fn io(
        operation: CacheIoOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for CompileCacheStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "cache {operation:?} {}: {source}", path.display()),
            Self::Encode(source) => write!(f, "cannot write build record: {source}"),
            Self::Decode(source) => write!(f, "cannot read build record: {source}"),
            Self::WrongLock => {
                f.write_str("compile cache operation requires the matching build lock")
            }
        }
    }
}

impl std::error::Error for CompileCacheStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Encode(source) => Some(source),
            Self::Decode(source) => Some(source),
            Self::WrongLock => None,
        }
    }
}
