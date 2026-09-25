use super::*;

/// Independent checked MIR-to-LIR input. Implementations must project actual
/// committed semantic roots rather than consult the candidate section wire.
pub trait LayoutAbiSectionSourceAuthorityV1<E> {
    fn validate_local_exports(&self, exports: &LayoutAbiExportConstituentsV1) -> Result<(), E>;

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], E>;

    fn validate_physical_imports(
        &self,
        imports: &[crate::ExternalShapeLinkImportV1<'_>],
    ) -> Result<(), E>;
}
