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
                let bytes = load_artifact_bytes(&input, limits)?;
                artifacts.push(LoadedExplicitDependencyArtifact { input, bytes });
            }
        }
        Ok(Self { artifacts, limits })
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
mod tests {
    use super::*;

    #[test]
    fn loader_requires_a_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let locator = HostArtifactLocator::new(directory.path()).unwrap();

        assert!(matches!(
            LoadedExplicitDependencyInputs::load(
                &[locator],
                &[],
                DecodeLimits::default(),
            ),
            Err(ExplicitDependencyLoadError::NotRegularFile(input))
                if input.role() == ExplicitDependencyRole::Direct
                    && input.index() == 0
                    && input.path() == directory.path()
        ));
    }

    #[test]
    fn loader_enforces_the_owned_byte_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dependency.slib");
        std::fs::write(&path, b"too large").unwrap();
        let locator = HostArtifactLocator::new(&path).unwrap();
        let limits = DecodeLimits {
            owned_bytes: 3,
            ..DecodeLimits::default()
        };

        assert!(matches!(
            LoadedExplicitDependencyInputs::load(&[locator], &[], limits),
            Err(ExplicitDependencyLoadError::ArtifactTooLarge {
                input,
                actual: 9,
                limit: 3,
            }) if input.path() == path
        ));
    }
}
