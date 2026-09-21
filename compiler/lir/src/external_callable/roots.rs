use crate::GcEffect;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// The caller-side root protocol inseparably paired with an external callable's
/// GC effect. A managed call must be emitted as a statepoint; a no-GC call has
/// no caller-root publication step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCallableRootPlan {
    ManagedStatepoint,
    NoGc,
}

impl ExternalCallableRootPlan {
    pub const fn gc_effect(self) -> GcEffect {
        match self {
            Self::ManagedStatepoint => GcEffect::Managed,
            Self::NoGc => GcEffect::NoGc,
        }
    }
}

impl WireEncode for ExternalCallableRootPlan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ManagedStatepoint => 1,
            Self::NoGc => 2,
        })
    }
}

impl WireDecode for ExternalCallableRootPlan {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ManagedStatepoint),
            2 => Ok(Self::NoGc),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

impl ExternalCallableRootPlan {
    pub(crate) const fn from_dependency(plan: crate::DependencyExternalCallableRootPlanV1) -> Self {
        match plan {
            crate::DependencyExternalCallableRootPlanV1::ManagedStatepoint => {
                Self::ManagedStatepoint
            }
            crate::DependencyExternalCallableRootPlanV1::NoGc => Self::NoGc,
        }
    }
}
