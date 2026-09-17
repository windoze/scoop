//! Typed external references admitted by the M23-3 trusted-core bridge.

use std::fmt;

pub use scoop_identity::ConeIdentity;
use scoop_identity::{
    CallableBodyKey, CanonicalScoopAbiFunctionSignature, GcEffect as CanonicalGcEffect,
    LinkageClass, ObjectDefinitionIdentityError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentSymbolError, PersistentSymbolKey,
    PersistentSymbolRequest, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CallingConvention, GcEffect, ScoopAbiSignature};

use crate::external_callable_abi::{CanonicalScoopAbiMismatch, validate_canonical_scoop_abi};

/// The caller-side root protocol inseparably paired with a core callable's
/// GC effect. A managed call must be emitted as a statepoint; a no-GC call has
/// no caller-root publication step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreExternalCallableRootPlan {
    ManagedStatepoint,
    NoGc,
}

impl CoreExternalCallableRootPlan {
    pub const fn gc_effect(self) -> GcEffect {
        match self {
            Self::ManagedStatepoint => GcEffect::Managed,
            Self::NoGc => GcEffect::NoGc,
        }
    }
}

impl WireEncode for CoreExternalCallableRootPlan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ManagedStatepoint => 1,
            Self::NoGc => 2,
        })
    }
}

impl WireDecode for CoreExternalCallableRootPlan {
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

/// A callable definition imported exclusively from the trusted core Cone.
/// Link spelling is derived from the persistent request and is never stored
/// as an independent string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreExternalCallable {
    target: StrongCallableDefinitionOwner,
    body: PersistentCallableBodyId,
    canonical_signature: CanonicalScoopAbiFunctionSignature,
    signature: ScoopAbiSignature,
    root_plan: CoreExternalCallableRootPlan,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl CoreExternalCallable {
    pub(crate) fn new(
        target: StrongCallableDefinitionOwner,
        canonical_signature: CanonicalScoopAbiFunctionSignature,
        signature: ScoopAbiSignature,
        root_plan: CoreExternalCallableRootPlan,
    ) -> Result<Self, CoreExternalBuildError> {
        validate_canonical_scoop_abi(&canonical_signature, &signature).map_err(
            |error| match error {
                CanonicalScoopAbiMismatch::ArgumentCount { expected, actual } => {
                    CoreExternalBuildError::AbiArgumentCount { expected, actual }
                }
                CanonicalScoopAbiMismatch::Argument { index } => {
                    CoreExternalBuildError::AbiArgumentMismatch { index }
                }
                CanonicalScoopAbiMismatch::Result => CoreExternalBuildError::AbiResultMismatch,
            },
        )?;
        let expected_effect = match canonical_signature.gc_effect() {
            CanonicalGcEffect::Managed => GcEffect::Managed,
            CanonicalGcEffect::NoGc => GcEffect::NoGc,
        };
        if root_plan.gc_effect() != expected_effect {
            return Err(CoreExternalBuildError::RootProtocolMismatch);
        }
        let (body, expected_symbol, required_definition) = core_callable_link_contract(target)?;
        Ok(Self {
            target,
            body,
            canonical_signature,
            signature,
            root_plan,
            expected_symbol,
            required_definition,
        })
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn canonical_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.canonical_signature
    }

    pub const fn signature(&self) -> &ScoopAbiSignature {
        &self.signature
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.signature.calling_convention()
    }

    pub const fn root_plan(&self) -> CoreExternalCallableRootPlan {
        self.root_plan
    }

    pub const fn gc_effect(&self) -> GcEffect {
        self.root_plan.gc_effect()
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

/// A TypeDescriptor definition imported exclusively from the trusted core
/// Cone. The persistent exact type is the semantic target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreExternalTypeDescriptor {
    target: PersistentExactTypeId,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl CoreExternalTypeDescriptor {
    pub fn new(target: PersistentExactTypeId) -> Result<Self, CoreExternalBuildError> {
        let (expected_symbol, required_definition) = core_type_descriptor_link_contract(target)?;
        Ok(Self {
            target,
            expected_symbol,
            required_definition,
        })
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

fn required_definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<ObjectDefinitionPlanId, CoreExternalBuildError> {
    let key = ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role)
        .map_err(CoreExternalBuildError::Definition)?;
    ObjectDefinitionPlanId::from_key(&key).map_err(CoreExternalBuildError::Identity)
}

pub(crate) fn core_callable_link_contract(
    target: StrongCallableDefinitionOwner,
) -> Result<
    (
        PersistentCallableBodyId,
        PersistentSymbolRequest,
        ObjectDefinitionPlanId,
    ),
    CoreExternalBuildError,
> {
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))
        .map_err(CoreExternalBuildError::Identity)?;
    let expected_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body),
        LinkageClass::ConeStrong,
    )
    .map_err(CoreExternalBuildError::Symbol)?;
    let required_definition = required_definition(
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableBody,
    )?;
    Ok((body, expected_symbol, required_definition))
}

