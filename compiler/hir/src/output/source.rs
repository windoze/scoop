//! Complete source HIR before local type and callable materialization.

/// The ordinary frontend result used by source dependencies and by the next
/// lowering step. Every body and type in `export` has completed normal checks.
pub struct DependencyExportHirOutput {
    pub export: crate::ExportHirOutput,
    pub imported_dependencies: crate::SelectedImportedDependencySet,
    pub binding_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
    pub warnings: Vec<scoop_ast::Diagnostic>,
    pub documentation: Option<crate::ConeDocumentation>,
}
