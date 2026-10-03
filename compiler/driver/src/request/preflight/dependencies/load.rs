use std::fs::File;
use std::io::Read;

use super::{
    ExplicitDependencyArtifactInput, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, LoadedExplicitDependencyArtifact, LoadedExplicitDependencyInputs,
};
use crate::HostArtifactLocator;

impl LoadedExplicitDependencyInputs {
    pub(crate) fn load(
        direct: &[HostArtifactLocator],
        support: &[HostArtifactLocator],
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
                artifacts.push(LoadedExplicitDependencyArtifact::load(input)?);
            }
        }
        Ok(Self { artifacts })
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
        let artifact = LoadedExplicitDependencyArtifact::load(input)?;
        self.artifacts.insert(index, artifact);
        Ok(())
    }
}

impl LoadedExplicitDependencyArtifact {
    fn load(input: ExplicitDependencyArtifactInput) -> Result<Self, ExplicitDependencyLoadError> {
        let bytes = load_artifact_bytes(&input)?;

        Ok(Self {
            input,
            bytes,
            summary: std::cell::OnceCell::new(),
        })
    }
}

fn load_artifact_bytes(
    input: &ExplicitDependencyArtifactInput,
) -> Result<Vec<u8>, ExplicitDependencyLoadError> {
    let mut file = File::open(input.path()).map_err(|source| ExplicitDependencyLoadError::Io {
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
    let length = metadata.len();
    let capacity = usize::try_from(length)
        .map_err(|_| ExplicitDependencyLoadError::LengthOverflow(input.clone()))?;
    let read_bound = length
        .checked_add(1)
        .ok_or_else(|| ExplicitDependencyLoadError::LengthOverflow(input.clone()))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| ExplicitDependencyLoadError::Allocation(input.clone()))?;
    file.by_ref()
        .take(read_bound)
        .read_to_end(&mut bytes)
        .map_err(|source| ExplicitDependencyLoadError::Io {
            input: input.clone(),
            operation: ExplicitDependencyLoadOperation::Read,
            source,
        })?;
    let after = file
        .metadata()
        .map_err(|source| ExplicitDependencyLoadError::Io {
            input: input.clone(),
            operation: ExplicitDependencyLoadOperation::Inspect,
            source,
        })?;
    if bytes.len() != capacity
        || after.len() != length
        || after.modified().ok() != metadata.modified().ok()
    {
        return Err(ExplicitDependencyLoadError::ChangedDuringRead(
            input.clone(),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
