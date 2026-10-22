//! Worker-owned semantic results for the selected project inputs.

use std::rc::Rc;

use scoop_hir_lower::DeclarationAnalysis;
use scoop_identity::ConeIdentity;

use crate::{AnalysisSnapshot, Cancelled, DeclarationAnalysisError, SourceId};

mod dependencies;
mod diagnostics;
mod documentation;
mod implementations;
pub use implementations::{MemberImplementationSearch, TypeImplementationSearch};
mod project;
mod references;
pub use references::ReferenceSearch;
mod sources;

#[derive(Clone, Debug)]
pub enum ProjectAnalysisError {
    Cancelled,
    MissingSource(SourceId),
    ConfigurationUnavailable,
    SourceDependencyUnavailable {
        cone: ConeIdentity,
        diagnostics: Vec<scoop_source::Diagnostic>,
    },
    Artifact(String),
    Dependency(String),
    Declarations(DeclarationAnalysisError),
}

impl ProjectAnalysisError {
    fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Declarations(DeclarationAnalysisError::Cancelled)
        )
    }
}

impl From<Cancelled> for ProjectAnalysisError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// Each selected project and Cone retains its own current semantic arena.
#[derive(Default)]
pub struct SemanticSession {
    projects: Vec<project::ProjectSemantics>,
}

impl SemanticSession {
    pub fn declarations(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<&mut DeclarationAnalysis>, &ProjectAnalysisError> {
        if snapshot.check_cancelled().is_err() || is_cancelled() {
            return Err(&ProjectAnalysisError::Cancelled);
        }
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            return Ok(None);
        };
        let (current, _) = project
            .source_cone(source)
            .expect("the selected project contains the source");
        let revision = snapshot.clone();
        let check: Rc<dyn Fn() -> bool> =
            Rc::new(move || is_cancelled() || revision.check_cancelled().is_err());
        self.project_declarations(snapshot, project, current, check)
            .map(Some)
    }

    fn project_declarations(
        &mut self,
        snapshot: &AnalysisSnapshot,
        project: &crate::ProjectInput,
        current: ConeIdentity,
        check: Rc<dyn Fn() -> bool>,
    ) -> Result<&mut DeclarationAnalysis, &ProjectAnalysisError> {
        self.projects
            .retain(|cached| snapshot.workspace.projects.contains(&cached.input));
        let index = match self
            .projects
            .iter()
            .position(|cached| cached.input == *project)
        {
            Some(index) => index,
            None => {
                let state = project::ProjectSemantics::new(project.clone(), &check)
                    .map_err(|_| &ProjectAnalysisError::Cancelled)?;
                if check() {
                    return Err(&ProjectAnalysisError::Cancelled);
                }
                self.projects.push(state);
                self.projects.len() - 1
            }
        };
        self.projects[index].declarations(snapshot, current, check)
    }
}
