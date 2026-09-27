use super::*;
use crate::{
    CanonicalSourceNominalIdsV1, CrossConeTypeSemanticsProductionError, SourceInventoryError,
    SourceNominalId,
};

impl CanonicalNominalInterfacesV1 {
    /// Projects shared declaration data for an explicitly selected local root
    /// set. Public lookup membership is irrelevant to this transient query.
    pub(in crate::production) fn declarations_for_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
    ) -> Result<Self, NominalInterfaceBuildError> {
        let required = CanonicalSourceNominalIdsV1::from_complete_roots(export, roots)
            .map_err(declaration_error)?;
        Self::declarations_for_required(export, &required)
    }

    pub(in crate::production) fn declarations_for_required(
        export: &ExportHir,
        required: &CanonicalSourceNominalIdsV1,
    ) -> Result<Self, NominalInterfaceBuildError> {
        let declarations = source_contracts::project_required_declarations(export, required)
            .map_err(declaration_error)?;
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

fn declaration_error(error: CrossConeTypeSemanticsProductionError) -> NominalInterfaceBuildError {
    match error {
        CrossConeTypeSemanticsProductionError::SourceInventory(SourceInventoryError::Resource(
            error,
        )) => NominalInterfaceBuildError::Resource(error),
        other => NominalInterfaceBuildError::Declarations(other.to_string()),
    }
}
