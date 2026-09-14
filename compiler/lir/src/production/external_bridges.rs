use std::fmt;

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, DecodedCanonicalScoopAbiFunctionSignature,
    DecodedPersistentId, DecodedPersistentSymbolRequest, DecodedStrongCallableDefinitionOwner,
    ObjectDefinitionPlanId, PersistentExactTypeId, PersistentSymbolRequest,
    StrongCallableDefinitionOwner,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use crate::{
    CallingConvention, CoreExternalBuildError, CoreExternalCallable, CoreExternalCallableRootPlan,
    CoreExternalTypeDescriptor, Module, core_callable_link_contract,
    core_type_descriptor_link_contract,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExternalCallableBridgeV1 {
    target: StrongCallableDefinitionOwner,
    abi_signature: CanonicalScoopAbiFunctionSignature,
    expected_symbol: PersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: CoreExternalCallableRootPlan,
    required_definition: ObjectDefinitionPlanId,
}

impl StrongExternalCallableBridgeV1 {
    pub fn new(
        target: StrongCallableDefinitionOwner,
        abi_signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        root_plan: CoreExternalCallableRootPlan,
    ) -> Result<Self, CoreExternalBuildError> {
        let (_, expected_symbol, required_definition) = core_callable_link_contract(target)?;
        Ok(Self {
            target,
            abi_signature,
            expected_symbol,
            calling_convention,
            root_plan,
            required_definition,
        })
    }

    fn from_lir(value: &CoreExternalCallable) -> Result<Self, CoreExternalBuildError> {
        Self::new(
            value.target(),
            value.canonical_signature().clone(),
            value.calling_convention(),
            value.root_plan(),
        )
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn abi_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.abi_signature
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub const fn root_plan(&self) -> CoreExternalCallableRootPlan {
        self.root_plan
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

impl WireEncode for StrongExternalCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(3)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(4)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(5)?;
        self.root_plan.encode(encoder)?;
        encoder.field(6)?;
        self.required_definition.encode(encoder)
    }
}

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

    fn from_lir(value: &CoreExternalTypeDescriptor) -> Result<Self, CoreExternalBuildError> {
        Self::new(value.target())
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
    Callable(StrongExternalCallableBridgeV1),
    TypeDescriptor(StrongExternalTypeDescriptorBridgeV1),
}

impl StrongExternalLirBridgeV1 {
    fn sort_key(&self) -> (u8, [u8; 32]) {
        match self {
            Self::Callable(bridge) => (1, *bridge.expected_symbol.key().owner_bytes()),
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
        let mut bridges = Vec::with_capacity(
            module.meta.core_external_callables.len()
                + module.meta.core_external_type_descriptors.len(),
        );
        for (_, callable) in module.meta.core_external_callables.iter() {
            bridges.push(StrongExternalLirBridgeV1::Callable(
                StrongExternalCallableBridgeV1::from_lir(callable)
                    .map_err(StrongExternalLirBridgeBuildError::Contract)?,
            ));
        }
        for (_, descriptor) in module.meta.core_external_type_descriptors.iter() {
            bridges.push(StrongExternalLirBridgeV1::TypeDescriptor(
                StrongExternalTypeDescriptorBridgeV1::from_lir(descriptor)
                    .map_err(StrongExternalLirBridgeBuildError::Contract)?,
            ));
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

#[derive(Debug)]
struct DecodedStrongExternalCallableBridgeV1 {
    target: DecodedStrongCallableDefinitionOwner,
    abi_signature: DecodedCanonicalScoopAbiFunctionSignature,
    expected_symbol: DecodedPersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: CoreExternalCallableRootPlan,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl WireEncode for DecodedStrongExternalCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(3)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(4)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(5)?;
        self.root_plan.encode(encoder)?;
        encoder.field(6)?;
        self.required_definition.encode(encoder)
    }
}

impl WireDecode for DecodedStrongExternalCallableBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            target: decoder.field(1, DecodedStrongCallableDefinitionOwner::decode)?,
            abi_signature: decoder.field(2, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            expected_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            calling_convention: decoder.field(4, CallingConvention::decode)?,
            root_plan: decoder.field(5, CoreExternalCallableRootPlan::decode)?,
            required_definition: decoder.field(6, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
enum DecodedStrongExternalLirBridgeV1 {
    Callable(DecodedStrongExternalCallableBridgeV1),
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
                .field(1, DecodedStrongExternalCallableBridgeV1::decode)
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

#[derive(Debug)]
pub struct DecodedStrongExternalLirBridgeSurfaceV1 {
    bridges: Vec<DecodedStrongExternalLirBridgeV1>,
}

impl DecodedStrongExternalLirBridgeSurfaceV1 {
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

#[derive(Debug)]
pub enum StrongExternalLirBridgeBuildError {
    Contract(CoreExternalBuildError),
    CoreBootstrapImport,
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
            Self::CoreBootstrapImport | Self::DuplicateTarget(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum StrongExternalLirBridgeValidationError {
    Wire(scoop_wire::cbor::EncodeError),
    SurfaceMismatch,
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
