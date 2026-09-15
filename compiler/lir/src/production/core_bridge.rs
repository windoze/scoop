//! Producer-side Scoop ABI authority published by the trusted core artifact.

use std::fmt;

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, DecodedCanonicalScoopAbiFunctionSignature,
    DecodedPersistentId, DecodedPersistentSymbolRequest, DecodedStrongCallableDefinitionOwner,
    GcEffect as CanonicalGcEffect, IdentityReferenceError, ObjectDefinitionPlanId,
    PersistentConstructorId, PersistentExportBindingId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentIdResolver, PersistentPropertyAccessorId,
    PersistentSymbolRequest, PersistentSymbolResolutionError, ScoopAbiResolutionError,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CallingConvention, ConeIdentity, CoreExternalBuildError, CoreExternalCallableRootPlan,
    OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1, core_callable_link_contract,
};

/// Canonical LIR contract for one callable exported by the core Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreLirCallableBridgeV1 {
    binding: PersistentExportBindingId,
    target: StrongCallableDefinitionOwner,
    abi_signature: CanonicalScoopAbiFunctionSignature,
    expected_symbol: PersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: CoreExternalCallableRootPlan,
    required_definition: ObjectDefinitionPlanId,
}

impl CoreLirCallableBridgeV1 {
    pub fn new(
        binding: PersistentExportBindingId,
        target: StrongCallableDefinitionOwner,
        abi_signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        root_plan: CoreExternalCallableRootPlan,
    ) -> Result<Self, CoreLirBridgeBuildError> {
        let expected_effect = match abi_signature.gc_effect() {
            CanonicalGcEffect::Managed => crate::GcEffect::Managed,
            CanonicalGcEffect::NoGc => crate::GcEffect::NoGc,
        };
        if root_plan.gc_effect() != expected_effect {
            return Err(CoreLirBridgeBuildError::Contract(
                CoreExternalBuildError::RootProtocolMismatch,
            ));
        }
        let (_, expected_symbol, required_definition) =
            core_callable_link_contract(target).map_err(CoreLirBridgeBuildError::Contract)?;
        Ok(Self {
            binding,
            target,
            abi_signature,
            expected_symbol,
            calling_convention,
            root_plan,
            required_definition,
        })
    }

    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
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

impl WireEncode for CoreLirCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(4)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(5)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(6)?;
        self.root_plan.encode(encoder)?;
        encoder.field(7)?;
        self.required_definition.encode(encoder)
    }
}

/// Complete canonical Scoop ABI publication surface of the core Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreLirBridgeV1 {
    callables: Vec<CoreLirCallableBridgeV1>,
}

impl CoreLirBridgeV1 {
    pub fn try_new(
        mut callables: Vec<CoreLirCallableBridgeV1>,
    ) -> Result<Self, CoreLirBridgeBuildError> {
        callables.sort_unstable_by_key(CoreLirCallableBridgeV1::binding);
        if let Some(pair) = callables
            .windows(2)
            .find(|pair| pair[0].binding() == pair[1].binding())
        {
            return Err(CoreLirBridgeBuildError::DuplicateBinding(pair[0].binding()));
        }
        if let Some((index, callable)) = callables.iter().enumerate().find(|(index, callable)| {
            callables[..*index]
                .iter()
                .any(|existing| existing.target() == callable.target())
        }) {
            return Err(CoreLirBridgeBuildError::DuplicateTarget {
                index,
                target: callable.target(),
            });
        }
        Ok(Self { callables })
    }

    pub fn callables(&self) -> &[CoreLirCallableBridgeV1] {
        &self.callables
    }

    pub fn callable(&self, binding: PersistentExportBindingId) -> Option<&CoreLirCallableBridgeV1> {
        self.callables
            .binary_search_by_key(&binding, CoreLirCallableBridgeV1::binding)
            .ok()
            .map(|index| &self.callables[index])
    }

