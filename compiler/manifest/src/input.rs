//! Shared classification of a current Cone directory, manifest, or source file.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::{ManifestRootLocator, SingleFileInputError, SingleFileLocator};

#[derive(Debug)]
pub enum CurrentConeInput {
    Manifest { root: ManifestRootLocator },
    SingleFile { source: SingleFileLocator },
}

pub fn classify_current_cone_operand(
    path: impl Into<PathBuf>,
) -> Result<CurrentConeInput, CurrentConeOperandError> {
    let path = path.into();
    let metadata = std::fs::metadata(&path).map_err(|error| {
        CurrentConeOperandError::new(path.clone(), CurrentConeOperandErrorKind::Inspect(error))
    })?;
    if metadata.is_dir() {
        return Ok(CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(path),
        });
    }
    if !metadata.is_file() {
        return Err(CurrentConeOperandError::new(
            path,
            CurrentConeOperandErrorKind::UnsupportedFileType,
        ));
    }
    if path.file_name().is_some_and(|name| name == "Cone.toml") {
        return Ok(CurrentConeInput::Manifest {
            root: ManifestRootLocator::exact_manifest_file(path),
        });
    }
    if path
        .extension()
        .is_some_and(|extension| extension == "scoop")
    {
        return SingleFileLocator::from_path(&path)
            .map(|source| CurrentConeInput::SingleFile { source })
            .map_err(|error| {
                CurrentConeOperandError::new(path, CurrentConeOperandErrorKind::SingleFile(error))
            });
    }
    Err(CurrentConeOperandError::new(
        path,
        CurrentConeOperandErrorKind::UnsupportedFile,
    ))
}

#[derive(Debug)]
pub struct CurrentConeOperandError {
    path: PathBuf,
    kind: CurrentConeOperandErrorKind,
}

impl CurrentConeOperandError {
    fn new(path: PathBuf, kind: CurrentConeOperandErrorKind) -> Self {
        Self { path, kind }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> &CurrentConeOperandErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum CurrentConeOperandErrorKind {
    Inspect(std::io::Error),
    UnsupportedFile,
    UnsupportedFileType,
    SingleFile(SingleFileInputError),
}

impl fmt::Display for CurrentConeOperandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid current Cone operand {}: ",
            self.path.display()
        )?;
        match &self.kind {
            CurrentConeOperandErrorKind::Inspect(error) => {
                write!(formatter, "cannot inspect: {error}")
            }
            CurrentConeOperandErrorKind::UnsupportedFile => formatter
                .write_str("regular file must be named Cone.toml or have exact .scoop extension"),
            CurrentConeOperandErrorKind::UnsupportedFileType => {
                formatter.write_str("operand must resolve to a directory or regular file")
            }
            CurrentConeOperandErrorKind::SingleFile(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeOperandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            CurrentConeOperandErrorKind::Inspect(error) => Some(error),
            CurrentConeOperandErrorKind::SingleFile(error) => Some(error),
            CurrentConeOperandErrorKind::UnsupportedFile
            | CurrentConeOperandErrorKind::UnsupportedFileType => None,
        }
    }
}
