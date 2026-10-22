//! Selected editor inputs enter the ordinary frontend's declaration analysis.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_hir_lower::{CoreProtocolInput, DeclarationAnalysis};
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
    ) -> Result<DeclarationAnalysis, DeclarationAnalysisError> {
        let mut sources = Vec::with_capacity(cone.sources.len());
        for file in &cone.sources {
            let source = self
                .source(file.source)?
                .ok_or(DeclarationAnalysisError::MissingSource(file.source))?;
            sources.push(
                source
                    .declaration_source(&file.identity, file.role)
                    .map_err(|error| DeclarationAnalysisError::SourceLength(*error))?,
            );
        }
        let analysis =
            scoop_hir_lower::analyze_declarations(&sources, core, world, &cone.test_dependencies)
                .map_err(DeclarationAnalysisError::Input)?;
        self.check_cancelled()?;
        Ok(analysis)
    }
}