    fn validate_foundation(
        &self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CoreLirBridgeBuildError> {
        for callable in &self.callables {
            let (body, expected_symbol, required_definition) =
                core_callable_link_contract(callable.target())
                    .map_err(CoreLirBridgeBuildError::Contract)?;
            if !foundation.contains_callable_body(body) {
                return Err(CoreLirBridgeBuildError::MissingBody(body));
            }
            if !foundation.contains_symbol_request(expected_symbol) {
                return Err(CoreLirBridgeBuildError::MissingSymbol(expected_symbol));
            }
            let plan = definitions.plan(required_definition).ok_or(
                CoreLirBridgeBuildError::MissingDefinition(required_definition),
            )?;
            if plan.owner() != StrongDefinitionEntity::callable_body(body)
                || plan.definition_role() != StrongDefinitionRole::CallableBody
                || plan.primary_symbol() != expected_symbol
            {
                return Err(CoreLirBridgeBuildError::DefinitionMismatch(
                    required_definition,
                ));
            }
        }
        Ok(())
    }
}

impl WireEncode for CoreLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.callables.len() as u64)?;
        for callable in &self.callables {
            callable.encode(encoder)?;
        }
        Ok(())
    }
}

/// Closed producer-kind branch. Only the core Cone may publish a core bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreLirBridgeBranchV1 {
    NotCore,
    Core(CoreLirBridgeV1),
}

impl CoreLirBridgeBranchV1 {
    pub fn validate_against(
        &self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CoreLirBridgeBuildError> {
        match (foundation.producer() == ConeIdentity::CORE, self) {
            (false, Self::NotCore) => Ok(()),
            (true, Self::Core(bridge)) => bridge.validate_foundation(foundation, definitions),
            _ => Err(CoreLirBridgeBuildError::ProducerBranchMismatch),
        }
    }

    pub const fn core(&self) -> Option<&CoreLirBridgeV1> {
        match self {
            Self::NotCore => None,
            Self::Core(bridge) => Some(bridge),
        }
    }
}

impl WireEncode for CoreLirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        if let Self::Core(bridge) = self {
            encoder.field(1)?;
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DecodedCoreLirCallableBridgeV1 {
    binding: DecodedPersistentId<PersistentExportBindingId>,
    target: DecodedStrongCallableDefinitionOwner,
    abi_signature: DecodedCanonicalScoopAbiFunctionSignature,
    expected_symbol: DecodedPersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: CoreExternalCallableRootPlan,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl WireEncode for DecodedCoreLirCallableBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(4)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(5)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(6)?;
        self.root_plan.encode(encoder)?;
        encoder.field(7)?;
        self.required_definition.encode(encoder)
    }
}

impl WireDecode for DecodedCoreLirCallableBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            binding: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
            abi_signature: decoder.field(3, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            expected_symbol: decoder.field(4, DecodedPersistentSymbolRequest::decode)?,
            calling_convention: decoder.field(5, CallingConvention::decode)?,
            root_plan: decoder.field(6, CoreExternalCallableRootPlan::decode)?,
            required_definition: decoder.field(7, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedCoreLirBridgeV1 {
    callables: Vec<DecodedCoreLirCallableBridgeV1>,
}

impl WireEncode for DecodedCoreLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.callables.len() as u64)?;
        for callable in &self.callables {
            callable.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreLirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCoreLirCallableBridgeV1::decode(decoder))
            .map(|callables| Self { callables })
    }
}

#[derive(Debug)]
pub enum DecodedCoreLirBridgeBranchV1 {
    NotCore,
    Core(DecodedCoreLirBridgeV1),
}

