use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use scoop_manifest::ManifestRootLocator;

use super::{
    CurrentConeInput, ExplicitDependencyInputs, SingleConeBuildRequestError, TrustedCoreInput,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlibOutputDestination {
    path: PathBuf,
}

impl SlibOutputDestination {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, SingleConeBuildRequestError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(SingleConeBuildRequestError::EmptyOutputDestination);
        }
        if path.extension() != Some(OsStr::new("slib")) {
            return Err(SingleConeBuildRequestError::InvalidOutputExtension { path });
        }
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(SingleConeBuildRequestError::CurrentDirectory)?
                .join(path)
        };
        Ok(Self { path })
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputAliasRole {
    CurrentManifest,
    CurrentSource,
    TrustedCoreArtifact,
    DirectArtifact { index: usize },
    SupportArtifact { index: usize },
}

impl fmt::Display for OutputAliasRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentManifest => formatter.write_str("current Cone manifest"),
            Self::CurrentSource => formatter.write_str("current single-file source"),
            Self::TrustedCoreArtifact => formatter.write_str("trusted core artifact"),
            Self::DirectArtifact { index } => write!(formatter, "direct artifact {index}"),
            Self::SupportArtifact { index } => write!(formatter, "support artifact {index}"),
        }
    }
}

#[derive(Debug)]
pub enum OutputIsolationErrorKind {
    InspectOutput(std::io::Error),
    CanonicalizeOutput(std::io::Error),
    CanonicalizeOutputParent(std::io::Error),
    OutputNotRegularFile,
    MissingOutputFileName,
    CanonicalizeInput {
        role: OutputAliasRole,
        source: std::io::Error,
    },
    AliasesInput {
        role: OutputAliasRole,
        input: PathBuf,
    },
}

impl fmt::Display for OutputIsolationErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectOutput(error) => write!(formatter, "cannot inspect output: {error}"),
            Self::CanonicalizeOutput(error) => {
                write!(formatter, "cannot canonicalize existing output: {error}")
            }
            Self::CanonicalizeOutputParent(error) => {
                write!(formatter, "cannot canonicalize output parent: {error}")
            }
            Self::OutputNotRegularFile => {
                formatter.write_str("existing output is not a regular file")
            }
            Self::MissingOutputFileName => formatter.write_str("output does not have a file name"),
            Self::CanonicalizeInput { role, source } => {
                write!(formatter, "cannot canonicalize {role}: {source}")
            }
            Self::AliasesInput { role, input } => {
                write!(formatter, "aliases {role} {}", input.display())
            }
        }
    }
}

impl std::error::Error for OutputIsolationErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InspectOutput(error)
            | Self::CanonicalizeOutput(error)
            | Self::CanonicalizeOutputParent(error) => Some(error),
            Self::CanonicalizeInput { source, .. } => Some(source),
            Self::OutputNotRegularFile
            | Self::MissingOutputFileName
            | Self::AliasesInput { .. } => None,
        }
    }
}

pub(super) fn validate_output_isolation(
    current: &CurrentConeInput,
    dependencies: &ExplicitDependencyInputs,
    trusted_core: &TrustedCoreInput,
    output: &SlibOutputDestination,
) -> Result<(), SingleConeBuildRequestError> {
    let output_path = canonical_output_candidate(output.as_path()).map_err(|kind| {
        SingleConeBuildRequestError::OutputIsolation {
            path: output.as_path().to_path_buf(),
            kind,
        }
    })?;
    let mut inputs =
        Vec::with_capacity(2 + dependencies.direct().len() + dependencies.support().len());
    match current {
        CurrentConeInput::Manifest { root } => {
            let path = match root {
                ManifestRootLocator::ConeDirectory(path) => path.join("Cone.toml"),
                ManifestRootLocator::ExactConeManifestFile(path) => path.clone(),
            };
            inputs.push((OutputAliasRole::CurrentManifest, path));
        }
        CurrentConeInput::SingleFile { source } => inputs.push((
            OutputAliasRole::CurrentSource,
            source.resolved_path().to_path_buf(),
        )),
    }
    if let TrustedCoreInput::Artifact(core) = trusted_core {
        inputs.push((
            OutputAliasRole::TrustedCoreArtifact,
            core.as_path().to_path_buf(),
        ));
    }
    inputs.extend(
        dependencies
            .direct()
            .iter()
            .enumerate()
            .map(|(index, input)| {
                (
                    OutputAliasRole::DirectArtifact { index },
                    input.as_path().to_path_buf(),
                )
            }),
    );
    inputs.extend(
        dependencies
            .support()
            .iter()
            .enumerate()
            .map(|(index, input)| {
                (
                    OutputAliasRole::SupportArtifact { index },
                    input.as_path().to_path_buf(),
                )
            }),
    );

    for (role, input) in inputs {
        let canonical = std::fs::canonicalize(&input).map_err(|source| {
            SingleConeBuildRequestError::OutputIsolation {
                path: output.as_path().to_path_buf(),
                kind: OutputIsolationErrorKind::CanonicalizeInput {
                    role: role.clone(),
                    source,
                },
            }
        })?;
        if canonical == output_path {
            return Err(SingleConeBuildRequestError::OutputIsolation {
                path: output.as_path().to_path_buf(),
                kind: OutputIsolationErrorKind::AliasesInput {
                    role,
                    input: canonical,
                },
            });
        }
    }
    Ok(())
}

fn canonical_output_candidate(output: &Path) -> Result<PathBuf, OutputIsolationErrorKind> {
    match std::fs::metadata(output) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(OutputIsolationErrorKind::OutputNotRegularFile);
            }
            std::fs::canonicalize(output).map_err(OutputIsolationErrorKind::CanonicalizeOutput)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = output
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .ok_or(OutputIsolationErrorKind::MissingOutputFileName)?;
            let file_name = output
                .file_name()
                .ok_or(OutputIsolationErrorKind::MissingOutputFileName)?;
            std::fs::canonicalize(parent)
                .map(|parent| parent.join(file_name))
                .map_err(OutputIsolationErrorKind::CanonicalizeOutputParent)
        }
        Err(error) => Err(OutputIsolationErrorKind::InspectOutput(error)),
    }
}
