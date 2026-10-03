use super::*;
use crate::{CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1};

impl CanonicalCallableInterfacesV1 {
    pub(in crate::production) fn from_nominal_declarations(
        export: &ExportHir,
        properties: &CanonicalPropertyInterfacesV1,
        nominals: &CanonicalNominalInterfacesV1,
    ) -> Result<Self, CallableInterfaceBuildError> {
        let projection = CallableProjection {
            export,
            signatures: HirInterfaceSignatureProjector::new(export),
            properties,
        };
        let records = support::project(
            &projection,
            nominals,
            &[],
            &std::collections::BTreeMap::new(),
        )?;
        let table =
            Self::with_support(Vec::new(), records).map_err(CallableInterfaceBuildError::Table)?;
        table
            .validate_declaration_inventory(nominals, properties)
            .map_err(CallableInterfaceBuildError::Inventory)?;
        properties
            .validate_accessor_closure(&table)
            .map_err(CallableInterfaceBuildError::AccessorClosure)?;
        Ok(table)
    }
}
