//! Implicit construction uses the same declaration and default protocols.

use super::{HirDependencyCallReasonV1, HirDependencyCallSiteV1};
use crate::{
    CallableParameterCallingV1, CrossConeHirInterfaceSectionV1, ExternalHirTargetV1,
    PublicDeclarationOwnerV1, SourceNominalId,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOwnerAtom, Effect, ExactTypeKey, GcEffect,
    GeneratedCallableKey, IdentityReferenceError, PersistentConstructorId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::WireError;

mod role;

impl HirDependencyCallSiteV1 {
    /// Resolves the source constructor behind the actual zero-argument call.
    /// The caller separately binds its compiler role and actual occurrence.
    pub fn runtime_constructor_source(
        &self,
        target: ExternalHirTargetV1,
        provider: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        interface: &CrossConeHirInterfaceSectionV1,
    ) -> Result<(PersistentConstructorId, PersistentTypeId), HirRuntimeConstructorError> {
        use HirRuntimeConstructorError as Error;

        if !matches!(self.reason(), HirDependencyCallReasonV1::CastFailure { .. })
            || !self.arguments().is_empty()
        {
            return Err(Error::CallShape);
        }
        let (constructor, adapter) = match target {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(id)) => (id, false),
            ExternalHirTargetV1::GeneratedCallable(id) => {
                let key = identities.canonical_key::<_, GeneratedCallableKey>(id)?;
                let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } =
                    key.as_ref()
                else {
                    return Err(Error::Target(target));
                };
                (*constructor, true)
            }
            _ => return Err(Error::Target(target)),
        };
        let key = identities.canonical_key::<_, SourceDeclarationKey>(constructor)?;
        let result = identities.canonical_key::<_, ExactTypeKey>(self.result())?;
        let ExactTypeKey::Nominal(owner) = result.as_ref() else {
            return Err(Error::Result);
        };
        if key.origin() != provider
            || key.owners().owners().last() != Some(&DefinitionOwnerAtom::Type(*owner))
        {
            return Err(Error::Owner);
        }
        let declaration = CallableTemplateOrigin::Constructor(constructor);
        let callables = interface.callable_interfaces();
        let sources = interface.source_interfaces();

        let source = callables
            .declaration(declaration)
            .ok_or(Error::Declaration)?;
        if source.owner() != PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(*owner))
            || source.result() != &SignatureTypeKey::Nominal(*owner)
            || !source.type_parameters().is_empty()
            || source.receiver().is_some()
            || source.effects().execution() != Effect::Ordinary
            || source.effects().gc_effect() != GcEffect::Managed
        {
            return Err(Error::Declaration);
        }
        let parameters = sources
            .get(declaration)
            .ok_or(Error::Parameters)?
            .parameters()
            .parameters();

        if if adapter {
            parameters.iter().any(|parameter| {
                matches!(parameter.calling(), CallableParameterCallingV1::Required)
            })
        } else {
            !parameters.is_empty()
        } {
            return Err(Error::Parameters);
        }
        Ok((constructor, *owner))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirRuntimeConstructorError {
    Resource(WireError),
    Identity(IdentityReferenceError),
    CallShape,
    Target(ExternalHirTargetV1),
    Result,
    Owner,
    Declaration,
    Parameters,
    RoleTarget,
    RoleSignature,
}

impl From<WireError> for HirRuntimeConstructorError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<IdentityReferenceError> for HirRuntimeConstructorError {
    fn from(error: IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}
impl std::fmt::Display for HirRuntimeConstructorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid implicit HIR constructor: {self:?}")
    }
}
impl std::error::Error for HirRuntimeConstructorError {}
