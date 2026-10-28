use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use scoop_protocol::package::{PreparedConeV1, PreparedResolutionV1};

use super::{
    ConeInput, LoadError, ProjectConfiguration, ProjectInput, SourceConeInput, WorkspaceLoader,
};
use crate::AnalysisHost;

mod order;

pub(super) struct ProjectBuilder {
    project: ProjectInput,
    manifest: PathBuf,
}

impl ProjectBuilder {
    pub(super) fn load(
        loader: &mut WorkspaceLoader,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        configuration: ProjectConfiguration,
        resolution: PreparedResolutionV1,
    ) -> Result<(ProjectInput, Vec<LoadError>), LoadError> {
        let manifest = configuration.root.join("Cone.toml");
        if resolution.target != configuration.target.target().contract().canonical_triple()
            || resolution.purpose != configuration.purpose
        {
            return Err(LoadError::new(
                &manifest,
                "prepared target or purpose differs from the request",
            ));
        }
        let root = resolution
            .coordinate
            .identity()
            .map_err(|error| LoadError::new(&manifest, error))?;
        let mut builder = Self {
            manifest,
            project: ProjectInput {
                configuration,
                root,
                cones: BTreeMap::new(),
                artifact_sources: BTreeMap::new(),
                dependencies: BTreeMap::new(),
                dependency_order: Vec::new(),
                complete: false,
            },
        };
        let mut errors = Vec::new();
        for node in resolution.nodes {
            let identity = node
                .cone
                .coordinate()
                .identity()
                .map_err(|error| LoadError::new(&builder.manifest, error))?;
            let dependencies = node
                .dependencies
                .into_iter()
                .chain(node.test_dependencies)
                .map(|coordinate| {
                    coordinate
                        .identity()
                        .map_err(|error| LoadError::new(&builder.manifest, error))
                })
                .collect::<Result<_, _>>()?;
            let input = match node.cone {
                PreparedConeV1::Source { config } => {
                    let manifest = loader.files.expanded(
                        host,
                        &builder.project.configuration.name,
                        root,
                        *config,
                    )?;
                    let include_tests = identity == root
                        && builder.project.configuration.purpose
                            == scoop_protocol::CompilationPurpose::Test;
                    let source = match loader.files.sources(
                        host,
                        overlays,
                        manifest.clone(),
                        &builder.project.configuration,
                        include_tests,
                    ) {
                        Ok(source) => source,
                        Err(error) => {
                            errors.push(error);
                            SourceConeInput {
                                manifest,
                                sources: Vec::new(),
                                test_dependencies: Vec::new(),
                            }
                        }
                    };
                    ConeInput::Source(source)
                }
                PreparedConeV1::Prebuilt { artifact, summary } => {
                    let path = artifact
                        .to_path_buf()
                        .map_err(|error| LoadError::new(&builder.manifest, error))?;
                    ConeInput::Artifact(loader.artifacts.read(
                        &path,
                        summary.cone().coordinate(),
                        builder.project.configuration.target,
                    )?)
                }
            };
            builder.project.cones.insert(identity, input);
            builder.project.dependencies.insert(identity, dependencies);
        }
        builder.order()?;
        builder.project.complete = errors.is_empty();
        errors.extend(loader.attach_sources(host, overlays, &mut builder.project));
        Ok((builder.project, errors))
    }
}