pub(crate) fn core_type_descriptor_link_contract(
    target: PersistentExactTypeId,
) -> Result<(PersistentSymbolRequest, ObjectDefinitionPlanId), CoreExternalBuildError> {
    let expected_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeDescriptor(target),
        LinkageClass::ConeStrong,
    )
    .map_err(CoreExternalBuildError::Symbol)?;
    let required_definition = required_definition(
        StrongDefinitionEntity::exact_type(target),
        StrongDefinitionRole::TypeDescriptor,
    )?;
    Ok((expected_symbol, required_definition))
}

#[derive(Debug)]
pub enum CoreExternalBuildError {
    AbiArgumentCount { expected: usize, actual: usize },
    AbiArgumentMismatch { index: usize },
    AbiResultMismatch,
    RootProtocolMismatch,
    Identity(HashError),
    Symbol(PersistentSymbolError),
    Definition(ObjectDefinitionIdentityError),
}

impl fmt::Display for CoreExternalBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiArgumentCount { expected, actual } => write!(
                formatter,
                "core external source signature requires {expected} logical ABI arguments, found {actual}"
            ),
            Self::AbiArgumentMismatch { index } => write!(
                formatter,
                "core external canonical and physical ABI disagree at argument {index}"
            ),
            Self::AbiResultMismatch => {
                formatter.write_str("core external canonical and physical result ABI disagree")
            }
            Self::RootProtocolMismatch => formatter.write_str(
                "core external caller root protocol disagrees with its canonical GC effect",
            ),
            Self::Identity(error) => {
                write!(formatter, "cannot derive core external identity: {error}")
            }
            Self::Symbol(error) => write!(formatter, "cannot derive core external symbol: {error}"),
            Self::Definition(error) => {
                write!(formatter, "cannot derive core external definition: {error}")
            }
        }
    }
}

impl std::error::Error for CoreExternalBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AbiArgumentCount { .. }
            | Self::AbiArgumentMismatch { .. }
            | Self::AbiResultMismatch
            | Self::RootProtocolMismatch => None,
            Self::Identity(error) => Some(error),
            Self::Symbol(error) => Some(error),
            Self::Definition(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, DeclarationScope,
        DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
        GcEffect as IdentityGcEffect, PackagePath, PersistentExactTypeId, PersistentFunctionId,
        PersistentSymbolKey, PersistentTypeId, ScoopAbiReturn as CanonicalAbiReturn,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;
    use crate::{AbiReturn, CallingConvention};

    #[test]
    fn callable_binds_target_symbol_definition_and_root_protocol() {
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                core_site(),
                CanonicalIdentifier::new("println").unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let target = StrongCallableDefinitionOwner::Function(function);
        let unit = exact_type("Unit", SourceNominalKind::Object);
        let canonical_signature = CanonicalScoopAbiFunctionSignature::new(
            ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
            Vec::new(),
            CanonicalAbiReturn::unit_void(),
            IdentityGcEffect::Managed,
        )
        .unwrap();
        let callable = CoreExternalCallable::new(
            target,
            canonical_signature,
            ScoopAbiSignature::new(Vec::new(), AbiReturn::UnitVoid, CallingConvention::Cdecl),
            CoreExternalCallableRootPlan::ManagedStatepoint,
        )
        .unwrap();

        assert_eq!(callable.target(), target);
        assert_eq!(callable.calling_convention(), CallingConvention::Cdecl);
        assert_eq!(callable.gc_effect(), GcEffect::Managed);
        assert_eq!(
            callable.expected_symbol().key(),
            PersistentSymbolKey::CallableBody(callable.body())
        );
        let expected = ObjectDefinitionPlanId::from_key(
            &ObjectDefinitionPlanKey::strong(
                ConeIdentity::CORE,
                StrongDefinitionEntity::callable_body(callable.body()),
                StrongDefinitionRole::CallableBody,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(callable.required_definition(), expected);
        assert!(
            callable
                .expected_symbol()
                .symbol()
                .as_str()
                .starts_with("scoop$1$cb$")
        );
    }

    #[test]
    fn type_descriptor_has_no_independent_link_spelling() {
        let exact = exact_type("String", SourceNominalKind::Class);
        let descriptor = CoreExternalTypeDescriptor::new(exact).unwrap();

        assert_eq!(descriptor.target(), exact);
        assert_eq!(
            descriptor.expected_symbol().key(),
            PersistentSymbolKey::TypeDescriptor(exact)
        );
        let expected = ObjectDefinitionPlanId::from_key(
            &ObjectDefinitionPlanKey::strong(
                ConeIdentity::CORE,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(descriptor.required_definition(), expected);
        assert!(
            descriptor
                .expected_symbol()
                .symbol()
                .as_str()
                .starts_with("scoop$1$td$")
        );
    }

    fn exact_type(name: &str, kind: SourceNominalKind) -> PersistentExactTypeId {
        let source = SourceDeclarationKey::nominal(
            core_site(),
            CanonicalIdentifier::new(name).unwrap(),
            kind,
            0,
        );
        let ty = PersistentTypeId::from_source_declaration(&source).unwrap();
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(ty)).unwrap()
    }

    fn core_site() -> SourceDeclarationSite {
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap()
    }
}
