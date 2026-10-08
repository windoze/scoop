use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryIoOperation {
    Inspect,
    Canonicalize,
    ListDirectory,
    ReadLink,
    ReadSource,
}

impl fmt::Display for DiscoveryIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inspect => "inspect",
            Self::Canonicalize => "canonicalize",
            Self::ListDirectory => "list directory",
            Self::ReadLink => "read symlink",
            Self::ReadSource => "read source",
        })
    }
}

#[derive(Debug)]
pub struct SourceDiscoveryError {
    path: PathBuf,
    kind: SourceDiscoveryErrorKind,
}

impl SourceDiscoveryError {
    pub(crate) fn new(path: PathBuf, kind: SourceDiscoveryErrorKind) -> Self {
        Self { path, kind }
    }

    pub(crate) fn io(
        operation: DiscoveryIoOperation,
        path: PathBuf,
        source: std::io::Error,
    ) -> Self {
        Self::new(path, SourceDiscoveryErrorKind::Io { operation, source })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> &SourceDiscoveryErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum SourceDiscoveryErrorKind {
    SourceRootNotDirectory,
    NonUtf8EntryName,
    EscapesConeRoot,
    SymlinkCycle,
    DirectoryCycle,
    InvalidLogicalPath(NormalizedSourcePathError),
    SelectedPathNotSource,
    ConflictingSelections {
        first: String,
        second: String,
    },
    InvalidUtf8Source,
    SourceChangedDuringDiscovery,
    EmptySourceSet,

    LengthOverflow,
    Allocation {
        requested_bytes: u64,
    },
    IdentityHash(String),
    InvalidSourceIdentity(String),
    Io {
        operation: DiscoveryIoOperation,
        source: std::io::Error,
    },
}

impl fmt::Display for SourceDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source discovery failed at {}: ",
            self.path.display()
        )?;
        match &self.kind {
            SourceDiscoveryErrorKind::SourceRootNotDirectory => {
                formatter.write_str("resolved src root is not a directory")
            }
            SourceDiscoveryErrorKind::NonUtf8EntryName => {
                formatter.write_str("directory entry name is not valid UTF-8")
            }
            SourceDiscoveryErrorKind::EscapesConeRoot => {
                formatter.write_str("symlink target escapes the Cone root")
            }
            SourceDiscoveryErrorKind::SymlinkCycle => {
                formatter.write_str("symlink targets form a cycle")
            }
            SourceDiscoveryErrorKind::DirectoryCycle => {
                formatter.write_str("symlink traversal creates a directory cycle")
            }
            SourceDiscoveryErrorKind::InvalidLogicalPath(error) => error.fmt(formatter),
            SourceDiscoveryErrorKind::SelectedPathNotSource => {
                formatter.write_str("selected path must be a directory or a regular .scoop file")
            }
            SourceDiscoveryErrorKind::ConflictingSelections { first, second } => {
                write!(
                    formatter,
                    "source selections overlap or select the same file: {first}; {second}"
                )
            }
            SourceDiscoveryErrorKind::InvalidUtf8Source => {
                formatter.write_str("source file is not valid UTF-8")
            }
            SourceDiscoveryErrorKind::SourceChangedDuringDiscovery => {
                formatter.write_str("source target changed during discovery")
            }
            SourceDiscoveryErrorKind::EmptySourceSet => {
                formatter.write_str("manifest Cone must contain at least one .scoop source")
            }

            SourceDiscoveryErrorKind::LengthOverflow => {
                formatter.write_str("source length does not fit the bounded reader")
            }
            SourceDiscoveryErrorKind::Allocation { requested_bytes } => write!(
                formatter,
                "cannot allocate {requested_bytes} bytes for source discovery"
            ),
            SourceDiscoveryErrorKind::IdentityHash(error) => {
                write!(formatter, "cannot derive Cone identity: {error}")
            }
            SourceDiscoveryErrorKind::InvalidSourceIdentity(error) => {
                write!(formatter, "cannot construct source identity: {error}")
            }
            SourceDiscoveryErrorKind::Io { operation, source } => {
                write!(formatter, "cannot {operation}: {source}")
            }
        }
    }
}

impl std::error::Error for SourceDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            SourceDiscoveryErrorKind::InvalidLogicalPath(error) => Some(error),
            SourceDiscoveryErrorKind::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
