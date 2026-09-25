//! Artifact-bound parameter facts used to replay source argument protocols.

use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, PersistentGenericTypeId};
use scoop_wire::WireError;

mod contracts;
mod errors;
mod inventory;
mod replay;
pub use errors::*;

/// An immutable source protocol, not default expansion or materialization authority.
#[derive(Debug)]
pub struct BoundInheritanceParameterProtocolsV1<'a> {
    provider: ConeIdentity,
    protocols: &'a CanonicalInheritanceSourceParameterProtocolsV1,
    array: PersistentGenericTypeId,
}

impl<'a, 'f> BoundInheritanceProtectedCallableSourcesV1<'a, 'f> {
    pub fn bind_parameter_protocols(
        &self,
        constructors: &BoundInheritanceConstructorSourcesV1<'a, 'f>,
        protocols: &'a CanonicalInheritanceSourceParameterProtocolsV1,
        core: &ImportedCoreFundamentalTypeProtocol,
    ) -> Result<BoundInheritanceParameterProtocolsV1<'a>, InheritanceParameterBindingError> {
        use InheritanceParameterBindingError as Error;

        if !std::ptr::eq(self.foundation, constructors.foundation) {
            return Err(Error::FoundationMismatch);
        }
        inventory::validate(self, constructors, protocols)?;
        let array = core.array().persistent();
        for protocol in protocols.records() {
            contracts::validate(self, constructors, protocol, array)?;
        }
        Ok(BoundInheritanceParameterProtocolsV1 {
            provider: self.provider(),
            protocols,
            array,
        })
    }
}
impl<'a> BoundInheritanceParameterProtocolsV1<'a> {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub const fn table(&self) -> &'a CanonicalInheritanceSourceParameterProtocolsV1 {
        self.protocols
    }
    pub fn protocol(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Result<&'a InheritanceSourceParameterProtocolV1, InheritanceParameterBindingError> {
        self.protocols
            .get(owner)
            .ok_or(InheritanceParameterBindingError::MissingProtocol(owner))
    }
    pub fn parameter(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&'a InheritanceSourceParameterV1, InheritanceParameterBindingError> {
        self.protocol(owner)?
            .parameters()
            .get(position as usize)
            .ok_or(InheritanceParameterBindingError::Position { owner, position })
    }
}
