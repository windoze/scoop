use super::ProtectedDefaultTemplateV1;
use crate::{
    CheckedProtectedSourceProtocolV1, DefaultTemplateProviderShapeV1,
    NominalInterfaceShapeAuthority, ProtectedDefaultOwnerSourceV1,
};
use scoop_wire::BudgetMeter;

mod contract;
mod errors;
mod origin;
mod owner;
mod parameters;
mod receiver;
mod root;
mod types;
pub use errors::*;
pub use origin::*;
pub use root::ProtectedDefaultRootSemanticAuthority;
#[cfg(test)]
mod tests;

impl ProtectedDefaultTemplateV1 {
    /// Checks the source/provider contract. Body operations, exact references
    /// and complete access coverage remain separate prerequisites.
    pub fn validate_contract_semantics<A, E>(
        &self,
        owner: ProtectedDefaultOwnerSourceV1<'_>,
        protocol: CheckedProtectedSourceProtocolV1<'_>,
        authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<(), ProtectedDefaultTemplateContractSemanticError<E>>
    where
        A: NominalInterfaceShapeAuthority<E> + ProtectedDefaultRootSemanticAuthority<E>,
    {
        self.validate_contract_semantics_with_provider(owner, protocol, authority, meter)
            .map(|_| ())
    }

    pub(crate) fn validate_contract_semantics_with_provider<A, E>(
        &self,
        owner: ProtectedDefaultOwnerSourceV1<'_>,
        protocol: CheckedProtectedSourceProtocolV1<'_>,
        authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultTemplateProviderShapeV1, ProtectedDefaultTemplateContractSemanticError<E>>
    where
        A: NominalInterfaceShapeAuthority<E> + ProtectedDefaultRootSemanticAuthority<E>,
    {
        contract::validate(self, owner, protocol, authority, meter)
    }
}
