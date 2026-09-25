use std::fs::File;
use std::io::Read;

use scoop_wire::DecodeLimits;

use super::{
    ExplicitDependencyArtifactInput, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, LoadedExplicitDependencyArtifact, LoadedExplicitDependencyInputs,
};
use crate::HostArtifactLocator;

impl LoadedExplicitDependencyInputs {
    pub(crate) fn load(
        direct: &[HostArtifactLocator],
        support: &[HostArtifactLocator],
        limits: DecodeLimits,
    ) -> Result<Self, ExplicitDependencyLoadError> {
        let mut artifacts = Vec::with_capacity(direct.len() + support.len());
        for (role, locators) in [
            (ExplicitDependencyRole::Direct, direct),
            (ExplicitDependencyRole::Support, support),
        ] {
            for (index, locator) in locators.iter().enumerate() {
                let input = ExplicitDependencyArtifactInput {
                    role,
                    index,
                    path: locator.as_path().to_path_buf(),
                };
                artifacts.push(LoadedExplicitDependencyArtifact::load(input, limits)?);
            }
        }
        Ok(Self { artifacts, limits })
    }

    pub(in crate::request::preflight) fn append_direct(
        &mut self,
        locator: &HostArtifactLocator,
    ) -> Result<(), ExplicitDependencyLoadError> {
        let index = self
            .artifacts
            .iter()
            .filter(|artifact| artifact.input.role == ExplicitDependencyRole::Direct)
            .count();
        let input = ExplicitDependencyArtifactInput {
            role: ExplicitDependencyRole::Direct,
            index,
            path: locator.as_path().to_path_buf(),
        };
        let artifact = LoadedExplicitDependencyArtifact::load(input, self.limits)?;
        self.artifacts.insert(index, artifact);
        Ok(())
    }
}

impl LoadedExplicitDependencyArtifact {
    fn load(
        input: ExplicitDependencyArtifactInput,
        limits: DecodeLimits,
    ) -> Result<Self, ExplicitDependencyLoadError> {
        let bytes = load_artifact_bytes(&input, limits)?;

        Ok(Self {
            input,
            bytes,
            summary: std::cell::OnceCell::new(),
        })
    }
}

fn load_artifact_bytes(
    input: &ExplicitDependencyArtifactInput,
    limits: DecodeLimits,
) -> Result<Vec<u8>, ExplicitDependencyLoadError> {
    let file = File::open(input.path()).map_err(|source| ExplicitDependencyLoadError::Io {
        input: input.clone(),
        operation: ExplicitDependencyLoadOperation::Open,
        source,
    })?;
    let metadata = file
        .metadata()
        .map_err(|source| ExplicitDependencyLoadError::Io {
            input: input.clone(),
            operation: ExplicitDependencyLoadOperation::Inspect,
            source,
        })?;
    if !metadata.is_file() {
        return Err(ExplicitDependencyLoadError::NotRegularFile(input.clone()));
    }
    require_size(input, metadata.len(), limits.owned_bytes)?;

    let mut bytes = Vec::new();
    file.take(limits.owned_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| ExplicitDependencyLoadError::Io {
            input: input.clone(),
            operation: ExplicitDependencyLoadOperation::Read,
            source,
        })?;
    require_size(
        input,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        limits.owned_bytes,
    )?;
    Ok(bytes)
}

fn require_size(
    input: &ExplicitDependencyArtifactInput,
    actual: u64,
    limit: u64,
) -> Result<(), ExplicitDependencyLoadError> {
    if actual > limit {
        Err(ExplicitDependencyLoadError::ArtifactTooLarge {
            input: input.clone(),
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
