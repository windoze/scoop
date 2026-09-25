//! Complete protected source replay against one artifact's bound declarations.
use super::nominal_nested_binding::compare;
use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity};
use scoop_wire::{WireError, WirePath};
use std::collections::{BTreeMap, BTreeSet};

mod errors;
mod inventory;
mod replay;
pub use errors::*;
type Error = ProtectedDeclarationBindingError;

/// Source completeness and protocol proofs. Default bodies, dispatch target
/// selection and machine-use eligibility remain separate obligations.
#[derive(Debug)]
pub struct BoundProtectedDeclarationSourcesV1<'c, 'p, 's, 'a, 'f> {
    members: &'p BoundNominalMemberSourcesV1<'s, 'a, 'f>,
    constructors: &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f>,
    parameter_sources: &'p CanonicalNominalSourceParameterProtocolsV1,
    table: &'c CanonicalProtectedDeclarationInterfacesV1,
    representations: &'c CanonicalNominalRepresentationSupportV1,
    protocols: BTreeMap<CallableTemplateOrigin, CheckedProtectedSourceProtocolV1<'c>>,
}
impl<'c, 'p, 's, 'a, 'f> BoundProtectedDeclarationSourcesV1<'c, 'p, 's, 'a, 'f> {
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
    pub const fn table(&self) -> &'c CanonicalProtectedDeclarationInterfacesV1 {
        self.table
    }
    pub const fn representations(&self) -> &'c CanonicalNominalRepresentationSupportV1 {
        self.representations
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
    pub fn validate_protected_declarations<'c>(
        &mut self,
        table: &'c CanonicalProtectedDeclarationInterfacesV1,
        protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
        representations: &'c CanonicalNominalRepresentationSupportV1,
    ) -> Result<BoundProtectedDeclarationSourcesV1<'c, 'p, 's, 'a, 'f>, Error> {
        inventory::validate(self, table)?;
        let mut checked = BoundProtectedDeclarationSourcesV1 {
            members: self.members(),
            constructors: self.constructors(),
            parameter_sources: self.table(),
            table,
            representations,
            protocols: BTreeMap::new(),
        };
        replay::validate(self, protocols, &mut checked)?;
        Ok(checked)
    }
}
fn retain<'c>(
    checked: &mut BoundProtectedDeclarationSourcesV1<'c, '_, '_, '_, '_>,
    protocol: CheckedProtectedSourceProtocolV1<'c>,
) -> Result<(), Error> {
    let owner = protocol.record().owner();

    checked.protocols.entry(owner).or_insert(protocol);
    Ok(())
}

pub(super) fn required_declarations(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
) -> Result<CanonicalProtectedDeclarationRefsV1, Error> {
    let required = inventory::collect(authority)?;
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, required.len(), &WirePath::root())?;

    values.extend(required);
    CanonicalProtectedDeclarationRefsV1::try_new(values).map_err(|_| Error::Inventory)
}
