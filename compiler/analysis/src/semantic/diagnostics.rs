use super::*;
use scoop_source::Diagnostic;

impl SemanticSession {
    /// Reuse declaration and body queries for the requested source revision.
    pub fn diagnostics(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
    ) -> Result<Vec<Diagnostic>, ProjectAnalysisError> {
        snapshot.check_cancelled()?;
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            return Ok(Vec::new());
        };
        let (_, cone) = project
            .source_cone(source)
            .expect("the selected project contains the source");
        let identity = &cone
            .sources
            .iter()
            .find(|file| file.source == source)
            .expect("the selected Cone contains the source")
            .identity;
        let diagnostics = match self.declarations(snapshot, source, || false) {
            Ok(Some(analysis)) => {
                let mut checked = std::collections::HashSet::new();
                loop {
                    snapshot.check_cancelled()?;
                    let revision = snapshot.clone();
                    analysis
                        .check_source_constructions(Some(identity), move || {
                            revision.check_cancelled().is_err()
                        })
                        .map_err(|_| ProjectAnalysisError::Cancelled)?;
                    let functions = analysis
                        .source_bodies()
                        .into_iter()
                        .filter(|body| body.origin.source == *identity)
                        .map(|body| body.id)
                        .filter(|function| !checked.contains(function))
                        .collect::<Vec<_>>();
                    if functions.is_empty() {
                        break;
                    }
                    for function in functions {
                        let revision = snapshot.clone();
                        analysis
                            .source_body_facts(function, move || {
                                revision.check_cancelled().is_err()
                            })
                            .map_err(|_| ProjectAnalysisError::Cancelled)?;
                        checked.insert(function);
                    }
                }
                analysis.diagnostics()
            }
            Ok(None) => Vec::new(),
            Err(ProjectAnalysisError::Declarations(DeclarationAnalysisError::Input(errors))) => {
                errors.clone()
            }
            Err(error) => return Err(error.clone()),
        };
        snapshot.check_cancelled()?;
        let mut diagnostics = diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.source.as_deref() == Some(identity))
            .collect::<Vec<_>>();
        diagnostics.sort_by(|a, b| {
            a.span
                .map(|span| (span.start, span.end))
                .cmp(&b.span.map(|span| (span.start, span.end)))
                .then(a.message.cmp(&b.message))
        });
        diagnostics.dedup();
        Ok(diagnostics)
    }
}
