//! Complete canonical ABI records bound to their actual callable definitions.

use std::sync::Arc;

use scoop_identity::{
    CallableDefinitionOwner, CanonicalScoopAbiFunctionSignature, GcEffect, PersistentCallableBodyId,
};
use scoop_wire::{WireError, WirePath};

use crate::{
    ConeLirFoundation, ExactLayoutExportV1, LirTargetProfile, StrongShapeDefinitionRefV1,
    StrongShapeDefinitionV1,
};

mod replay;
pub use replay::ExactCallableAbiError;
mod canonical;
pub use canonical::{canonical_scoop_abi_argument, canonical_scoop_abi_value_return};

mod table;
pub use table::*;

mod wire;
pub use wire::{
    DecodedCanonicalExactCallableAbiExportsV1, DecodedExactCallableAbiExportV1,
    ExactCallableAbiWireError,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactCallableProtocolV1 {
    OrdinaryManaged,
    OrdinaryNoGc,
}
impl ExactCallableProtocolV1 {
    pub const fn gc_effect(self) -> GcEffect {
        match self {
            Self::OrdinaryManaged => GcEffect::Managed,
            Self::OrdinaryNoGc => GcEffect::NoGc,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CallableAbiReceiverInputV1<'a> {
    NoReceiver,
    Receiver(&'a ExactLayoutExportV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCallableAbiExportV1(Arc<CallableAbiBodyV1>);

#[derive(Debug, Eq, PartialEq)]
struct CallableAbiBodyV1 {
    target: CallableDefinitionOwner,
    target_profile: LirTargetProfile,
    signature: CanonicalScoopAbiFunctionSignature,
    protocol: ExactCallableProtocolV1,
    physical: StrongShapeDefinitionRefV1,
    definition: StrongShapeDefinitionV1<PersistentCallableBodyId>,
}

impl ExactCallableAbiExportV1 {
    /// Attaches the complete ABI from the defining body to its physical symbol.
    pub fn from_signature(
        target_profile: LirTargetProfile,
        target: impl Into<CallableDefinitionOwner>,
        signature: CanonicalScoopAbiFunctionSignature,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactCallableAbiError> {
        replay::callable(target_profile, target.into(), signature, foundation)
    }

    pub fn target(&self) -> CallableDefinitionOwner {
        self.0.target
    }
    pub fn target_profile(&self) -> LirTargetProfile {
        self.0.target_profile
    }
    pub fn canonical_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.0.signature
    }
    pub const fn calling_convention(&self) -> crate::CallingConvention {
        crate::CallingConvention::Cdecl
    }
    pub fn call_protocol(&self) -> ExactCallableProtocolV1 {
        self.0.protocol
    }
    pub fn physical_definition(&self) -> StrongShapeDefinitionRefV1 {
        self.0.physical
    }
    pub fn definition(&self) -> StrongShapeDefinitionV1<PersistentCallableBodyId> {
        self.0.definition
    }
}
