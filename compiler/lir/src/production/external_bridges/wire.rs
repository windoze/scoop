use super::*;
use crate::{DecodedExternalTypeDescriptor, DecodedSelectedDependencyLirCallableV1};
use scoop_identity::ValidatedIdentityGraph;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

impl WireEncode for StrongExternalLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 3,
            Self::TypeDescriptor(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(bridge) => bridge.encode(encoder),
            Self::TypeDescriptor(bridge) => bridge.encode(encoder),
        }
    }
}

impl WireEncode for StrongExternalLirBridgeSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.bridges.len() as u64)?;
        for bridge in &self.bridges {
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) enum DecodedStrongExternalLirBridgeV1 {
    Callable(Box<DecodedSelectedDependencyLirCallableV1>),
    TypeDescriptor(DecodedExternalTypeDescriptor),
}

impl WireEncode for DecodedStrongExternalLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 3,
            Self::TypeDescriptor(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(bridge) => bridge.encode(encoder),
            Self::TypeDescriptor(bridge) => bridge.encode(encoder),
        }
    }
}

impl WireDecode for DecodedStrongExternalLirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(WireError::new(
                WireErrorKind::MissingField { field: 0 },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        if fields != 2 {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        match tag {
            3 => decoder
                .field(1, DecodedSelectedDependencyLirCallableV1::decode)
                .map(Box::new)
                .map(Self::Callable),
            2 => decoder
                .field(1, DecodedExternalTypeDescriptor::decode)
                .map(Self::TypeDescriptor),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DecodedStrongExternalLirBridgeSurfaceV1 {
    pub(super) bridges: Vec<DecodedStrongExternalLirBridgeV1>,
}

impl DecodedStrongExternalLirBridgeSurfaceV1 {
    /// Reconstructs the typed external bridge authority solely from decoded
    /// references that already belong to one validated identity graph.
    /// Contract-derived symbols and definition ids are recomputed rather than
    /// trusted from the artifact bytes.
    pub fn reconstruct(
        &self,
        producer: scoop_identity::ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeReconstructionError> {
        let mut bridges = Vec::with_capacity(self.bridges.len());
        for bridge in &self.bridges {
            bridges.push(match bridge {
                DecodedStrongExternalLirBridgeV1::Callable(bridge) => {
                    StrongExternalLirBridgeV1::Callable(Box::new(
                        bridge
                            .clone()
                            .reconstruct(identities)
                            .map_err(StrongExternalLirBridgeReconstructionError::Callable)?,
                    ))
                }
                DecodedStrongExternalLirBridgeV1::TypeDescriptor(bridge) => {
                    StrongExternalLirBridgeV1::TypeDescriptor(
                        bridge
                            .clone()
                            .validate(identities)
                            .map_err(StrongExternalLirBridgeReconstructionError::TypeDescriptor)?,
                    )
                }
            });
        }
        StrongExternalLirBridgeSurfaceV1::try_new(producer, bridges)
            .map_err(StrongExternalLirBridgeReconstructionError::Surface)
    }

    /// Validate untrusted bytes against an independently reconstructed typed
    /// surface. The returned value is the trusted reconstruction, never a
    /// cast of decoded identities.
    pub fn validate_against(
        self,
        expected: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeValidationError> {
        let actual_bytes = encode(&self).map_err(StrongExternalLirBridgeValidationError::Wire)?;
        let expected_bytes =
            encode(expected).map_err(StrongExternalLirBridgeValidationError::Wire)?;
        if actual_bytes != expected_bytes {
            return Err(StrongExternalLirBridgeValidationError::SurfaceMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedStrongExternalLirBridgeSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.bridges.len() as u64)?;
        for bridge in &self.bridges {
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedStrongExternalLirBridgeSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedStrongExternalLirBridgeV1::decode(decoder))
            .map(|bridges| Self { bridges })
    }
}
