//! Declaration source-domain projection without a candidate section input.
use super::CrossConeTypeSemanticsProductionError as Error;
use super::nested_sources::inventory;
use crate::*;

impl TypeDeclarationSourceAuthorityV1 {
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let export = &output.output().export;
        let roots = CanonicalSourceNominalIdsV1::from_export_hir(export)?;
        let nominals = CanonicalNominalSourceContractsV1::from_export_hir(export, &roots)?;
        let mut required = inventory::collect(nominals.records().iter())?;
        let properties =
            CanonicalNominalSourcePropertiesV1::from_export_hir(export, &required.properties()?)?;
        required.accessors(properties.records())?;
        let constructors =
            CanonicalNominalSourceConstructorsV1::from_export_hir(export, &required.constructors)?;
        let callables =
            CanonicalNominalSourceCallablesV1::from_export_hir(export, &required.callables)?;
        let required_protected = CanonicalProtectedDeclarationRefsV1::from_export_hir(export)?;
        let entries = TypeDeclarationSourceEntriesV1 {
            required_protected,
            nominals,
            constructors,
            properties,
            callables,
            inheritance: CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output)?,
            interfaces: CanonicalInterfaceSourceDispatchesV1::from_dependency_hir(output)?,
            selections: CanonicalInheritanceSourceSlotSelectionsV1::from_dependency_hir(output)?,
            dispatch_callables: CanonicalInheritanceSourceCallablesV1::from_dependency_hir(output)?,
        };
        Ok(Self::new(entries))
    }
}
