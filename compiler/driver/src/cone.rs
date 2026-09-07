//! Source discovery for the current Cone (DESIGN section 1.3).
//!
//! `scoopc` collects exactly the regular `src/**/*.scoop` files of the
//! current Cone, normalized and sorted by their Cone-relative UTF-8
//! bytes. Host absolute paths, directory enumeration order, inodes and
//! mtimes never enter any semantic identity: a file's identity is
//! `(ConeIdentity, normalized relative path)`.

use core::fmt;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use scoop_manifest::ConeManifest;

/// One discovered source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeSourceFile {
    /// Normalized Cone-relative path (`src/` excluded), `/`-separated.
    relative_path: String,
    text: String,
}

impl ConeSourceFile {
    pub fn new(relative_path: String, text: String) -> Result<Self, ConeInputError> {
        validate_relative_path(&relative_path)?;
        Ok(ConeSourceFile {
            relative_path,
            text,
        })
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// The typed input for one compilation: coordinate, kind and the sorted
/// source set. Fixture runners may construct this directly with stable
/// test coordinates; the constructor is deliberately absent from the
/// CLI, environment variables and `.slib` content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConeInput {
    manifest: ConeManifest,
    sources: Vec<ConeSourceFile>,
}

impl ResolvedConeInput {
    /// Builds an input from already-discovered sources, enforcing the
    /// same ordering/uniqueness rules as filesystem discovery.
    pub fn from_parts(
        manifest: ConeManifest,
        mut sources: Vec<ConeSourceFile>,
    ) -> Result<Self, ConeInputError> {
        sources.sort_by(|left, right| {
            left.relative_path
                .as_bytes()
                .cmp(right.relative_path.as_bytes())
        });
        let mut seen = BTreeSet::new();
        for source in &sources {
            if !seen.insert(source.relative_path.clone()) {
                return Err(ConeInputError::DuplicatePath(source.relative_path.clone()));
            }
        }
        Ok(ResolvedConeInput { manifest, sources })
    }

    pub fn manifest(&self) -> &ConeManifest {
        &self.manifest
    }

    pub fn sources(&self) -> &[ConeSourceFile] {
        &self.sources
    }
}

/// Source-discovery failures. All are single-Cone compiler errors,
/// reported with the manifest path by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConeInputError {
    MissingSourceRoot,
    MissingManifest,
    Io(String),
    NonUtf8Path(String),
    SymlinkInSources(String),
    NonRegularSource(String),
    DuplicatePath(String),
    InvalidRelativePath(String),
    EmptySourceSet,
}

impl fmt::Display for ConeInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConeInputError::MissingSourceRoot => {
                write!(f, "the Cone has no `src/` directory")
            }
            ConeInputError::MissingManifest => {
                write!(f, "the Cone root has no Cone.toml")
            }
            ConeInputError::Io(message) => write!(f, "source discovery failed: {message}"),
            ConeInputError::NonUtf8Path(path) => {
                write!(f, "source path {path:?} is not valid UTF-8")
            }
            ConeInputError::SymlinkInSources(path) => write!(
                f,
                "symlink {path:?} inside `src/` cannot enter the compilation unit"
            ),
            ConeInputError::NonRegularSource(path) => {
                write!(f, "{path:?} is not a regular .scoop source file")
            }
            ConeInputError::DuplicatePath(path) => write!(
                f,
                "two source paths normalize to the same identity {path:?}"
            ),
            ConeInputError::InvalidRelativePath(path) => {
                write!(
                    f,
                    "source path {path:?} is not a normalized Cone-relative .scoop path"
                )
            }
            ConeInputError::EmptySourceSet => {
                write!(f, "the Cone declares no .scoop sources under src/")
            }
        }
    }
}

impl std::error::Error for ConeInputError {}

fn validate_relative_path(path: &str) -> Result<(), ConeInputError> {
    if path.is_empty() || path.starts_with('/') || path.ends_with('/') {
        return Err(ConeInputError::InvalidRelativePath(path.to_owned()));
    }
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(ConeInputError::InvalidRelativePath(path.to_owned()));
        }
    }
    if !path.ends_with(".scoop") || path.ends_with("/.scoop") || path == ".scoop" {
        return Err(ConeInputError::InvalidRelativePath(path.to_owned()));
    }
    Ok(())
}

/// Reads `Cone.toml` and discovers `src/**/*.scoop` under `cone_root`.
pub fn discover_cone(cone_root: &Path) -> Result<ResolvedConeInput, ConeInputError> {
    let manifest_path = cone_root.join("Cone.toml");
    if !manifest_path.is_file() {
        return Err(ConeInputError::MissingManifest);
    }
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .map_err(|error| ConeInputError::Io(format!("cannot read Cone.toml: {error}")))?;
    let manifest = scoop_manifest::parse_manifest(&manifest_text)
        .map_err(|errors| ConeInputError::Io(format!("invalid Cone.toml: {errors:#?}")))?;
    let src_root = cone_root.join("src");
    if !src_root.is_dir() {
        return Err(ConeInputError::MissingSourceRoot);
    }
    let mut files = Vec::new();
    walk(&src_root, &mut Vec::new(), &mut files)?;
    if files.is_empty() {
        return Err(ConeInputError::EmptySourceSet);
    }
    let sources = files
        .into_iter()
        .map(|(relative, absolute)| {
            let text = std::fs::read_to_string(&absolute).map_err(|error| {
                ConeInputError::Io(format!("cannot read {}: {error}", relative))
            })?;
            ConeSourceFile::new(relative, text)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ResolvedConeInput::from_parts(manifest, sources)
}

/// Depth-first walk collecting `.scoop` regular files with normalized
/// relative paths. Symlinks and non-regular entries are hard errors so
/// that no host path trickery can smuggle files in or out.
fn walk(
    directory: &Path,
    prefix: &mut Vec<String>,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), ConeInputError> {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .map_err(|error| {
            ConeInputError::Io(format!("cannot list {}: {error}", directory.display()))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ConeInputError::Io(format!("cannot list {}: {error}", directory.display()))
        })?;
    // Deterministic regardless of filesystem enumeration order.
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| ConeInputError::NonUtf8Path(name.to_string_lossy().into_owned()))?;
        let path = entry.path();
        // `entry.metadata` follows symlinks on some platforms; use
        // symlink metadata and reject symlinks explicitly.
        let file_type = std::fs::symlink_metadata(&path)
            .map_err(|error| ConeInputError::Io(format!("cannot lstat {name:?}: {error}")))?
            .file_type();
        if file_type.is_symlink() {
            return Err(ConeInputError::SymlinkInSources(name));
        }
        if file_type.is_dir() {
            prefix.push(name.clone());
            walk(&path, prefix, out)?;
            prefix.pop();
        } else if name.ends_with(".scoop") && name != ".scoop" {
            if !file_type.is_file() {
                return Err(ConeInputError::NonRegularSource(name));
            }
            let mut relative = prefix.join("/");
            if !relative.is_empty() {
                relative.push('/');
            }
            relative.push_str(&name);
            out.push((relative, path));
        }
        // Non-`.scoop` files (including Cone-internal helpers) are
        // ignored: only exact `.scoop` regular files enter the unit.
    }
    Ok(())
}
