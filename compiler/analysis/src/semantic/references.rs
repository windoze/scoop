use super::*;
use scoop_hir::{EditingReference, EditingReferenceTarget as Target, SemanticSourceLocation};
use scoop_source::Span;

/// Completed declaration and function-body references in the selected project.
#[derive(Debug)]
pub struct ReferenceSearch {
    pub occurrences: Vec<EditingReference>,
    pub unavailable: Vec<SemanticSourceLocation>,
    pub declarations_available: bool,
}

impl SemanticSession {
    pub fn references(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        span: Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<ReferenceSearch>, ProjectAnalysisError> {
        snapshot.check_cancelled()?;
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            return Ok(None);
        };
        let (current, cone) = project
            .source_cone(source)
            .expect("the selected project contains the source");
        let file = cone
            .sources
            .iter()
            .find(|file| file.source == source)
            .expect("the selected Cone contains the source");
        let origin = SemanticSourceLocation {
            source: file.identity.clone(),
            span,
        };
        let declaration_span = snapshot
            .source(source)?
            .ok_or(ProjectAnalysisError::MissingSource(source))?
            .declaration_source_span(span)
            .map_err(|error| {
                ProjectAnalysisError::Declarations(DeclarationAnalysisError::SourceLength(*error))
            })?;
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
            let index = analysis
                .reference_index(move || cancellation())
                .map_err(|_| ProjectAnalysisError::Cancelled)?;
            let mut targets = index.targets_at(&origin).to_vec();
            if let Some(span) = declaration_span.filter(|span| *span != origin.span) {
                let declaration = SemanticSourceLocation { span, ..origin };
                for target in index.targets_at(&declaration) {
                    if matches!(
                        target,
                        Target::Callable(scoop_hir::EditingCallableIdentity::Constructor(_))
                    ) && targets
                        .iter()
                        .any(|target| matches!(target, Target::Type(_)))
                    {
                        continue;
                    }
                    if !targets.contains(target) {
                        targets.push(*target);
                    }
                }
            }
            let exported = targets
                .iter()
                .filter_map(|target| match target {
                    Target::ImportAlias(alias) => Some(&index.aliases[*alias].exports),
                    _ => None,
                })
                .flatten()
                .copied()
                .map(Target::ExportBinding)
                .collect::<Vec<_>>();
            for target in exported {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
            targets
        };
        if targets.is_empty() {
            return Ok(None);
        }
        let mut result = ReferenceSearch {
            occurrences: Vec::new(),
            unavailable: Vec::new(),
            declarations_available: true,
        };
        for (&cone, input) in &project.cones {
            if input.source().is_none()
                || (cone != current && targets.iter().all(|target| target.is_local()))
            {
                continue;
            }
            if check() {
                return Err(ProjectAnalysisError::Cancelled);
            }
            let analysis = self
                .project_declarations(snapshot, project, cone, check.clone())
                .map_err(Clone::clone)?;
            let cancellation = check.clone();
            let index = analysis
                .reference_index(move || cancellation())
                .map_err(|_| ProjectAnalysisError::Cancelled)?;
            result.unavailable.extend(index.unavailable.iter().cloned());
            result.declarations_available &= index.declarations_available;
            for &target in &targets {
                if cone == current || !target.is_local() {
                    result
                        .occurrences
                        .extend(index.references_to(target).cloned());
                }
            }
        }
        if check() {
            return Err(ProjectAnalysisError::Cancelled);
        }
        Ok(Some(result))
    }
}
