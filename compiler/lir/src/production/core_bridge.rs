//! Producer-side Scoop ABI authority published by the trusted core artifact.

use std::fmt;

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, DecodedCanonicalScoopAbiFunctionSignature,
    DecodedPersistentId, DecodedPersistentSymbolRequest, DecodedStrongCallableDefinitionOwner,
    GcEffect as CanonicalGcEffect, IdentityReferenceError, ObjectDefinitionPlanId,
    PersistentConstructorId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentIdResolver, PersistentPropertyAccessorId, PersistentSymbolRequest,
    PersistentSymbolResolutionError, ScoopAbiResolutionError, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CallingConvention, ConeIdentity, CoreExternalBuildError, CoreExternalCallableRootPlan,
    OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1, core_callable_link_contract,
};

/// Canonical LIR contract for the core-internal initialization cycle service.
/// It intentionally has no public export binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreLirInitializationCycleThrowerV1 {
    target: StrongCallableDefinitionOwner,
    abi_signature: CanonicalScoopAbiFunctionSignature,
    expected_symbol: PersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: CoreExternalCallableRootPlan,
    required_definition: ObjectDefinitionPlanId,
}

impl CoreLirInitializationCycleThrowerV1 {
    pub fn new(
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
            target,
            abi_signature,
            expected_symbol,
            calling_convention,
            root_plan,
            required_definition,
        })
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

impl WireEncode for CoreLirInitializationCycleThrowerV1 {
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

#[cfg(test)]
pub(crate) fn core_lir_cycle_thrower_for_test() -> CoreLirInitializationCycleThrowerV1 {
    let declaration = scoop_identity::SourceDeclarationKey::function(
        scoop_identity::SourceDeclarationSite::new(
            ConeIdentity::CORE,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap(),
        scoop_identity::CanonicalIdentifier::new("__scoopThrowInitializationCycle").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let definition = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let string_declaration = scoop_identity::CborIdentityRecord::from_key(
        scoop_identity::SourceDeclarationKey::nominal(
            scoop_identity::SourceDeclarationSite::new(
                ConeIdentity::CORE,
                scoop_identity::PackagePath::root(),
                scoop_identity::DefinitionOwnerChain::top_level(),
                scoop_identity::DeclarationScope::ConeWide,
            )
            .unwrap(),
            scoop_identity::CanonicalIdentifier::new("String").unwrap(),
            scoop_identity::SourceNominalKind::Class,
            0,
        ),
    )
    .unwrap();
    let string = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(string_declaration.id()),
    )
    .unwrap();
    let unit =
        scoop_identity::PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
    let signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![string],
        unit,
    );
    let string_storage = scoop_identity::CanonicalScoopStorage::new(
        string,
        8,
        std::num::NonZeroU64::new(8).unwrap(),
        scoop_identity::ScoopAbiValueShape::Scalar,
    );
    let abi = CanonicalScoopAbiFunctionSignature::new(
        signature,
        vec![scoop_identity::ScoopAbiArgument::direct(string_storage).unwrap()],
        scoop_identity::ScoopAbiReturn::unit_void(),
        CanonicalGcEffect::Managed,
    )
    .unwrap();
    CoreLirInitializationCycleThrowerV1::new(
        StrongCallableDefinitionOwner::Function(definition),
        abi,
        CallingConvention::Cdecl,
        CoreExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap()
}

/// Complete canonical Scoop ABI publication surface of the core Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreLirBridgeV1 {
    initialization_cycle_thrower: Box<CoreLirInitializationCycleThrowerV1>,
}

impl CoreLirBridgeV1 {
    pub fn new(initialization_cycle_thrower: CoreLirInitializationCycleThrowerV1) -> Self {
        Self {
            initialization_cycle_thrower: Box::new(initialization_cycle_thrower),
        }
    }

    pub fn initialization_cycle_thrower(&self) -> &CoreLirInitializationCycleThrowerV1 {
        &self.initialization_cycle_thrower
    }

    fn validate_foundation(
        &self,
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectSymbolSurfaceV1,
    ) -> Result<(), CoreLirBridgeBuildError> {
        let cycle = &self.initialization_cycle_thrower;
        let (body, expected_symbol, required_definition) =
            core_callable_link_contract(cycle.target())
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
        Ok(())
    }
}

impl WireEncode for CoreLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(2)?;
        self.initialization_cycle_thrower.encode(encoder)
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
struct DecodedCoreLirInitializationCycleThrowerV1 {
    target: DecodedStrongCallableDefinitionOwner,
    abi_signature: DecodedCanonicalScoopAbiFunctionSignature,
    expected_symbol: DecodedPersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: CoreExternalCallableRootPlan,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl WireEncode for DecodedCoreLirInitializationCycleThrowerV1 {
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

impl WireDecode for DecodedCoreLirInitializationCycleThrowerV1 {
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
pub struct DecodedCoreLirBridgeV1 {
    initialization_cycle_thrower: Box<DecodedCoreLirInitializationCycleThrowerV1>,
}

impl WireEncode for DecodedCoreLirBridgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(2)?;
        self.initialization_cycle_thrower.encode(encoder)
    }
}

impl WireDecode for DecodedCoreLirBridgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            initialization_cycle_thrower: Box::new(
                decoder.field(2, DecodedCoreLirInitializationCycleThrowerV1::decode)?,
            ),
        })
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
                let decoded_cycle = *decoded.initialization_cycle_thrower;
                let cycle_target = resolve_target(decoded_cycle.target, identities)
                    .map_err(CoreLirBridgeValidationError::Identity)?;
                let cycle_abi = decoded_cycle
                    .abi_signature
                    .resolve(identities)
                    .map_err(CoreLirBridgeValidationError::Abi)?;
                let cycle_symbol = decoded_cycle
                    .expected_symbol
                    .resolve(identities)
                    .map_err(CoreLirBridgeValidationError::Symbol)?;
                let cycle_definition = identities
                    .resolve(decoded_cycle.required_definition)
                    .map_err(CoreLirBridgeValidationError::Identity)?;
                let cycle = CoreLirInitializationCycleThrowerV1::new(
                    cycle_target,
                    cycle_abi,
                    decoded_cycle.calling_convention,
                    decoded_cycle.root_plan,
                )
                .map_err(CoreLirBridgeValidationError::Build)?;
                if cycle.expected_symbol() != cycle_symbol
                    || cycle.required_definition() != cycle_definition
                {
                    return Err(
                        CoreLirBridgeValidationError::InitializationCycleLinkContractMismatch,
                    );
                }
                CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(cycle))
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
    InitializationCycleLinkContractMismatch,
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
            Self::InitializationCycleLinkContractMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests;
