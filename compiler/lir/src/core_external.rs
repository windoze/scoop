//! Typed external references admitted by the M23-3 trusted-core bridge.

use std::fmt;

pub use scoop_identity::ConeIdentity;
use scoop_identity::{
    CallableBodyKey, LinkageClass, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::HashError;

use crate::{CallingConvention, GcEffect, ScoopAbiSignature};

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

/// A callable definition imported exclusively from the trusted core Cone.
/// Link spelling is derived from the persistent request and is never stored
/// as an independent string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreExternalCallable {
    target: StrongCallableDefinitionOwner,
    body: PersistentCallableBodyId,
    signature: ScoopAbiSignature,
    root_plan: CoreExternalCallableRootPlan,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl CoreExternalCallable {
    pub fn new(
        target: StrongCallableDefinitionOwner,
        signature: ScoopAbiSignature,
        root_plan: CoreExternalCallableRootPlan,
    ) -> Result<Self, CoreExternalBuildError> {
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
        Ok(Self {
            target,
            body,
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
        let expected_symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeDescriptor(target),
            LinkageClass::ConeStrong,
        )
        .map_err(CoreExternalBuildError::Symbol)?;
        let required_definition = required_definition(
            StrongDefinitionEntity::exact_type(target),
            StrongDefinitionRole::TypeDescriptor,
        )?;
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

#[derive(Debug)]
pub enum CoreExternalBuildError {
    Identity(HashError),
    Symbol(PersistentSymbolError),
    Definition(ObjectDefinitionIdentityError),
}

impl fmt::Display for CoreExternalBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Identity(error) => Some(error),
            Self::Symbol(error) => Some(error),
            Self::Definition(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, ExactTypeKey, PackagePath,
        PersistentFunctionId, PersistentSymbolKey, PersistentTypeId, SourceDeclarationKey,
        SourceDeclarationSite, SourceNominalKind,
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
        let callable = CoreExternalCallable::new(
            target,
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
        let source = SourceDeclarationKey::nominal(
            core_site(),
            CanonicalIdentifier::new("String").unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let ty = PersistentTypeId::from_source_declaration(&source).unwrap();
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(ty)).unwrap();
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
