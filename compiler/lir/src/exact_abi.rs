//! Complete Scoop callable ABI records derived from checked value layouts.
//! Source-to-implementation and selected-use joins belong to the containing
//! section; a physical definition alone does not authorize an import.

use std::sync::Arc;

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ExactCallableSignature, GcEffect, PersistentCallableBodyId,
    StrongCallableDefinitionOwner,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::{
    ExactLayoutExportV1, ExactValueLayoutV1, LirTargetProfile, OdrFreeLirFoundation,
    StrongShapeDefinitionRefV1, StrongShapeDefinitionV1,
};

mod replay;
pub use replay::ExactCallableAbiError;
mod canonical;
pub use canonical::{canonical_scoop_abi_argument, canonical_scoop_abi_value_return};

/// Replays a complete signature from checked layouts without requiring a local
/// body definition. Ordinary bridges and full callable exports share this path.
pub fn replay_canonical_scoop_abi_from_layouts(
    target: LirTargetProfile,
    signature: ExactCallableSignature,
    protocol: ExactCallableProtocolV1,
    layouts: CallableAbiLayoutInputsV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<CanonicalScoopAbiFunctionSignature, ExactCallableAbiError> {
    replay::signature(target, signature, protocol, layouts, meter).map(|(signature, _)| signature)
}

mod physical;
pub use physical::ExactCallablePhysicalAbiError;

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

pub struct CallableAbiLayoutInputsV1<'a> {
    pub receiver: CallableAbiReceiverInputV1<'a>,
    pub parameters: &'a [&'a ExactLayoutExportV1],
    pub result: &'a ExactLayoutExportV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableAbiReceiverLayoutV1 {
    NoReceiver,
    Receiver(Arc<ExactValueLayoutV1>),
}
impl CallableAbiReceiverLayoutV1 {
    pub fn value(&self) -> Option<&ExactValueLayoutV1> {
        match self {
            Self::NoReceiver => None,
            Self::Receiver(value) => Some(value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableAbiLayoutDependenciesV1 {
    receiver: CallableAbiReceiverLayoutV1,
    parameters: Vec<Arc<ExactValueLayoutV1>>,
    result: Arc<ExactValueLayoutV1>,
}
impl CallableAbiLayoutDependenciesV1 {
    pub const fn receiver(&self) -> &CallableAbiReceiverLayoutV1 {
        &self.receiver
    }
    pub fn parameters(&self) -> &[Arc<ExactValueLayoutV1>] {
        &self.parameters
    }
    pub fn result(&self) -> &ExactValueLayoutV1 {
        &self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCallableAbiExportV1(Arc<CallableAbiBodyV1>);

#[derive(Debug, Eq, PartialEq)]
struct CallableAbiBodyV1 {
    target: StrongCallableDefinitionOwner,
    target_profile: LirTargetProfile,
    signature: CanonicalScoopAbiFunctionSignature,
    protocol: ExactCallableProtocolV1,
    layouts: CallableAbiLayoutDependenciesV1,
    physical: StrongShapeDefinitionRefV1,
    definition: StrongShapeDefinitionV1<PersistentCallableBodyId>,
}

impl ExactCallableAbiExportV1 {
    pub fn replay(
        target_profile: LirTargetProfile,
        target: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
        protocol: ExactCallableProtocolV1,
        layouts: CallableAbiLayoutInputsV1<'_>,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactCallableAbiError> {
        replay::callable(
            target_profile,
            target,
            signature,
            protocol,
            layouts,
            foundation,
            meter,
        )
    }

    pub fn target(&self) -> StrongCallableDefinitionOwner {
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
    pub fn layout_dependencies(&self) -> &CallableAbiLayoutDependenciesV1 {
        &self.0.layouts
    }
    pub fn physical_definition(&self) -> StrongShapeDefinitionRefV1 {
        self.0.physical
    }
    pub fn definition(&self) -> StrongShapeDefinitionV1<PersistentCallableBodyId> {
        self.0.definition
    }
}
