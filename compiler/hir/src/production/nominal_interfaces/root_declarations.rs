use super::*;
use crate::{CrossConeTypeSemanticsProductionError, SourceInventoryError, SourceNominalId};
use scoop_wire::BudgetMeter;

impl CanonicalNominalInterfacesV1 {
    /// Projects shared declaration data for an explicitly selected local root
    /// set. Public lookup membership is irrelevant to this transient query.
    pub(in crate::production) fn declarations_for_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
        meter: &mut BudgetMeter,
    ) -> Result<Self, NominalInterfaceBuildError> {
        let declarations = source_contracts::project_root_declarations(export, roots, meter)
            .map_err(|error| match error {
                CrossConeTypeSemanticsProductionError::SourceInventory(
                    SourceInventoryError::Resource(error),
                ) => NominalInterfaceBuildError::Resource(error),
                other => NominalInterfaceBuildError::Declarations(other.to_string()),
            })?;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(
                &mut records,
                declarations.len(),
                &scoop_wire::WirePath::root(),
            )
            .map_err(NominalInterfaceBuildError::Resource)?;
        records.extend(declarations.into_values());
        Self::with_support(Vec::new(), records).map_err(NominalInterfaceBuildError::Table)
    }
}
