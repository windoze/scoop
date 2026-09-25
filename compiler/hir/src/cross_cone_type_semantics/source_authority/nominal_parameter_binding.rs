//! Complete parameter protocols bound to one nominal source transaction.
use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, PersistentGenericTypeId};
use scoop_wire::WireError;

mod candidates;
mod contracts;
mod errors;
mod inventory;
mod replay;
pub use errors::*;
type Error = NominalParameterBindingError;

/// Source protocols only; default bodies and executable uses require independent proofs.
#[derive(Clone, Copy, Debug)]
pub struct BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    members: &'p BoundNominalMemberSourcesV1<'s, 'a, 'f>,
    constructors: &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f>,
    protocols: &'p CanonicalNominalSourceParameterProtocolsV1,
    array: PersistentGenericTypeId,
}
impl<'s, 'a, 'f> BoundNominalMemberSourcesV1<'s, 'a, 'f> {
    pub fn bind_parameter_protocols<'p>(
        &'p self,
        constructors: &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f>,
        protocols: &'p CanonicalNominalSourceParameterProtocolsV1,
    ) -> Result<BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>, Error> {
        if !std::ptr::eq(self.nominals, constructors.nominals) {
            return Err(Error::NominalSourcesMismatch);
        }
        inventory::validate(self, constructors, protocols)?;
        let array = self.core.array().persistent();
        for protocol in protocols.records() {
            contracts::validate(self, constructors, protocol, array)?;
        }
        Ok(BoundNominalParameterProtocolsV1 {
            members: self,
            constructors,
            protocols,
            array,
        })
    }
}
impl<'p, 's, 'a, 'f> BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.members.provider()
    }
    pub const fn members(&self) -> &'p BoundNominalMemberSourcesV1<'s, 'a, 'f> {
        self.members
    }
    pub const fn constructors(&self) -> &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f> {
        self.constructors
    }
    pub const fn table(&self) -> &'p CanonicalNominalSourceParameterProtocolsV1 {
        self.protocols
    }
    pub fn protocol(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Result<&'p NominalSourceParameterProtocolV1, Error> {
        self.protocols
            .get(owner)
            .ok_or(Error::MissingProtocol(owner))
    }
    pub fn parameter(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&'p InheritanceSourceParameterV1, Error> {
        self.protocol(owner)?
            .parameters()
            .get(position as usize)
            .ok_or(Error::Position { owner, position })
    }
}
