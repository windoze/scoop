//! Worker-owned semantic results for the selected project inputs.

use scoop_hir_lower::{CoreProtocolInput, DeclarationAnalysis};
use scoop_identity::ConeIdentity;

use crate::{
    AnalysisSnapshot, Cancelled, ConeInput, DeclarationAnalysisError, ProjectInput, SourceId,
    SourceVersion,
};

#[derive(Debug)]
pub enum ProjectAnalysisError {
    Cancelled,
    MissingSource(SourceId),
    ConfigurationUnavailable,
    SourceDependencyUnavailable(ConeIdentity),
    Artifact(String),
    Declarations(DeclarationAnalysisError),
}

impl From<Cancelled> for ProjectAnalysisError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

struct CachedDeclarations {
    project: ProjectInput,
    cone: ConeIdentity,
    versions: Vec<SourceVersion>,
    result: Result<DeclarationAnalysis, ProjectAnalysisError>,
}

/// Keeps the current selected Cone; no arena from an older input is mixed in.
#[derive(Default)]
pub struct SemanticSession {
    declarations: Option<CachedDeclarations>,
}

impl SemanticSession {
    pub fn declarations(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
    ) -> Result<Option<&mut DeclarationAnalysis>, &ProjectAnalysisError> {
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            self.declarations = None;
            return Ok(None);
        };
        let (cone, _) = project
            .source_cone(source)
            .expect("the selected project contains the source");
        let versions = project
            .cones
            .values()
            .filter_map(ConeInput::source)
            .flat_map(|input| &input.sources)
            .map(|file| {
                snapshot
                    .source(file.source)
                    .map_err(ProjectAnalysisError::from)?
                    .ok_or(ProjectAnalysisError::MissingSource(file.source))
                    .map(|source| source.version)
            })
            .collect::<Result<Vec<_>, _>>();
        let unchanged = versions.as_ref().ok().is_some_and(|versions| {
            self.declarations.as_ref().is_some_and(|cached| {
                cached.project == *project
                    && cached.cone == cone
                    && cached.versions == *versions
                    && !matches!(
                        cached.result,
                        Err(ProjectAnalysisError::Cancelled
                            | ProjectAnalysisError::Declarations(
                                DeclarationAnalysisError::Cancelled
                            ))
                    )
            })
        });
        if !unchanged {
            let (versions, result) = match versions {
                Ok(versions) => {
                    let result = snapshot.analyze_artifact_declarations(project, source);
                    (versions, result)
                }
                Err(error) => (Vec::new(), Err(error)),
            };
            self.declarations = Some(CachedDeclarations {
                project: project.clone(),
                cone,
                versions,
                result,
            });
        }
        match &mut self
            .declarations
            .as_mut()
            .expect("the selected input is cached")
            .result
        {
            Ok(analysis) => Ok(Some(analysis)),
            Err(error) => Err(error),
        }
    }
}

impl AnalysisSnapshot {
    /// An artifact dependency closure is already selected by the ordinary loader.
    pub fn analyze_artifact_declarations(
        &self,
        project: &ProjectInput,
        source: SourceId,
    ) -> Result<DeclarationAnalysis, ProjectAnalysisError> {
        self.check_cancelled()?;
        if !project.complete {
            return Err(ProjectAnalysisError::ConfigurationUnavailable);
        }
        let (current, input) = project
            .source_cone(source)
            .ok_or(ProjectAnalysisError::MissingSource(source))?;
        let direct = project.dependencies[&current].clone();
        let mut reachable = std::collections::BTreeSet::new();
        let mut pending = direct.clone();
        while let Some(cone) = pending.pop() {
            if reachable.insert(cone) {
                pending.extend(&project.dependencies[&cone]);
            }
        }
        let mut artifacts = Vec::new();
        for cone in project
            .dependency_order
            .iter()
            .filter(|cone| reachable.contains(cone))
        {
            self.check_cancelled()?;
            match &project.cones[cone] {
                ConeInput::Artifact(artifact) => artifacts.push(&artifact.file),
                ConeInput::Source(_) => {
                    return Err(ProjectAnalysisError::SourceDependencyUnavailable(*cone));
                }
            }
        }
        let dependencies = scoop_slib::CompileDependencyClosure::read_hir(
            current,
            project.configuration.target,
            direct,
            artifacts,
        )
        .map_err(|error| ProjectAnalysisError::Artifact(error.to_string()))?;
        self.check_cancelled()?;
        let world = dependencies
            .imported_semantic_world()
            .map_err(|error| ProjectAnalysisError::Artifact(error.to_string()))?;
        let core = if current == ConeIdentity::CORE {
            CoreProtocolInput::CurrentDeclarations
        } else {
            dependencies
                .import_compiler_protocols(ConeIdentity::CORE)
                .map_err(|error| ProjectAnalysisError::Artifact(error.to_string()))?
                .into()
        };
        self.analyze_declarations(input, &core, &world)
            .map_err(ProjectAnalysisError::Declarations)
    }
}
