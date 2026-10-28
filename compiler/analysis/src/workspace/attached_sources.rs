//! Installed library sources supply navigation without replacing artifact semantics.

use std::{collections::BTreeSet, path::PathBuf};

use super::{
    ArtifactConeInput, ConeInput, LoadError, ProjectConfiguration, ProjectInput, ProjectSource,
    WorkspaceLoader, normalize_path,
};
use crate::AnalysisHost;

impl WorkspaceLoader {
    pub(super) fn attach_sources(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        project: &mut ProjectInput,
    ) -> Vec<LoadError> {
        let mut errors = Vec::new();
        for cone in project.cones.values() {
            let ConeInput::Artifact(artifact) = cone else {
                continue;
            };
            match self.installed_sources(host, overlays, &project.configuration, artifact) {
                Ok(sources) => project.artifact_sources.extend(
                    sources
                        .into_iter()
                        .map(|source| (source.identity.clone(), source)),
                ),
                Err(error) => errors.push(error),
            }
        }
        errors
    }

    fn installed_sources(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        configuration: &ProjectConfiguration,
        artifact: &ArtifactConeInput,
    ) -> Result<Vec<ProjectSource>, LoadError> {
        let coordinate = artifact.summary.cone().coordinate();
        if coordinate.group() != "scoop" {
            return Ok(Vec::new());
        }
        let installed = scoop_toolchain::sysroot_library_path(
            &configuration.sysroot,
            configuration.target.target(),
            coordinate,
        );
        if normalize_path(&installed).map_err(|error| LoadError::new(&installed, error))?
            != artifact.path
        {
            return Ok(Vec::new());
        }
        let root = configuration.sysroot.join("src").join(coordinate.name());
        let path = root.join("Cone.toml");
        if !path
            .try_exists()
            .map_err(|error| LoadError::new(&path, error))?
        {
            return Ok(Vec::new());
        }
        let members =
            super::package::PackageInputs::new(configuration, host, overlays)?.members(&root)?;
        let Some(member) = members
            .members
            .iter()
            .find(|member| &member.coordinate == coordinate)
        else {
            return Ok(Vec::new());
        };
        self.files
            .member_sources(host, overlays, member, configuration, false)
    }
}
