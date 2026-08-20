use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use scoop_wire::Digest256;

use crate::{
    CacheReceiptDecodeError, CacheReceiptFingerprintV1, ConeCompileCacheKeyV1, SnapshotFileError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheNondeterminismV1 {
    pub(super) existing_artifact: Digest256,
    pub(super) produced_artifact: Digest256,
    pub(super) existing_receipt: CacheReceiptFingerprintV1,
    pub(super) produced_receipt: CacheReceiptFingerprintV1,
}

impl CacheNondeterminismV1 {
    pub const fn existing_artifact(self) -> Digest256 {
        self.existing_artifact
    }

    pub const fn produced_artifact(self) -> Digest256 {
        self.produced_artifact
    }

    pub const fn existing_receipt(self) -> CacheReceiptFingerprintV1 {
        self.existing_receipt
    }

    pub const fn produced_receipt(self) -> CacheReceiptFingerprintV1 {
        self.produced_receipt
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CachePathRole {
    CacheRoot,
    Namespace,
    LockDirectory,
    StagingDirectory,
    LockFile,
    EntryDirectory,
    ArtifactFile,
    ReceiptFile,
}

impl fmt::Display for CachePathRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CacheRoot => "cache root",
            Self::Namespace => "compile cache namespace",
            Self::LockDirectory => "compile cache lock directory",
            Self::StagingDirectory => "compile cache staging directory",
            Self::LockFile => "compile cache lock file",
            Self::EntryDirectory => "compile cache entry directory",
            Self::ArtifactFile => "cached artifact",
            Self::ReceiptFile => "cache receipt",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheIoOperation {
    CreateDirectory,
    CreateFile,
    Open,
    Inspect,
    ListDirectory,
    Lock,
    Write,
    Sync,
    SetPermissions,
    Publish,
}

impl fmt::Display for CacheIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateDirectory => "create directory",
            Self::CreateFile => "create file",
            Self::Open => "open",
            Self::Inspect => "inspect",
            Self::ListDirectory => "list directory",
            Self::Lock => "lock",
            Self::Write => "write",
            Self::Sync => "sync",
            Self::SetPermissions => "set permissions on",
            Self::Publish => "publish",
        })
    }
}

#[derive(Debug)]
pub enum CompileCacheStoreError {
    Io {
        operation: CacheIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidNamespace(PathBuf),
    InvalidPathType {
        role: CachePathRole,
        path: PathBuf,
    },
    UnexpectedEntryContents {
        path: PathBuf,
        names: Vec<OsString>,
    },
    Snapshot {
        role: CachePathRole,
        source: SnapshotFileError,
    },
    ReceiptDecode(Box<CacheReceiptDecodeError>),
    ReceiptEncode(scoop_wire::cbor::EncodeError),
    ReceiptKeyMismatch {
        expected: ConeCompileCacheKeyV1,
        actual: ConeCompileCacheKeyV1,
    },

    WrongLock,
    CandidateVerificationMissing,
    CandidateVerificationMismatch,
    PublishedEntryDisappeared(PathBuf),
    NondeterministicProduction(Box<CacheNondeterminismV1>),
    AtomicNoReplaceUnavailable(PathBuf),
}

impl fmt::Display for CompileCacheStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::InvalidNamespace(path) => {
                write!(
                    formatter,
                    "invalid compile cache namespace {}",
                    path.display()
                )
            }
            Self::InvalidPathType { role, path } => {
                write!(
                    formatter,
                    "{role} {} has the wrong file type",
                    path.display()
                )
            }
            Self::UnexpectedEntryContents { path, names } => write!(
                formatter,
                "compile cache entry {} has unexpected contents {names:?}",
                path.display()
            ),
            Self::Snapshot { role, source } => {
                write!(formatter, "cannot snapshot {role}: {source}")
            }
            Self::ReceiptDecode(source) => write!(formatter, "invalid cache receipt: {source}"),
            Self::ReceiptEncode(source) => {
                write!(formatter, "cannot encode cache receipt: {source}")
            }
            Self::ReceiptKeyMismatch { expected, actual } => write!(
                formatter,
                "cache receipt key mismatch: expected {expected}, found {actual}"
            ),

            Self::WrongLock => {
                formatter.write_str("cache operation received the wrong per-key lock")
            }
            Self::CandidateVerificationMissing => {
                formatter.write_str("private cache candidate disappeared before publication")
            }
            Self::CandidateVerificationMismatch => {
                formatter.write_str("private cache candidate changed before publication")
            }
            Self::PublishedEntryDisappeared(path) => write!(
                formatter,
                "published cache entry {} disappeared during verification",
                path.display()
            ),
            Self::NondeterministicProduction(error) => write!(
                formatter,
                "nondeterministic cache production: existing artifact {} receipt {}, produced artifact {} receipt {}",
                error.existing_artifact,
                error.existing_receipt,
                error.produced_artifact,
                error.produced_receipt,
            ),
            Self::AtomicNoReplaceUnavailable(path) => write!(
                formatter,
                "atomic no-replace directory publication is unavailable for {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CompileCacheStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Snapshot { source, .. } => Some(source),
            Self::ReceiptDecode(source) => Some(source),
            Self::ReceiptEncode(source) => Some(source),
            _ => None,
        }
    }
}
