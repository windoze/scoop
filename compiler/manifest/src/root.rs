use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_wire::HashError;

use crate::{ManifestParseError, ParsedConeManifest, parse_cone_manifest};

const MANIFEST_FILE_NAME: &str = "Cone.toml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestRootLocator {
    ConeDirectory(PathBuf),
    ExactConeManifestFile(PathBuf),
}

impl ManifestRootLocator {
    pub fn cone_directory(path: impl Into<PathBuf>) -> Self {
        Self::ConeDirectory(path.into())
    }

    pub fn exact_manifest_file(path: impl Into<PathBuf>) -> Self {
        Self::ExactConeManifestFile(path.into())
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ManifestRootError> {
        let path = path.into();
        let metadata = std::fs::metadata(&path).map_err(|error| {
            ManifestRootError::io(ManifestRootIoOperation::Inspect, path.clone(), error)
        })?;
        if metadata.is_dir() {
            Ok(Self::ConeDirectory(path))
        } else if metadata.is_file() {
            if path
                .file_name()
                .is_some_and(|name| name == MANIFEST_FILE_NAME)
            {
                Ok(Self::ExactConeManifestFile(path))
            } else {
                Err(ManifestRootError::new(
                    path,
                    ManifestRootErrorKind::InvalidManifestFileName,
                ))
            }
        } else {
            Err(ManifestRootError::new(
                path,
                ManifestRootErrorKind::UnsupportedRootFileType,
            ))
        }
    }

    pub fn as_path(&self) -> &Path {
        match self {
            Self::ConeDirectory(path) | Self::ExactConeManifestFile(path) => path,
        }
    }
}

#[derive(Debug)]
pub struct LoadedConeManifest {
    real_root: PathBuf,
    manifest_path: PathBuf,
    source: String,
    identity: ConeIdentity,
    parsed: ParsedConeManifest,
}

impl LoadedConeManifest {
    pub fn real_root(&self) -> &Path {
        &self.real_root
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Returns the exact UTF-8 text read from `Cone.toml`.
    pub fn source_text(&self) -> &str {
        &self.source
    }

    /// Returns the exact original manifest bytes used to produce all spans.
    pub fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    pub fn coordinate(&self) -> &ConeCoordinate {
        self.parsed.semantic().coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub fn parsed(&self) -> &ParsedConeManifest {
        &self.parsed
    }

    pub fn into_parsed(self) -> ParsedConeManifest {
        self.parsed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestRootIoOperation {
    Inspect,
    Canonicalize,
    Read,
}

impl fmt::Display for ManifestRootIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inspect => "inspect",
            Self::Canonicalize => "canonicalize",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub struct ManifestRootError {
    path: PathBuf,
    kind: ManifestRootErrorKind,
}

impl ManifestRootError {
    fn new(path: PathBuf, kind: ManifestRootErrorKind) -> Self {
        Self { path, kind }
    }

    fn io(operation: ManifestRootIoOperation, path: PathBuf, source: std::io::Error) -> Self {
        Self::new(path, ManifestRootErrorKind::Io { operation, source })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> &ManifestRootErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum ManifestRootErrorKind {
    InvalidManifestFileName,
    UnsupportedRootFileType,
    RootNotDirectory,
    ManifestNotRegularFile,
    Io {
        operation: ManifestRootIoOperation,
        source: std::io::Error,
    },
    InvalidUtf8,
    Identity(HashError),
    Parse(ManifestParseError),
}

impl fmt::Display for ManifestRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Cone manifest root {}: ",
            self.path.display()
        )?;
        match &self.kind {
            ManifestRootErrorKind::InvalidManifestFileName => {
                formatter.write_str("manifest file must be named exactly Cone.toml")
            }
            ManifestRootErrorKind::UnsupportedRootFileType => {
                formatter.write_str("root must be a directory or regular Cone.toml file")
            }
            ManifestRootErrorKind::RootNotDirectory => {
                formatter.write_str("resolved Cone root is not a directory")
            }
            ManifestRootErrorKind::ManifestNotRegularFile => {
                formatter.write_str("resolved Cone.toml is not a regular file")
            }
            ManifestRootErrorKind::Io { operation, source } => {
                write!(formatter, "cannot {operation}: {source}")
            }
            ManifestRootErrorKind::InvalidUtf8 => {
                formatter.write_str("Cone.toml is not valid UTF-8")
            }
            ManifestRootErrorKind::Identity(error) => {
                write!(formatter, "cannot derive Cone identity: {error}")
            }
            ManifestRootErrorKind::Parse(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ManifestRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ManifestRootErrorKind::Io { source, .. } => Some(source),
            ManifestRootErrorKind::Identity(error) => Some(error),
            ManifestRootErrorKind::Parse(error) => Some(error),
            _ => None,
        }
    }
}

pub fn load_cone_manifest(
    locator: &ManifestRootLocator,
) -> Result<LoadedConeManifest, ManifestRootError> {
    let (real_root, manifest_path) = match locator {
        ManifestRootLocator::ConeDirectory(root) => {
            let real_root = canonicalize(root)?;
            if !std::fs::metadata(&real_root)
                .map_err(|error| {
                    ManifestRootError::io(
                        ManifestRootIoOperation::Inspect,
                        real_root.clone(),
                        error,
                    )
                })?
                .is_dir()
            {
                return Err(ManifestRootError::new(
                    root.clone(),
                    ManifestRootErrorKind::RootNotDirectory,
                ));
            }
            let manifest_path = real_root.join(MANIFEST_FILE_NAME);
            (real_root, manifest_path)
        }
        ManifestRootLocator::ExactConeManifestFile(manifest) => {
            if !manifest
                .file_name()
                .is_some_and(|name| name == MANIFEST_FILE_NAME)
            {
                return Err(ManifestRootError::new(
                    manifest.clone(),
                    ManifestRootErrorKind::InvalidManifestFileName,
                ));
            }
            let manifest_path = canonicalize(manifest)?;
            let real_root = manifest_path.parent().ok_or_else(|| {
                ManifestRootError::new(manifest.clone(), ManifestRootErrorKind::RootNotDirectory)
            })?;
            let real_root = canonicalize(real_root)?;
            (real_root, manifest_path)
        }
    };

    let metadata = std::fs::metadata(&manifest_path).map_err(|error| {
        ManifestRootError::io(
            ManifestRootIoOperation::Inspect,
            manifest_path.clone(),
            error,
        )
    })?;
    if !metadata.is_file() {
        return Err(ManifestRootError::new(
            manifest_path,
            ManifestRootErrorKind::ManifestNotRegularFile,
        ));
    }

    let bytes = std::fs::read(&manifest_path).map_err(|error| {
        ManifestRootError::io(ManifestRootIoOperation::Read, manifest_path.clone(), error)
    })?;
    let source = String::from_utf8(bytes).map_err(|_| {
        ManifestRootError::new(manifest_path.clone(), ManifestRootErrorKind::InvalidUtf8)
    })?;
    let parsed = parse_cone_manifest(&source).map_err(|error| {
        ManifestRootError::new(manifest_path.clone(), ManifestRootErrorKind::Parse(error))
    })?;
    let identity = parsed.semantic().coordinate().identity().map_err(|error| {
        ManifestRootError::new(
            manifest_path.clone(),
            ManifestRootErrorKind::Identity(error),
        )
    })?;

    Ok(LoadedConeManifest {
        real_root,
        manifest_path,
        source,
        identity,
        parsed,
    })
}

fn canonicalize(path: &Path) -> Result<PathBuf, ManifestRootError> {
    std::fs::canonicalize(path).map_err(|error| {
        ManifestRootError::io(
            ManifestRootIoOperation::Canonicalize,
            path.to_path_buf(),
            error,
        )
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "scoop-manifest-root-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn manifest(&self) -> PathBuf {
            self.0.join(MANIFEST_FILE_NAME)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_manifest(path: &Path) {
        std::fs::write(
            path,
            "schema = 1\n[cone]\ngroup = \"dev.example\"\nname = \"app\"\nversion = \"1.0.0\"\nkind = \"executable\"\n",
        )
        .unwrap();
    }

    #[test]
    fn loads_directory_and_exact_file_as_the_same_root() {
        let directory = TempDirectory::new();
        write_manifest(&directory.manifest());

        let from_directory =
            load_cone_manifest(&ManifestRootLocator::cone_directory(&directory.0)).unwrap();
        let from_file = load_cone_manifest(&ManifestRootLocator::exact_manifest_file(
            directory.manifest(),
        ))
        .unwrap();

        assert_eq!(from_directory.real_root(), from_file.real_root());
        assert_eq!(from_directory.manifest_path(), from_file.manifest_path());
        assert_eq!(
            from_directory.parsed().semantic(),
            from_file.parsed().semantic()
        );
        assert_eq!(from_directory.source_bytes(), from_file.source_bytes());
        assert_eq!(
            from_directory.source_bytes(),
            std::fs::read(directory.manifest()).unwrap()
        );
        assert_eq!(
            from_directory.source_text().as_bytes(),
            from_directory.source_bytes()
        );
        assert_eq!(from_directory.coordinate(), from_file.coordinate());
        assert_eq!(
            from_directory.identity(),
            from_directory.coordinate().identity().unwrap()
        );
    }

    #[test]
    fn rejects_a_regular_file_with_another_name() {
        let directory = TempDirectory::new();
        let path = directory.0.join("manifest.toml");
        write_manifest(&path);

        assert!(matches!(
            ManifestRootLocator::from_path(path).unwrap_err().kind(),
            ManifestRootErrorKind::InvalidManifestFileName
        ));
    }

    #[test]
    fn ordinary_loader_accepts_core_from_a_user_source_directory() {
        let directory = TempDirectory::new();
        std::fs::write(
            directory.manifest(),
            "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
        )
        .unwrap();
        let locator = ManifestRootLocator::cone_directory(&directory.0);

        let ordinary = load_cone_manifest(&locator).unwrap();
        assert_eq!(
            ordinary.parsed().semantic().coordinate(),
            &scoop_identity::ConeCoordinate::reserved_core()
        );
    }
}
