use std::fmt;

use crate::{
    CallableAbiBuildError, CallableAbiDecodeError, CallableAbiRecordV1, CallableAbiValidationError,
    DecodedCallableAbiRecordV1, DecodedExternalTypeDescriptor, ExternalTypeDescriptor,
    ExternalTypeDescriptorDecodeError, ExternalTypeDescriptorValidationError, Module,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

mod runtime_string;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongExternalLirBridgeV1 {
    Callable(CallableAbiRecordV1),
    TypeDescriptor(ExternalTypeDescriptor),
}

impl StrongExternalLirBridgeV1 {
    fn sort_key(&self) -> (u8, [u8; 32]) {
        match self {
            Self::Callable(bridge) => (1, *bridge.expected_symbol().key().owner_bytes()),
            Self::TypeDescriptor(bridge) => (2, *bridge.target().as_array()),
        }
    }
}

impl WireEncode for StrongExternalLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::TypeDescriptor(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(bridge) => bridge.encode(encoder),
            Self::TypeDescriptor(bridge) => bridge.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExternalLirBridgeSurfaceV1 {
    producer: scoop_identity::ConeIdentity,
    bridges: Vec<StrongExternalLirBridgeV1>,
}

impl StrongExternalLirBridgeSurfaceV1 {
    pub const fn empty_core_bootstrap() -> Self {
        Self {
            producer: scoop_identity::ConeIdentity::CORE,
            bridges: Vec::new(),
        }
    }

    pub fn from_module(module: &Module) -> Result<Self, StrongExternalLirBridgeBuildError> {
        let mut bridges = Vec::with_capacity(module.meta.external_callables.len() + 1);
        for (_, callable) in module.meta.external_callables.iter() {
            if callable.origin() != crate::ExternalCallableOrigin::InitializationCycle {
                continue;
            }
            bridges.push(StrongExternalLirBridgeV1::Callable(
                CallableAbiRecordV1::from_lir(callable)
                    .map_err(StrongExternalLirBridgeBuildError::Callable)?,
            ));
        }
        if let Some(descriptor) = Self::runtime_string(module)? {
            bridges.push(StrongExternalLirBridgeV1::TypeDescriptor(descriptor));
        }
        Self::try_new(module.cone, bridges)
    }

    pub fn try_new(
        producer: scoop_identity::ConeIdentity,
        mut bridges: Vec<StrongExternalLirBridgeV1>,
    ) -> Result<Self, StrongExternalLirBridgeBuildError> {
        if producer == scoop_identity::ConeIdentity::CORE && !bridges.is_empty() {
            return Err(StrongExternalLirBridgeBuildError::CoreBootstrapImport);
        }
        for bridge in &bridges {
            match bridge {
                StrongExternalLirBridgeV1::Callable(callable) => {
                    callable
                        .link_contract(scoop_identity::ConeIdentity::CORE)
                        .map_err(StrongExternalLirBridgeBuildError::CallableContract)?;
                }
                StrongExternalLirBridgeV1::TypeDescriptor(descriptor) => {
                    Self::validate_runtime_string(*descriptor)?;
                }
            }
        }
        bridges.sort_unstable_by_key(StrongExternalLirBridgeV1::sort_key);
        if let Some(pair) = bridges
            .windows(2)
            .find(|pair| pair[0].sort_key() == pair[1].sort_key())
        {
            return Err(StrongExternalLirBridgeBuildError::DuplicateTarget(
                pair[0].sort_key(),
            ));
        }
        Ok(Self { producer, bridges })
    }

    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.producer
    }

    pub fn bridges(&self) -> &[StrongExternalLirBridgeV1] {
        &self.bridges
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
enum DecodedStrongExternalLirBridgeV1 {
    Callable(DecodedCallableAbiRecordV1),
    TypeDescriptor(DecodedExternalTypeDescriptor),
}

impl WireEncode for DecodedStrongExternalLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
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
            1 => decoder
                .field(1, DecodedCallableAbiRecordV1::decode)
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
    bridges: Vec<DecodedStrongExternalLirBridgeV1>,
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
                    StrongExternalLirBridgeV1::Callable(
                        bridge
                            .clone()
                            .validate(scoop_identity::ConeIdentity::CORE, identities)
                            .map_err(StrongExternalLirBridgeReconstructionError::Callable)?,
                    )
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongExternalLirBridgeBuildError {
    Callable(CallableAbiBuildError),
    CallableContract(CallableAbiValidationError),
    TypeDescriptor(ExternalTypeDescriptorValidationError),
    CoreBootstrapImport,
    MissingRuntimeStringDescriptor,
    InvalidRuntimeStringDescriptor,
    DuplicateTarget((u8, [u8; 32])),
}

impl fmt::Display for StrongExternalLirBridgeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot build strong external LIR bridge surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongExternalLirBridgeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TypeDescriptor(error) => Some(error),
            Self::Callable(error) => Some(error),
            Self::CallableContract(error) => Some(error),
            Self::CoreBootstrapImport
            | Self::MissingRuntimeStringDescriptor
            | Self::InvalidRuntimeStringDescriptor
            | Self::DuplicateTarget(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum StrongExternalLirBridgeValidationError {
    Wire(scoop_wire::cbor::EncodeError),
    SurfaceMismatch,
}

#[derive(Debug)]
pub enum StrongExternalLirBridgeReconstructionError {
    Callable(CallableAbiDecodeError),
    TypeDescriptor(ExternalTypeDescriptorDecodeError),
    Surface(StrongExternalLirBridgeBuildError),
}

impl fmt::Display for StrongExternalLirBridgeReconstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot reconstruct strong external LIR bridge surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongExternalLirBridgeReconstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Callable(error) => error,
            Self::TypeDescriptor(error) => error,
            Self::Surface(error) => error,
        })
    }
}

impl fmt::Display for StrongExternalLirBridgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong external LIR bridge surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongExternalLirBridgeValidationError {}

#[cfg(test)]
mod tests;
