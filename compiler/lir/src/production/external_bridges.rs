use std::fmt;

use crate::{
    CallableAbiBuildError, CallableAbiDecodeError, CallableAbiRecordV1, CallableAbiValidationError,
    CoreExternalBuildError, DecodedCallableAbiRecordV1, ExternalTypeDescriptor, Module,
    core_type_descriptor_link_contract,
};
use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, IdentityReferenceError,
    ObjectDefinitionPlanId, PersistentExactTypeId, PersistentIdResolver, PersistentSymbolRequest,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExternalTypeDescriptorBridgeV1 {
    target: PersistentExactTypeId,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl StrongExternalTypeDescriptorBridgeV1 {
    pub fn new(target: PersistentExactTypeId) -> Result<Self, CoreExternalBuildError> {
        let (expected_symbol, required_definition) = core_type_descriptor_link_contract(target)?;
        Ok(Self {
            target,
            expected_symbol,
            required_definition,
        })
    }

    fn from_lir(value: &ExternalTypeDescriptor) -> Result<Self, StrongExternalLirBridgeBuildError> {
        let bridge =
            Self::new(value.target()).map_err(StrongExternalLirBridgeBuildError::Contract)?;
        if value.provider() != scoop_identity::ConeIdentity::CORE
            || value.expected_symbol() != bridge.expected_symbol()
            || value.required_definition() != bridge.required_definition()
        {
            return Err(StrongExternalLirBridgeBuildError::InvalidRuntimeStringDescriptor);
        }
        Ok(bridge)
    }

    pub(crate) fn runtime_string(
        module: &Module,
    ) -> Result<Option<Self>, StrongExternalLirBridgeBuildError> {
        match module.meta.well_known_type_descriptors.string {
            crate::TypeDescriptorRef::Local(_) => Ok(None),
            crate::TypeDescriptorRef::External(id) => {
                if id.into_raw().into_u32() as usize >= module.meta.external_type_descriptors.len()
                {
                    return Err(StrongExternalLirBridgeBuildError::MissingRuntimeStringDescriptor);
                }
                Self::from_lir(&module.meta.external_type_descriptors[id]).map(Some)
            }
        }
    }

    pub const fn target(&self) -> PersistentExactTypeId {
        self.target
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

impl WireEncode for StrongExternalTypeDescriptorBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(3)?;
        self.required_definition.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongExternalLirBridgeV1 {
    Callable(CallableAbiRecordV1),
    TypeDescriptor(StrongExternalTypeDescriptorBridgeV1),
}

impl StrongExternalLirBridgeV1 {
    fn sort_key(&self) -> (u8, [u8; 32]) {
        match self {
            Self::Callable(bridge) => (1, *bridge.expected_symbol().key().owner_bytes()),
            Self::TypeDescriptor(bridge) => (2, *bridge.target.as_array()),
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
        if let Some(descriptor) = StrongExternalTypeDescriptorBridgeV1::runtime_string(module)? {
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
            if let StrongExternalLirBridgeV1::Callable(callable) = bridge {
                callable
                    .link_contract(scoop_identity::ConeIdentity::CORE)
                    .map_err(StrongExternalLirBridgeBuildError::CallableContract)?;
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
struct DecodedStrongExternalTypeDescriptorBridgeV1 {
    target: DecodedPersistentId<PersistentExactTypeId>,
    expected_symbol: DecodedPersistentSymbolRequest,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl WireEncode for DecodedStrongExternalTypeDescriptorBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(3)?;
        self.required_definition.encode(encoder)
    }
}

impl WireDecode for DecodedStrongExternalTypeDescriptorBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            target: decoder.field(1, DecodedPersistentId::decode)?,
            expected_symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            required_definition: decoder.field(3, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug)]
enum DecodedStrongExternalLirBridgeV1 {
    Callable(DecodedCallableAbiRecordV1),
    TypeDescriptor(DecodedStrongExternalTypeDescriptorBridgeV1),
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
                .field(1, DecodedStrongExternalTypeDescriptorBridgeV1::decode)
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
                    let target = <ValidatedIdentityGraph as PersistentIdResolver<
                        PersistentExactTypeId,
                    >>::resolve(identities, bridge.target)
                    .map_err(StrongExternalLirBridgeReconstructionError::Identity)?;
                    StrongExternalLirBridgeV1::TypeDescriptor(
                        StrongExternalTypeDescriptorBridgeV1::new(target)
                            .map_err(StrongExternalLirBridgeReconstructionError::Contract)?,
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
    Contract(CoreExternalBuildError),
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
            Self::Contract(error) => Some(error),
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
    Identity(IdentityReferenceError),
    Callable(CallableAbiDecodeError),
    Contract(CoreExternalBuildError),
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
            Self::Identity(error) => error,
            Self::Callable(error) => error,
            Self::Contract(error) => error,
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
