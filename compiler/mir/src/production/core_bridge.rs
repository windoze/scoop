//! Initialization protocol in core MIR metadata.

use super::*;

/// Required strong implementation of the core-internal initialization cycle
/// service. It has no public export binding and therefore cannot enter source
/// prelude lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreMirInitializationCycleThrowerV1 {
    definition: PersistentFunctionId,
    implementation: CallableOwner,
}

impl CoreMirInitializationCycleThrowerV1 {
    pub fn new(
        definition: PersistentFunctionId,
        implementation: CallableOwner,
    ) -> Result<Self, MirProductionBuildError> {
        if implementation != CallableOwner::Function(definition) {
            return Err(MirProductionBuildError::CoreImplementationMismatch {
                definition,
                implementation,
            });
        }
        Ok(Self {
            definition,
            implementation,
        })
    }

    pub const fn definition(self) -> PersistentFunctionId {
        self.definition
    }

    pub const fn implementation(self) -> CallableOwner {
        self.implementation
    }
}

impl WireEncode for CoreMirInitializationCycleThrowerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedCoreMirInitializationCycleThrowerV1 {
    definition: DecodedPersistentId<PersistentFunctionId>,
    implementation: DecodedCallableOwner,
}

impl WireEncode for DecodedCoreMirInitializationCycleThrowerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

impl WireDecode for DecodedCoreMirInitializationCycleThrowerV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            definition: decoder.field(1, DecodedPersistentId::decode)?,
            implementation: decoder.field(2, DecodedCallableOwner::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreMirBridgeV1 {
    initialization_cycle_thrower: CoreMirInitializationCycleThrowerV1,
}

impl CoreMirBridgeV1 {
    pub const fn new(initialization_cycle_thrower: CoreMirInitializationCycleThrowerV1) -> Self {
        Self {
            initialization_cycle_thrower,
        }
    }

    pub const fn initialization_cycle_thrower(&self) -> CoreMirInitializationCycleThrowerV1 {
        self.initialization_cycle_thrower
    }
}

impl WireEncode for CoreMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(3)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

#[derive(Debug)]
pub(super) struct DecodedCoreMirBridgeV1 {
    initialization_cycle_thrower: DecodedCoreMirInitializationCycleThrowerV1,
}

impl DecodedCoreMirBridgeV1 {
    pub(super) fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CoreMirBridgeV1, MirProductionValidationError> {
        let definition = identities
            .resolve(self.initialization_cycle_thrower.definition)
            .map_err(MirProductionValidationError::Identity)?;
        let implementation = self
            .initialization_cycle_thrower
            .implementation
            .resolve(identities)
            .map_err(MirProductionValidationError::Identity)?;
        let initialization_cycle_thrower =
            CoreMirInitializationCycleThrowerV1::new(definition, implementation)
                .map_err(MirProductionValidationError::Relation)?;
        Ok(CoreMirBridgeV1 {
            initialization_cycle_thrower,
        })
    }
}

impl WireEncode for DecodedCoreMirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(3)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

impl WireDecode for DecodedCoreMirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            initialization_cycle_thrower: decoder
                .field(3, DecodedCoreMirInitializationCycleThrowerV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreMirBridgeBranchV1 {
    NotCore,
    Core(CoreMirBridgeV1),
}

impl WireEncode for CoreMirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(bridge) => encode_value_sum(encoder, 2, bridge),
        }
    }
}

#[derive(Debug)]
pub(super) enum DecodedCoreMirBridgeBranchV1 {
    NotCore,
    Core(DecodedCoreMirBridgeV1),
}

impl WireEncode for DecodedCoreMirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NotCore => encode_empty_sum(encoder, 1),
            Self::Core(bridge) => encode_value_sum(encoder, 2, bridge),
        }
    }
}

impl WireDecode for DecodedCoreMirBridgeBranchV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode_sum(decoder, |tag, decoder| match tag {
            1 => Ok(Self::NotCore),
            2 => DecodedCoreMirBridgeV1::decode(decoder).map(Self::Core),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}
