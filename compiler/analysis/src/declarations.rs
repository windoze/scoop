//! Selected editor inputs enter the ordinary frontend's declaration analysis.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_hir_lower::{CoreProtocolInput, DeclarationAnalysis, EditingAnalysisError};
use scoop_identity::{SourceIdentity, SourceRole};

use crate::{AnalysisSnapshot, Cancelled, SourceConeInput, SourceId, SourceInput};

#[derive(Clone, Debug)]
pub enum DeclarationAnalysisError {
    Cancelled,
    MissingSource(SourceId),
    SourceLength(scoop_source::SourceLengthError),
    Input(Vec<ast::Diagnostic>),
}

impl From<Cancelled> for DeclarationAnalysisError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

impl SourceInput {
    /// Project an exact AST source range to the name recorded by its grammar.
    pub fn declaration_name_span(
        &self,
        span: scoop_source::Span,
    ) -> Result<Option<scoop_source::Span>, &scoop_source::SourceLengthError> {
        Ok(self
            .parse()?
            .declarations()
            .iter()
            .find(|declaration| declaration.source_span == Some(span))
            .and_then(|declaration| declaration.name.as_ref())
            .map(|name| name.span))
    }

    pub fn declaration_source<'a>(
        &'a self,
        identity: &'a SourceIdentity,
        role: SourceRole,
    ) -> Result<ast::DeclarationSource<'a, 'a>, &'a scoop_source::SourceLengthError> {
        Ok(scoop_parser::declaration_source(
            self.parse()?,
            identity,
            role,
            self.text.as_str(),
        ))
    }
}

impl AnalysisSnapshot {
    pub fn analyze_declarations(
        &self,
        cone: &SourceConeInput,
        core: &CoreProtocolInput,
        world: &hir::ImportedSemanticWorld<'_>,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<DeclarationAnalysis, DeclarationAnalysisError> {
        let mut sources = Vec::with_capacity(cone.sources.len());
        for file in &cone.sources {
            if is_cancelled() {
                return Err(DeclarationAnalysisError::Cancelled);
            }
            let source = self
                .source(file.source)?
                .ok_or(DeclarationAnalysisError::MissingSource(file.source))?;
            sources.push(
                source
                    .declaration_source(&file.identity, file.role)
                    .map_err(|error| DeclarationAnalysisError::SourceLength(*error))?,
            );
        }
        let analysis = scoop_hir_lower::analyze_declarations(
            &sources,
            core,
            world,
            &cone.test_dependencies,
            || is_cancelled() || self.check_cancelled().is_err(),
        )
        .map_err(|error| match error {
            EditingAnalysisError::Cancelled => DeclarationAnalysisError::Cancelled,
            EditingAnalysisError::Input(diagnostics) => {
                DeclarationAnalysisError::Input(diagnostics)
            }
        })?;
        self.check_cancelled()?;
        Ok(analysis)
    }
}