impl DecodedCoreLirBridgeBranchV1 {
    pub fn validate(
        self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CoreLirBridgeBranchV1, CoreLirBridgeValidationError> {
        let branch = match self {
            Self::NotCore => CoreLirBridgeBranchV1::NotCore,
            Self::Core(decoded) => {
                let mut callables = Vec::with_capacity(decoded.callables.len());
                for decoded in decoded.callables {
                    let binding = identities
                        .resolve(decoded.binding)
                        .map_err(CoreLirBridgeValidationError::Identity)?;
                    let target = resolve_target(decoded.target, identities)
                        .map_err(CoreLirBridgeValidationError::Identity)?;
                    let abi_signature = decoded
                        .abi_signature
                        .resolve(identities)
                        .map_err(CoreLirBridgeValidationError::Abi)?;
                    let expected_symbol = decoded
                        .expected_symbol
                        .resolve(identities)
                        .map_err(CoreLirBridgeValidationError::Symbol)?;
                    let required_definition = identities
                        .resolve(decoded.required_definition)
                        .map_err(CoreLirBridgeValidationError::Identity)?;
                    let callable = CoreLirCallableBridgeV1::new(
                        binding,
                        target,
                        abi_signature,
                        decoded.calling_convention,
                        decoded.root_plan,
                    )
                    .map_err(CoreLirBridgeValidationError::Build)?;
                    if callable.expected_symbol() != expected_symbol
                        || callable.required_definition() != required_definition
                    {
                        return Err(CoreLirBridgeValidationError::LinkContractMismatch(binding));
                    }
                    callables.push(callable);
                }
                let original = callables.clone();
                let bridge = CoreLirBridgeV1::try_new(callables)
                    .map_err(CoreLirBridgeValidationError::Build)?;
                if original != bridge.callables {
                    return Err(CoreLirBridgeValidationError::NonCanonicalOrder);
                }
                CoreLirBridgeBranchV1::Core(bridge)
            }
        };
        branch
            .validate_against(foundation, definitions)
            .map_err(CoreLirBridgeValidationError::Build)?;
        Ok(branch)
    }
}

impl WireEncode for DecodedCoreLirBridgeBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::NotCore => 1,
            Self::Core(_) => 2,
        })?;
        if let Self::Core(bridge) = self {
            encoder.field(1)?;
            bridge.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreLirBridgeBranchV1 {
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
        match (tag, fields) {
            (1, 1) => Ok(Self::NotCore),
            (2, 2) => decoder
                .field(1, DecodedCoreLirBridgeV1::decode)
                .map(Self::Core),
            (1 | 2, actual) => Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: if tag == 1 { 1 } else { 2 },
                    actual,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
            (tag, _) => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

fn resolve_target(
    target: DecodedStrongCallableDefinitionOwner,
    identities: &mut ValidatedIdentityGraph,
) -> Result<StrongCallableDefinitionOwner, IdentityReferenceError> {
    Ok(match target {
        DecodedStrongCallableDefinitionOwner::Function(id) => {
            StrongCallableDefinitionOwner::Function(
                <ValidatedIdentityGraph as PersistentIdResolver<PersistentFunctionId>>::resolve(
                    identities, id,
                )?,
            )
        }
        DecodedStrongCallableDefinitionOwner::Constructor(id) => {
            StrongCallableDefinitionOwner::Constructor(
                <ValidatedIdentityGraph as PersistentIdResolver<PersistentConstructorId>>::resolve(
                    identities, id,
                )?,
            )
        }
        DecodedStrongCallableDefinitionOwner::PropertyAccessor(id) => {
            StrongCallableDefinitionOwner::PropertyAccessor(
                <ValidatedIdentityGraph as PersistentIdResolver<
                    PersistentPropertyAccessorId,
                >>::resolve(identities, id)?,
            )
        }
        DecodedStrongCallableDefinitionOwner::GeneratedCallable(id) => {
            StrongCallableDefinitionOwner::GeneratedCallable(
                <ValidatedIdentityGraph as PersistentIdResolver<
                    PersistentGeneratedCallableId,
                >>::resolve(identities, id)?,
            )
        }
    })
}

#[derive(Debug)]
pub enum CoreLirBridgeBuildError {
    Contract(CoreExternalBuildError),
    DuplicateBinding(PersistentExportBindingId),
    DuplicateTarget {
        index: usize,
        target: StrongCallableDefinitionOwner,
    },
    ProducerBranchMismatch,
    MissingBody(scoop_identity::PersistentCallableBodyId),
    MissingSymbol(PersistentSymbolRequest),
    MissingDefinition(ObjectDefinitionPlanId),
    DefinitionMismatch(ObjectDefinitionPlanId),
}

impl fmt::Display for CoreLirBridgeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core LIR bridge: {self:?}")
    }
}

impl std::error::Error for CoreLirBridgeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum CoreLirBridgeValidationError {
    Identity(IdentityReferenceError),
    Symbol(PersistentSymbolResolutionError<IdentityReferenceError>),
    Abi(ScoopAbiResolutionError<IdentityReferenceError>),
    Build(CoreLirBridgeBuildError),
    LinkContractMismatch(PersistentExportBindingId),
    NonCanonicalOrder,
}

impl fmt::Display for CoreLirBridgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid decoded core LIR bridge: {self:?}")
    }
}

impl std::error::Error for CoreLirBridgeValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Symbol(error) => Some(error),
            Self::Abi(error) => Some(error),
            Self::Build(error) => Some(error),
            Self::LinkContractMismatch(_) | Self::NonCanonicalOrder => None,
        }
    }
}

#[cfg(test)]
mod tests;
