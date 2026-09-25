use super::*;
use crate::{CrossConeTypeSemanticsProductionError, SourceInventoryError, SourceNominalId};

impl CanonicalNominalInterfacesV1 {
    /// Projects shared declaration data for an explicitly selected local root
    /// set. Public lookup membership is irrelevant to this transient query.
    pub(in crate::production) fn declarations_for_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
    ) -> Result<Self, NominalInterfaceBuildError> {
        let declarations = source_contracts::project_root_declarations(export, roots).map_err(
            |error| match error {
                CrossConeTypeSemanticsProductionError::SourceInventory(
                    SourceInventoryError::Resource(error),
                ) => NominalInterfaceBuildError::Resource(error),
                other => NominalInterfaceBuildError::Declarations(other.to_string()),
            },
        )?;
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut records,
            declarations.len(),
            &scoop_wire::WirePath::root(),
        )
        .map_err(NominalInterfaceBuildError::Resource)?;
        records.extend(declarations.into_values());
        Self::with_support(Vec::new(), records).map_err(NominalInterfaceBuildError::Table)
    }
}
