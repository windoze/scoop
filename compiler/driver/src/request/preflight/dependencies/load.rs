use scoop_lir::ValidatedLirTargetSelection;

use super::{
    ExplicitDependencyArtifactInput, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, LoadedExplicitDependencyArtifact, LoadedExplicitDependencyInputs,
};
use crate::HostArtifactLocator;

impl LoadedExplicitDependencyInputs {
    pub(crate) fn load(
        direct: &[HostArtifactLocator],
        support: &[HostArtifactLocator],
        target: ValidatedLirTargetSelection,
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
                artifacts.push(LoadedExplicitDependencyArtifact::load(input, target)?);
            }
        }
        Ok(Self { artifacts })
    }

    pub(in crate::request::preflight) fn append_direct(
        &mut self,
        locator: &HostArtifactLocator,
        target: ValidatedLirTargetSelection,
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
        let artifact = LoadedExplicitDependencyArtifact::load(input, target)?;
        self.artifacts.insert(index, artifact);
        Ok(())
    }
}

impl LoadedExplicitDependencyArtifact {
    fn load(
        input: ExplicitDependencyArtifactInput,
        target: ValidatedLirTargetSelection,
    ) -> Result<Self, ExplicitDependencyLoadError> {
        let artifact = scoop_slib::ArtifactFile::open(input.path(), target).map_err(|source| {
            ExplicitDependencyLoadError::Io {
                input: input.clone(),
                operation: ExplicitDependencyLoadOperation::Open,
                source,
            }
        })?;
        Ok(Self { input, artifact })
    }
}

#[cfg(test)]
mod tests;
