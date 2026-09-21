//! Declaration source-domain projection without a candidate section input.
use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::resource;
use super::nested_sources::inventory;
use crate::*;
use scoop_wire::{BudgetMeter, WirePath};

impl TypeDeclarationSourceAuthorityV1 {
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path).map_err(resource)?;
        meter.charge_nodes(1, &path).map_err(resource)?;
        let export = &output.output().export;
        let roots = CanonicalSourceNominalIdsV1::from_export_hir(export, meter)?;
        let nominals = CanonicalNominalSourceContractsV1::from_export_hir(export, &roots, meter)?;
        let mut required = inventory::collect(nominals.records().iter(), meter)?;
        let properties = CanonicalNominalSourcePropertiesV1::from_export_hir(
            export,
            &required.properties(meter)?,
            meter,
        )?;
        required.accessors(properties.records(), meter)?;
        let constructors = CanonicalNominalSourceConstructorsV1::from_export_hir(
            export,
            &required.constructors,
            meter,
        )?;
        let callables =
            CanonicalNominalSourceCallablesV1::from_export_hir(export, &required.callables, meter)?;
        let required_protected =
            CanonicalProtectedDeclarationRefsV1::from_export_hir(export, meter)?;
        let entries = TypeDeclarationSourceEntriesV1 {
            required_protected,
            nominals,
            constructors,
            properties,
            callables,
            inheritance: CanonicalSourceInheritanceInventoriesV1::from_ordinary_hir(output, meter)?,
            interfaces: CanonicalInterfaceSourceDispatchesV1::from_ordinary_hir(output, meter)?,
            selections: CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
                output, meter,
            )?,
            dispatch_callables: CanonicalInheritanceSourceCallablesV1::from_ordinary_hir(
                output, meter,
            )?,
        };
        Ok(Self::new(entries))
    }
}
