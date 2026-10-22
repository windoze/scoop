use super::*;
use scoop_hir::{EditingTypeImplementation, EditingValue as V};
use scoop_source::Span;

#[derive(Debug)]
pub struct TypeImplementationSearch {
    pub implementations: Vec<EditingTypeImplementation>,
    pub declarations_available: bool,
}

impl SemanticSession {
    pub fn type_implementations(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        span: Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<TypeImplementationSearch>, ProjectAnalysisError> {
        snapshot.check_cancelled()?;
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            return Ok(None);
        };
        let (current, cone) = project.source_cone(source).expect("selected source Cone");
        let file = cone
            .sources
            .iter()
            .find(|file| file.source == source)
            .expect("selected source");
        let revision = snapshot.clone();
        let check: Rc<dyn Fn() -> bool> =
            Rc::new(move || is_cancelled() || revision.check_cancelled().is_err());
        if check() {
            return Err(ProjectAnalysisError::Cancelled);
        }
        let targets = {
            let analysis = self
                .project_declarations(snapshot, project, current, check.clone())
                .map_err(Clone::clone)?;
            let cancellation = check.clone();
            match analysis
                .type_implementation_targets(&file.identity, span, move || cancellation())
                .map_err(|_| ProjectAnalysisError::Cancelled)?
            {
                V::Known(targets) if !targets.is_empty() => targets,
                _ => return Ok(None),
            }
        };
        let mut result = TypeImplementationSearch {
            implementations: Vec::new(),
            declarations_available: true,
        };
        for (&cone, input) in &project.cones {
            if input.source().is_none() {
                continue;
            }
            if check() {
                return Err(ProjectAnalysisError::Cancelled);
            }
            let analysis = self
                .project_declarations(snapshot, project, cone, check.clone())
                .map_err(Clone::clone)?;
            for &target in &targets {
                match analysis
                    .type_implementations(target, || check())
                    .map_err(|_| ProjectAnalysisError::Cancelled)?
                {
                    V::Known(implementations) => {
                        result.implementations.extend_from_slice(implementations)
                    }
                    V::Unavailable(_) => result.declarations_available = false,
                }
            }
        }
        if check() {
            return Err(ProjectAnalysisError::Cancelled);
        }
        let mut seen = std::collections::HashSet::new();
        result
            .implementations
            .retain(|implementation| seen.insert(implementation.declaration));
        Ok(Some(result))
    }
}
