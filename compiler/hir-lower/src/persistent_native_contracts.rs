use std::fmt;

use scoop_hir as hir;

use crate::{Lowerer, persistent_types::identity_inputs};

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
    function_identities: &hir::HirFunctionIdentities,
    property_identities: &hir::HirPropertyIdentities,
    intrinsic_core: &hir::IntrinsicTypeCore,
) -> Result<hir::HirSourceNativeContracts, PersistentSourceNativeContractError> {
    hir::HirSourceNativeContracts::from_declarations(hir::HirSourceNativeContractInputs {
        functions: &lowerer.functions,
        extern_functions: &lowerer.extern_functions,
        globals: &lowerer.globals,
        properties: &lowerer.properties,
        function_identities,
        property_identities,
        type_inputs: identity_inputs(lowerer, nominal_identities, intrinsic_core),
        unit: lowerer.unit,
    })
    .map_err(PersistentSourceNativeContractError)
}

#[derive(Debug)]
pub(crate) struct PersistentSourceNativeContractError(hir::HirSourceNativeContractError);

impl PersistentSourceNativeContractError {
    pub(crate) const fn owner(&self) -> Option<hir::HirSourceNativeContractOwner> {
        self.0.owner()
    }
}

impl fmt::Display for PersistentSourceNativeContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive source-native external contract: {}",
            self.0
        )
    }
}

impl std::error::Error for PersistentSourceNativeContractError {}
