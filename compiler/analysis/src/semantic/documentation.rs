use std::sync::Arc;

use super::*;
use scoop_hir::{SemanticDocumentation, SemanticSourceLocation};

impl SemanticSession {
    /// Keep current and source-provider document arenas separate and reusable.
    pub fn documentation(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        origin: &SemanticSourceLocation,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<Arc<SemanticDocumentation>>, ProjectAnalysisError> {
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            return Ok(None);
        };
        let (current, _) = project
            .source_cone(source)
            .expect("the selected project contains the source");
        let Some(analysis) = self
            .declarations(snapshot, source, is_cancelled)
            .map_err(Clone::clone)?
        else {
            return Ok(None);
        };
        if origin.source.cone() == current {
            return Ok(analysis.documentation().cloned());
        }
        Ok(self
            .projects
            .iter()
            .find(|cached| cached.input == *project)
            .and_then(|cached| cached.dependency_documentation(current, origin.source.cone()))
            .cloned())
    }
}
