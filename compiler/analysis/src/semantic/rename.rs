//! Source edits use completed bindings and a separate ordinary analysis candidate.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    rc::Rc,
    sync::Arc,
};

use scoop_hir::{
    EditingCallableIdentity as Callable, EditingReferenceIndex, EditingReferenceTarget as Target,
    EditingTypeIdentity as Type, SemanticSourceLocation as Origin,
};
use scoop_identity::{ConeIdentity, SourceIdentity};
use scoop_source::{SourceEdit, SourceVersion, Span};

use super::{ProjectAnalysisError, SemanticSession};
use crate::{AnalysisSnapshot, ProjectInput, SnapshotEditError, SourceId, SourceInput};

mod anchors;
mod plan;
mod positions;
mod world;

use world::World;

#[derive(Clone, Debug)]
pub struct RenamePreparation {
    pub source: SourceVersion,
    pub span: Span,
    pub placeholder: String,
}

#[derive(Clone, Debug)]
pub enum RenameError {
    Cancelled,
    Analysis(ProjectAnalysisError),
    Unavailable(&'static str),
    InvalidName,
    Conflict(Origin),
    Edit(SnapshotEditError),
}

impl fmt::Display for RenameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("rename was cancelled"),
            Self::Analysis(error) => write!(formatter, "rename analysis is unavailable: {error:?}"),
            Self::Unavailable(reason) => formatter.write_str(reason),
            Self::InvalidName => formatter.write_str("new name is not a valid source identifier"),
            Self::Conflict(_) => {
                formatter.write_str("rename would change a declaration or reference binding")
            }
            Self::Edit(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RenameError {}

impl From<ProjectAnalysisError> for RenameError {
    fn from(error: ProjectAnalysisError) -> Self {
        if error.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Analysis(error)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct Key {
    arena: Option<ConeIdentity>,
    target: Target,
}

impl Key {
    fn new(cone: ConeIdentity, target: Target) -> Self {
        Self {
            arena: target.is_local().then_some(cone),
            target,
        }
    }
}

impl SemanticSession {
    pub fn prepare_rename(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        span: Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<RenamePreparation>, RenameError> {
        let Some((_, world, origin)) = self.rename_inputs(snapshot, source, span, is_cancelled)?
        else {
            return Ok(None);
        };
        world
            .plan(&origin)
            .map(|plan| plan.map(|plan| plan.preparation))
    }

    pub fn rename(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        span: Span,
        new_name: &str,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<Vec<SourceEdit>>, RenameError> {
        let new_name =
            scoop_parser::identifier_name(new_name).unwrap_or_else(|| new_name.to_owned());
        let spelling = scoop_parser::identifier_spelling(&new_name).into_owned();
        if scoop_parser::identifier_name(&spelling).as_deref() != Some(&new_name) {
            return Err(RenameError::InvalidName);
        }
        let Some((project, world, origin)) =
            self.rename_inputs(snapshot, source, span, is_cancelled)?
        else {
            return Ok(None);
        };
        let Some(plan) = world.plan(&origin)? else {
            return Ok(None);
        };
        if plan.preparation.placeholder == new_name {
            return Ok(Some(Vec::new()));
        }
        let edits = plan.edits(&world, &spelling);
        let candidate = snapshot.with_edits(&edits).map_err(RenameError::Edit)?;
        world.check()?;
        let mut session = Self {
            projects: self
                .projects
                .iter()
                .map(|project| project.for_edits())
                .collect(),
        };
        let updated = World::load(
            &mut session,
            &candidate,
            &project,
            world.cancellation.clone(),
        )?;
        world.verify(&updated, &edits)?;
        world.check()?;
        Ok(Some(edits))
    }

    fn rename_inputs(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        span: Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<(ProjectInput, World, Origin)>, RenameError> {
        snapshot
            .check_cancelled()
            .map_err(|_| RenameError::Cancelled)?;
        let Some(input) = snapshot
            .source(source)
            .map_err(|_| RenameError::Cancelled)?
        else {
            return Ok(None);
        };
        if input
            .text
            .as_str()
            .get(span.start as usize..span.end as usize)
            .and_then(scoop_parser::identifier_name)
            .is_none()
        {
            return Ok(None);
        }
        let Some(project) = snapshot.workspace.project_for_source(source).cloned() else {
            return Ok(None);
        };
        let (_, cone) = project.source_cone(source).expect("selected source Cone");
        let file = cone
            .sources
            .iter()
            .find(|file| file.source == source)
            .expect("selected source input");
        let origin = Origin {
            source: file.identity.clone(),
            span,
        };
        let revision = snapshot.clone();
        let check = Rc::new(move || is_cancelled() || revision.check_cancelled().is_err());
        let world = World::load(self, snapshot, &project, check)?;
        Ok(Some((project, world, origin)))
    }
}
