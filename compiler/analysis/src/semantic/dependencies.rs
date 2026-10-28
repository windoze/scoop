use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use scoop_hir::{self as hir, OwnedImportedProvider};
use scoop_hir_lower::CoreProtocolInput;
use scoop_identity::{ConeIdentity, SemanticIdentitySession};

use super::{ProjectAnalysisError as Error, sources};
use crate::{AnalysisSnapshot, ConeInput, ProjectInput};

pub(super) struct ArtifactInputs {
    providers: Vec<OwnedImportedProvider>,
    identities: SemanticIdentitySession,
}

pub(super) struct DependencyInputs {
    pub providers: Vec<OwnedImportedProvider>,
    pub documentation: BTreeMap<ConeIdentity, Arc<hir::SemanticDocumentation>>,
}

impl ArtifactInputs {
    pub fn load(project: &ProjectInput, check: &Rc<dyn Fn() -> bool>) -> Result<Self, Error> {
        if check() {
            return Err(Error::Cancelled);
        }
        if !project.complete {
            return Err(Error::ConfigurationUnavailable);
        }
        let artifacts = project
            .dependency_order
            .iter()
            .filter_map(|cone| match &project.cones[cone] {
                ConeInput::Artifact(artifact) => Some(&artifact.file),
                ConeInput::Source(_) => None,
            })
            .collect();
        let closure = scoop_slib::CompileDependencyClosure::read_hir(
            project.root,
            project.configuration.target,
            project.dependencies[&project.root].clone(),
            artifacts,
        )
        .map_err(|error| Error::Artifact(error.to_string()))?;
        if check() {
            return Err(Error::Cancelled);
        }
        let (providers, identities) = closure.into_semantic_inputs();
        Ok(Self {
            providers,
            identities,
        })
    }

    pub fn with_sources(
        &self,
        snapshot: &AnalysisSnapshot,
        project: &ProjectInput,
        reachable: &BTreeSet<ConeIdentity>,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<DependencyInputs, Error> {
        let mut providers = self
            .providers
            .iter()
            .filter(|provider| reachable.contains(&provider.foundation.origin()))
            .cloned()
            .collect::<Vec<_>>();
        let mut documentation = BTreeMap::new();
        let mut sources = project
            .dependency_order
            .iter()
            .filter(|cone| reachable.contains(cone))
            .filter_map(|cone| project.cones[cone].source().map(|input| (*cone, input)))
            .peekable();
        if sources.peek().is_none() {
            return Ok(DependencyInputs {
                providers,
                documentation,
            });
        }
        let mut identities = self
            .identities
            .try_clone()
            .map_err(|error| Error::Dependency(error.to_string()))?;
        for (current, input) in sources {
            if check() {
                return Err(Error::Cancelled);
            }
            let world = world(project, current, &providers)?;
            let (parsed, comments) = sources::complete(snapshot, input, check)?;
            let core = core(current, &providers)?;
            let sources = scoop_hir_lower::CurrentConeSources::try_new(&parsed, core, &world)
                .and_then(|sources| sources.with_documentation(comments))
                .map_err(|error| Error::Dependency(error.to_string()))?
                .with_compile_time_inputs(Arc::new(scoop_toolchain::CompileTimeInputs::default()));
            let cancellation = check.clone();
            let output = scoop_hir_lower::lower_current_export_with_cancellation(
                input.manifest.parsed().semantic().requested_kind(),
                &sources,
                move || cancellation(),
            )
            .map_err(|error| match error {
                scoop_hir_lower::EditingAnalysisError::Cancelled => Error::Cancelled,
                scoop_hir_lower::EditingAnalysisError::Input(diagnostics) => {
                    Error::SourceDependencyUnavailable {
                        cone: current,
                        diagnostics,
                    }
                }
            })?;
            let provider =
                scoop_hir_lower::project_source_dependency(&output, &world, &mut identities)
                    .map_err(|error| Error::Dependency(error.to_string()))?;
            if check() {
                return Err(Error::Cancelled);
            }
            if let Some(comments) = output.documentation {
                documentation.insert(
                    current,
                    Arc::new(
                        hir::SemanticDocumentation::from_complete(comments, &output.export)
                            .map_err(|error| Error::Dependency(error.to_string()))?,
                    ),
                );
            }
            providers.push(provider);
        }
        Ok(DependencyInputs {
            providers,
            documentation,
        })
    }
}

pub(super) fn reachable(project: &ProjectInput, current: ConeIdentity) -> BTreeSet<ConeIdentity> {
    let mut reachable = BTreeSet::new();
    let mut pending = project.dependencies[&current].clone();
    while let Some(cone) = pending.pop() {
        if reachable.insert(cone) {
            pending.extend(&project.dependencies[&cone]);
        }
    }
    reachable
}

pub(super) fn world<'a>(
    project: &ProjectInput,
    current: ConeIdentity,
    providers: &'a [OwnedImportedProvider],
) -> Result<hir::ImportedSemanticWorld<'a>, Error> {
    let reachable = reachable(project, current);
    let mut direct = Vec::new();
    let mut support = Vec::new();
    for provider in providers {
        let cone = provider.foundation.origin();
        if project.dependencies[&current].contains(&cone) {
            direct.push(provider.input());
        } else if reachable.contains(&cone) {
            support.push(provider.input());
        }
    }
    hir::ImportedSemanticWorld::from_dependencies(current, direct, support)
        .map_err(|error| Error::Dependency(error.to_string()))
}

pub(super) fn core(
    current: ConeIdentity,
    providers: &[OwnedImportedProvider],
) -> Result<CoreProtocolInput, Error> {
    if current == ConeIdentity::CORE {
        return Ok(CoreProtocolInput::CurrentDeclarations);
    }
    let core = providers
        .iter()
        .find(|provider| provider.foundation.origin() == ConeIdentity::CORE)
        .ok_or_else(|| Error::Dependency("selected core input is unavailable".into()))?;
    let definitions = core
        .core
        .compiler_protocols()
        .ok_or_else(|| Error::Dependency("selected core has no compiler protocols".into()))?;
    core.foundation
        .import_core_inputs(definitions)
        .map(CoreProtocolInput::from)
        .map_err(|error| Error::Dependency(error.to_string()))
}
