use super::*;
use crate::{CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1};
use scoop_wire::BudgetMeter;

impl CanonicalCallableInterfacesV1 {
    pub(in crate::production) fn from_nominal_declarations(
        export: &ExportHir,
        properties: &CanonicalPropertyInterfacesV1,
        nominals: &CanonicalNominalInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, CallableInterfaceBuildError> {
        let projection = CallableProjection {
            export,
            signatures: HirInterfaceSignatureProjector::new(export),
            properties,
        };
        let records = support::project(&projection, nominals, &[], meter)?;
        let table =
            Self::with_support(Vec::new(), records).map_err(CallableInterfaceBuildError::Table)?;
        table
            .validate_declaration_inventory(nominals, properties, meter)
            .map_err(CallableInterfaceBuildError::Inventory)?;
        properties
            .validate_accessor_closure_with_budget(&table, meter)
            .map_err(CallableInterfaceBuildError::AccessorClosure)?;
        Ok(table)
    }
}
