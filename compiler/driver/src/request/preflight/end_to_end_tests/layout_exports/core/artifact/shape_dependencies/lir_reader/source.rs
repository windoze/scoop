use super::*;

/// Test-only semantic graph input. The empty local exports intentionally do
/// not stand for a compiled consumer or confer any artifact publication proof.
pub(super) struct GraphFixture<'a> {
    pub exports: &'a lir::LayoutAbiExportConstituentsV1,
    pub roots: &'a [lir::LayoutAbiDependencyV1],
}

impl lir::LayoutAbiSectionSourceAuthorityV1<&'static str> for GraphFixture<'_> {
    fn validate_local_exports(
        &self,
        exports: &lir::LayoutAbiExportConstituentsV1,
        _: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        if exports == self.exports {
            Ok(())
        } else {
            Err("fixture exports changed")
        }
    }
    fn committed_semantic_roots(&self) -> Result<&[lir::LayoutAbiDependencyV1], &'static str> {
        Ok(self.roots)
    }
    fn validate_physical_imports(
        &self,
        imports: &[lir::ExternalShapeLinkImportV1<'_>],
        _: &mut BudgetMeter,
    ) -> Result<(), &'static str> {
        if imports.is_empty() {
            Ok(())
        } else {
            Err("graph fixture contains no object imports")
        }
    }
}
