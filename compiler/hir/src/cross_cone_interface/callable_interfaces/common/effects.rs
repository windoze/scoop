use std::fmt;

use scoop_identity::{Effect, GcEffect};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{CallableImplementationV1, CallableOperatorRoleV1};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableSafetyV1 {
    Safe,
    Unsafe,
}

impl WireEncode for CallableSafetyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Safe => 1,
            Self::Unsafe => 2,
        })
    }
}

impl WireDecode for CallableSafetyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Safe),
            2 => Ok(Self::Unsafe),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableInfixV1 {
    Ordinary,
    Infix,
}

impl WireEncode for CallableInfixV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Ordinary => 1,
            Self::Infix => 2,
        })
    }
}

impl WireDecode for CallableInfixV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Ordinary),
            2 => Ok(Self::Infix),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableModalityV1 {
    Final,
    Open,
    Abstract,
    InterfaceDefault,
}

impl WireEncode for CallableModalityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Final => 1,
            Self::Open => 2,
            Self::Abstract => 3,
            Self::InterfaceDefault => 4,
        })
    }
}

impl WireDecode for CallableModalityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Final),
            2 => Ok(Self::Open),
            3 => Ok(Self::Abstract),
            4 => Ok(Self::InterfaceDefault),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PublicLookupAccessV1 {
    DirectOnly,
    PublicSlot,
}

impl WireEncode for PublicLookupAccessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::DirectOnly => 1,
            Self::PublicSlot => 2,
        })
    }
}

impl WireDecode for PublicLookupAccessV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::DirectOnly),
            2 => Ok(Self::PublicSlot),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableSourceEffectsV1 {
    execution: Effect,
    safety: CallableSafetyV1,
    gc_effect: GcEffect,
    implementation: CallableImplementationV1,
    operator_role: CallableOperatorRoleV1,
    infix: CallableInfixV1,
}

impl CallableSourceEffectsV1 {
    pub fn try_new(
        execution: Effect,
        safety: CallableSafetyV1,
        gc_effect: GcEffect,
        implementation: CallableImplementationV1,
        operator_role: CallableOperatorRoleV1,
        infix: CallableInfixV1,
    ) -> Result<Self, CallableSourceEffectsBuildError> {
        if execution == Effect::Suspend && gc_effect == GcEffect::NoGc {
            return Err(CallableSourceEffectsBuildError::NoGcSuspend);
        }
        match implementation {
            CallableImplementationV1::SourceExternScoop => {
                if execution == Effect::Suspend {
                    return Err(CallableSourceEffectsBuildError::SuspendExtern(
                        implementation,
                    ));
                }
            }
            CallableImplementationV1::SourceExternC => {
                if execution == Effect::Suspend {
                    return Err(CallableSourceEffectsBuildError::SuspendExtern(
                        implementation,
                    ));
                }
                if safety != CallableSafetyV1::Unsafe {
                    return Err(CallableSourceEffectsBuildError::SafeCExtern);
                }
                if gc_effect != GcEffect::NoGc {
                    return Err(CallableSourceEffectsBuildError::ManagedCExtern);
                }
            }
            CallableImplementationV1::Intrinsic(kind) => {
                if let Some(expected) = kind.integer_gc_effect() {
                    let expected = match expected {
                        crate::GcEffect::NoGc => GcEffect::NoGc,
                        crate::GcEffect::Managed => GcEffect::Managed,
                    };
                    if gc_effect != expected || execution != Effect::Ordinary {
                        return Err(CallableSourceEffectsBuildError::IntegerIntrinsicEffect {
                            kind,
                            execution,
                            gc_effect,
                        });
                    }
                }
            }
            CallableImplementationV1::Scoop => {}
        }
        Ok(Self {
            execution,
            safety,
            gc_effect,
            implementation,
            operator_role,
            infix,
        })
    }

    pub const fn execution(self) -> Effect {
        self.execution
    }

    pub const fn safety(self) -> CallableSafetyV1 {
        self.safety
    }

    pub const fn gc_effect(self) -> GcEffect {
        self.gc_effect
    }

