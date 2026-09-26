//! Complete recursive source support joined to one artifact's bound declarations.
use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity};
use scoop_wire::{WireEncode, WireError};
use std::collections::BTreeMap;

mod contracts;
mod errors;
mod replay;
pub use errors::*;
type Error = NominalNestedBindingError;

/// Artifact source joins and parameter protocols only. Defaults, dispatch
/// selection and machine-use eligibility remain separate obligations.
#[derive(Debug)]
pub struct BoundNestedNominalSourceV1<'c, 'p, 's, 'a, 'f> {
    members: &'p BoundNominalMemberSourcesV1<'s, 'a, 'f>,
    constructors: &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f>,
    parameter_sources: &'p CanonicalNominalSourceParameterProtocolsV1,
    record: &'c NominalSupportNestedInterfaceV1,
    protocols: BTreeMap<CallableTemplateOrigin, CheckedProtectedSourceProtocolV1<'c>>,
}
impl<'c, 'p, 's, 'a, 'f> BoundNestedNominalSourceV1<'c, 'p, 's, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.members.provider()
    }
    pub const fn members(&self) -> &'p BoundNominalMemberSourcesV1<'s, 'a, 'f> {
        self.members
    }
    pub const fn constructors(&self) -> &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f> {
        self.constructors
    }
    pub const fn parameter_sources(&self) -> &'p CanonicalNominalSourceParameterProtocolsV1 {
        self.parameter_sources
    }
    pub const fn record(&self) -> &'c NominalSupportNestedInterfaceV1 {
        self.record
    }
    pub fn protocols(&self) -> impl Iterator<Item = CheckedProtectedSourceProtocolV1<'c>> + '_ {
        self.protocols.values().copied()
    }
    pub fn protocol(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Option<CheckedProtectedSourceProtocolV1<'c>> {
        self.protocols.get(&owner).copied()
    }
}
impl<'p, 's, 'a, 'f> BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    pub fn validate_nested_source<'c>(
        &mut self,
        candidate: &'c NominalSupportNestedInterfaceV1,
        protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
    ) -> Result<BoundNestedNominalSourceV1<'c, 'p, 's, 'a, 'f>, Error> {
        let mut checked = BoundNestedNominalSourceV1 {
            members: self.members(),
            constructors: self.constructors(),
            parameter_sources: self.table(),
            record: candidate,
            protocols: BTreeMap::new(),
        };
        replay::validate(self, candidate, protocols, &mut checked)?;
        Ok(checked)
    }
}
pub(super) fn compare<T: WireEncode + PartialEq>(
    actual: &T,
    expected: &T,
    declaration: NestedSupportDeclarationV1,
    field: &'static str,
) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::Contract { declaration, field });
    }
    Ok(())
}
