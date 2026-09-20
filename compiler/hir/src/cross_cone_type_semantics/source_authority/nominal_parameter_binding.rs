//! Complete parameter protocols bound to one nominal source transaction.
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentGenericTypeId, SourceDeclarationKey,
    SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        if !std::ptr::eq(self.nominals, constructors.nominals) {
            return Err(Error::NominalSourcesMismatch);
        }
        inventory::validate(self, constructors, protocols, meter)?;
        let array = self.core.array().persistent();
        meter.charge_work(1, &path)?;
        let key = self
            .nominals
            .foundation
            .identities
            .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(array)
            .map_err(|error| Error::Identity(error.to_string()))?;
        NominalRepresentationSupportV1::charge_source_key_resources(&key, meter, &path)?;
        if key.origin() != ConeIdentity::CORE
            || key.declaration_kind() != SourceDeclarationKind::Class
            || key.duplicate_signature().type_parameter_count() != 1
        {
            return Err(Error::CoreArray);
        }
        for protocol in protocols.records() {
            contracts::validate(self, constructors, protocol, array, meter)?;
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
fn query(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?;
    Ok(())
}