    /// The Scoop entry for a source extern performs a native transition, even
    /// when the native callee itself promises not to interact with the GC.
    pub const fn provider_entry_gc_effect(self) -> GcEffect {
        match self.implementation {
            CallableImplementationV1::SourceExternC
            | CallableImplementationV1::SourceExternScoop => GcEffect::Managed,
            _ => self.gc_effect,
        }
    }

    pub const fn implementation(self) -> CallableImplementationV1 {
        self.implementation
    }

    pub const fn operator_role(self) -> CallableOperatorRoleV1 {
        self.operator_role
    }

    pub const fn infix(self) -> CallableInfixV1 {
        self.infix
    }
}

impl WireEncode for CallableSourceEffectsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.execution.encode(encoder)?;
        encoder.field(2)?;
        self.safety.encode(encoder)?;
        encoder.field(3)?;
        self.gc_effect.encode(encoder)?;
        encoder.field(4)?;
        self.implementation.encode(encoder)?;
        encoder.field(5)?;
        self.operator_role.encode(encoder)?;
        encoder.field(6)?;
        self.infix.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedCallableSourceEffectsV1 {
    execution: Effect,
    safety: CallableSafetyV1,
    gc_effect: GcEffect,
    implementation: CallableImplementationV1,
    operator_role: CallableOperatorRoleV1,
    infix: CallableInfixV1,
}

impl DecodedCallableSourceEffectsV1 {
    pub fn validate(self) -> Result<CallableSourceEffectsV1, CallableSourceEffectsBuildError> {
        CallableSourceEffectsV1::try_new(
            self.execution,
            self.safety,
            self.gc_effect,
            self.implementation,
            self.operator_role,
            self.infix,
        )
    }
}

impl WireEncode for DecodedCallableSourceEffectsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.execution.encode(encoder)?;
        encoder.field(2)?;
        self.safety.encode(encoder)?;
        encoder.field(3)?;
        self.gc_effect.encode(encoder)?;
        encoder.field(4)?;
        self.implementation.encode(encoder)?;
        encoder.field(5)?;
        self.operator_role.encode(encoder)?;
        encoder.field(6)?;
        self.infix.encode(encoder)
    }
}

impl WireDecode for DecodedCallableSourceEffectsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            execution: decoder.field(1, decode_effect)?,
            safety: decoder.field(2, CallableSafetyV1::decode)?,
            gc_effect: decoder.field(3, GcEffect::decode)?,
            implementation: decoder.field(4, CallableImplementationV1::decode)?,
            operator_role: decoder.field(5, CallableOperatorRoleV1::decode)?,
            infix: decoder.field(6, CallableInfixV1::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableSourceEffectsBuildError {
    NoGcSuspend,
    IntegerIntrinsicEffect {
        kind: crate::IntrinsicFunctionKind,
        execution: Effect,
        gc_effect: GcEffect,
    },
    SuspendExtern(CallableImplementationV1),
    SafeCExtern,
    ManagedCExtern,
}

impl fmt::Display for CallableSourceEffectsBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGcSuspend => formatter.write_str("suspend callable cannot be NoGc"),
            Self::IntegerIntrinsicEffect {
                kind,
                execution,
                gc_effect,
            } => write!(
                formatter,
                "integer intrinsic {kind:?} cannot have {execution:?} execution with {gc_effect:?} GC effect"
            ),
            Self::SuspendExtern(implementation) => {
                write!(formatter, "{implementation:?} callable cannot be suspend")
            }
            Self::SafeCExtern => formatter.write_str("C extern callable must be unsafe"),
            Self::ManagedCExtern => formatter.write_str("C extern callable must be NoGc"),
        }
    }
}

impl std::error::Error for CallableSourceEffectsBuildError {}

fn decode_effect(decoder: &mut Decoder<'_>) -> Result<Effect, WireError> {
    match decoder.unsigned()? {
        1 => Ok(Effect::Ordinary),
        2 => Ok(Effect::Suspend),
        tag => Err(unknown_tag(decoder, tag)),
    }
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
