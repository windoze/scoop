use std::{collections::BTreeMap, rc::Rc, sync::Arc};

use scoop_hir_lower::DeclarationAnalysis;
use scoop_identity::ConeIdentity;

use super::{ProjectAnalysisError as Error, dependencies};
use crate::{AnalysisSnapshot, ConeInput, ProjectInput, SourceVersion};

struct CachedDeclarations {
    versions: Vec<SourceVersion>,
    inputs: Arc<scoop_toolchain::CompileTimeInputs>,
    result: Result<DeclarationAnalysis, Error>,
}

struct CachedDependencies {
    versions: Vec<SourceVersion>,
    inputs: Vec<Arc<scoop_toolchain::CompileTimeInputs>>,
    result: Result<dependencies::DependencyInputs, Error>,
}

pub(super) struct ProjectSemantics {
    pub input: ProjectInput,
    artifacts: Rc<Result<dependencies::ArtifactInputs, Error>>,
    dependencies: BTreeMap<ConeIdentity, CachedDependencies>,
    declarations: BTreeMap<ConeIdentity, CachedDeclarations>,
}

impl ProjectSemantics {
    pub fn compiler_inputs(
        &self,
    ) -> impl Iterator<Item = &Arc<scoop_toolchain::CompileTimeInputs>> {
        self.declarations
            .values()
            .map(|cached| &cached.inputs)
            .chain(self.dependencies.values().flat_map(|cached| &cached.inputs))
    }

    pub fn dependency_documentation(
        &self,
        current: ConeIdentity,
        target: ConeIdentity,
    ) -> Option<&std::sync::Arc<scoop_hir::SemanticDocumentation>> {
        self.dependencies
            .get(&current)?
            .result
            .as_ref()
            .ok()?
            .documentation
            .get(&target)
    }

    pub fn new(input: ProjectInput, check: &Rc<dyn Fn() -> bool>) -> Result<Self, Error> {
        let artifacts = dependencies::ArtifactInputs::load(&input, check);
        if artifacts.as_ref().is_err_and(Error::is_cancelled) {
            return Err(Error::Cancelled);
        }
        Ok(Self {
            input,
            artifacts: Rc::new(artifacts),
            dependencies: BTreeMap::new(),
            declarations: BTreeMap::new(),
        })
    }

    pub(super) fn for_edits(&self) -> Self {
        Self {
            input: self.input.clone(),
            artifacts: self.artifacts.clone(),
            dependencies: BTreeMap::new(),
            declarations: BTreeMap::new(),
        }
    }

    pub fn declarations(
        &mut self,
        snapshot: &AnalysisSnapshot,
        current: ConeIdentity,
        check: Rc<dyn Fn() -> bool>,
    ) -> Result<&mut DeclarationAnalysis, &Error> {
        if !self.input.complete {
            return Err(&Error::ConfigurationUnavailable);
        }
        let reachable = dependencies::reachable(&self.input, current);
        let versions = source_versions(snapshot, &self.input, |cone| {
            cone == current || reachable.contains(&cone)
        });
        let dependency_inputs_current = self
            .dependencies
            .get(&current)
            .is_some_and(|cached| cached.inputs.iter().all(|inputs| inputs.inputs_current()));
        let unchanged = versions.as_ref().ok().is_some_and(|versions| {
            self.declarations.get(&current).is_some_and(|cached| {
                cached.versions == *versions
                    && cached.inputs.inputs_current()
                    && dependency_inputs_current
            })
        });
        if !unchanged {
            let inputs = Arc::new(
                self.input.cones[&current]
                    .source()
                    .expect("the requested Cone is source input")
                    .compiler_inputs(),
            );
            let (versions, result) = match versions {
                Ok(versions) => {
                    let result = self.analyze(
                        snapshot,
                        current,
                        check,
                        inputs.clone(),
                        dependency_inputs_current,
                    );
                    (versions, result)
                }
                Err(error) => (Vec::new(), Err(error)),
            };
            if result.as_ref().is_err_and(Error::is_cancelled) {
                return Err(&Error::Cancelled);
            }
            self.declarations.insert(
                current,
                CachedDeclarations {
                    versions,
                    inputs,
                    result,
                },
            );
        }
        self.declarations
            .get_mut(&current)
            .expect("the selected Cone was analyzed")
            .result
            .as_mut()
            .map_err(|error| &*error)
    }

    fn analyze(
        &mut self,
        snapshot: &AnalysisSnapshot,
        current: ConeIdentity,
        check: Rc<dyn Fn() -> bool>,
        inputs: Arc<scoop_toolchain::CompileTimeInputs>,
        dependency_inputs_current: bool,
    ) -> Result<DeclarationAnalysis, Error> {
        let artifacts = self.artifacts.as_ref().as_ref().map_err(Clone::clone)?;
        let reachable = dependencies::reachable(&self.input, current);
        let versions = source_versions(snapshot, &self.input, |cone| reachable.contains(&cone))?;
        if self
            .dependencies
            .get(&current)
            .is_none_or(|cached| cached.versions != versions || !dependency_inputs_current)
        {
            let mut inputs = Vec::new();
            let result =
                artifacts.with_sources(snapshot, &self.input, &reachable, &check, &mut inputs);
            if result.as_ref().is_err_and(Error::is_cancelled) {
                return Err(Error::Cancelled);
            }
            self.dependencies.insert(
                current,
                CachedDependencies {
                    versions,
                    inputs,
                    result,
                },
            );
        }
        let dependencies = self.dependencies[&current]
            .result
            .as_ref()
            .map_err(Clone::clone)?;
        let providers = &dependencies.providers;
        let world = dependencies::world(&self.input, current, providers)?;
        let core = dependencies::core(current, providers)?;
        let input = self.input.cones[&current]
            .source()
            .expect("the requested Cone is source input");
        snapshot
            .analyze_declarations_with_inputs(input, &core, &world, inputs, || check())
            .map_err(Error::Declarations)
    }
}

fn source_versions(
    snapshot: &AnalysisSnapshot,
    project: &ProjectInput,
    selected: impl Fn(ConeIdentity) -> bool,
) -> Result<Vec<SourceVersion>, Error> {
    project
        .cones
        .iter()
        .filter(|(cone, _)| selected(**cone))
        .filter_map(|(_, cone)| ConeInput::source(cone))
        .flat_map(|input| &input.sources)
        .map(|file| {
            snapshot
                .source(file.source)?
                .ok_or(Error::MissingSource(file.source))
                .map(|source| source.version)
        })
        .collect()
}
