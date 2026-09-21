//! Typed external references admitted by the M23-3 trusted-core bridge.

use std::fmt;

pub use scoop_identity::ConeIdentity;
use scoop_identity::{
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, PersistentExactTypeId,
    PersistentSymbolError, PersistentSymbolRequest,
};
use scoop_wire::HashError;

pub(crate) fn core_type_descriptor_link_contract(
    target: PersistentExactTypeId,
) -> Result<(PersistentSymbolRequest, ObjectDefinitionPlanId), CoreExternalBuildError> {
    use crate::ExternalTypeDescriptorBuildError as Error;
    let descriptor = crate::ExternalTypeDescriptor::new(ConeIdentity::CORE, target).map_err(
        |error| match error {
            Error::Identity(error) => CoreExternalBuildError::Identity(error),
            Error::Symbol(error) => CoreExternalBuildError::Symbol(error),
            Error::Definition(error) => CoreExternalBuildError::Definition(error),
        },
    )?;
    Ok((
        descriptor.expected_symbol(),
        descriptor.required_definition(),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, DeclarationScope,
        DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
        GcEffect as IdentityGcEffect, PackagePath, PersistentExactTypeId, PersistentFunctionId,
        PersistentSymbolKey, PersistentTypeId, ScoopAbiReturn as CanonicalAbiReturn,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;
    use crate::{AbiReturn, CallingConvention, GcEffect, ScoopAbiSignature};
    use scoop_identity::{
        ObjectDefinitionPlanKey, StrongCallableDefinitionOwner, StrongDefinitionEntity,
        StrongDefinitionRole,
    };

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
        let selected = crate::SelectedDependencyLirCallableV1::new(
            ConeIdentity::CORE,
            scoop_identity::DependencyCallableDeclarationId::Function(function),
            target,
            canonical_signature,
            CallingConvention::Cdecl,
            crate::ExternalCallableRootPlan::ManagedStatepoint,
        )
        .unwrap();
        let set = crate::SelectedExternalLirSet::empty(ConeIdentity::SINGLE_FILE)
            .with_initialization_cycle(selected)
            .unwrap();
        let callable = set
            .callable(set.initialization_cycle().unwrap())
            .unwrap()
            .materialize(ScoopAbiSignature::new(
                Vec::new(),
                AbiReturn::UnitVoid,
                CallingConvention::Cdecl,
            ))
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
