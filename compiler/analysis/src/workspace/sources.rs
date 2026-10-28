use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use scoop_identity::{ConeIdentity, NormalizedSourcePath};
use scoop_manifest::{LoadedConeManifest, SelectedManifestSources};
use scoop_protocol::package::PackageMemberV1;

use super::{LoadError, ProjectConfiguration, ProjectSource, SourceConeInput, files::FileInputs};
use crate::AnalysisHost;

impl FileInputs {
    pub(super) fn sources(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        manifest: Arc<LoadedConeManifest>,
        configuration: &ProjectConfiguration,
        include_tests: bool,
    ) -> Result<SourceConeInput, LoadError> {
        let target = configuration.target.target().id();
        let selected = if include_tests {
            scoop_manifest::select_manifest_sources_for_tests(&manifest, target)
        } else {
            scoop_manifest::select_manifest_sources(&manifest, target)
        }
        .map_err(|error| LoadError::new(manifest.manifest_path(), error))?;
        let sources = self.selected_sources(
            host,
            overlays,
            manifest.real_root(),
            manifest.identity(),
            selected,
        )?;
        let test_dependencies = manifest
            .parsed()
            .tests()
            .dependency_iter()
            .filter(|_| include_tests)
            .map(|(_, dependency)| {
                dependency
                    .coordinate()
                    .identity()
                    .map_err(|error| LoadError::new(manifest.manifest_path(), error))
            })
            .collect::<Result<_, _>>()?;
        Ok(SourceConeInput {
            manifest,
            sources,
            test_dependencies,
        })
    }

    pub(super) fn member_sources(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        member: &PackageMemberV1,
        configuration: &ProjectConfiguration,
        include_tests: bool,
    ) -> Result<Vec<ProjectSource>, LoadError> {
        let root = member
            .directory
            .to_path_buf()
            .map_err(|error| LoadError::new(&configuration.root, error))?;
        let identity = member
            .coordinate
            .identity()
            .map_err(|error| LoadError::new(&root, error))?;
        let selected = scoop_manifest::select_cone_sources(
            &root,
            identity,
            &member.sources,
            include_tests.then_some(&member.test_sources),
            configuration.target.target().id(),
        )
        .map_err(|error| LoadError::new(root.join("Cone.toml"), error))?;
        self.selected_sources(host, overlays, &root, identity, selected)
    }

    fn selected_sources(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        root: &Path,
        identity: ConeIdentity,
        selected: SelectedManifestSources,
    ) -> Result<Vec<ProjectSource>, LoadError> {
        let mut sources = Vec::new();
        for source in selected.iter() {
            let path = source.physical_path();
            let (id, _) = self.read(host, overlays, path)?;
            sources.push(ProjectSource {
                source: id,
                path: path.to_owned(),
                identity: source.identity().clone(),
                role: source.role(),
            });
        }
        for path in overlays {
            if path
                .extension()
                .is_none_or(|extension| extension != "scoop")
                || sources.iter().any(|source| source.path == *path)
            {
                continue;
            }
            let Some(logical) = path.strip_prefix(root).ok().and_then(|path| path.to_str()) else {
                continue;
            };
            let logical =
                NormalizedSourcePath::new(logical).map_err(|error| LoadError::new(path, error))?;
            let Some(role) = selected.role_for_path(&logical) else {
                continue;
            };
            let (source, _) = self.read(host, overlays, path)?;
            sources.push(ProjectSource {
                source,
                identity: scoop_identity::SourceIdentity::new(identity, logical),
                path: path.clone(),
                role,
            });
        }
        sources.sort_by(|left, right| {
            left.identity
                .logical_path()
                .cmp(right.identity.logical_path())
        });
        Ok(sources)
    }
}
