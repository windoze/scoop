//! Canonical source roots for kind-preserving external HIR targets.

use std::fmt;
use std::sync::Arc;

use scoop_hir::{ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1};
use scoop_identity::{
    BindingTarget, BindingTargetError, CallableTemplateOrigin, CallableTemplateOwner, ConeIdentity,
    DefinitionOwnerAtom, EnumVariantIdentityKey, GeneratedCallableKey, NominalDeclarationOwner,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExtensionPropertyId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SourceDeclarationKey,
};

use super::CanonicalCrossConeRouteAuthority;

mod fields;

#[derive(Clone, Copy)]
struct ExternalTargetResolution {
    origin: ConeIdentity,
    binding_root: BindingTarget,
}

impl CanonicalCrossConeRouteAuthority<'_> {
    fn source_declaration_key<I>(
        &self,
        id: I,
    ) -> Result<Arc<SourceDeclarationKey>, CrossConeHirReferenceAuthorityError>
    where
        I: PersistentId + 'static,
    {
        self.identities
            .canonical_key::<I, SourceDeclarationKey>(id)
            .map_err(CrossConeHirReferenceAuthorityError::Identity)
    }

    fn nominal_source_key(
        &self,
        owner: NominalDeclarationOwner,
    ) -> Result<Arc<SourceDeclarationKey>, CrossConeHirReferenceAuthorityError> {
        match owner {
            NominalDeclarationOwner::Concrete(id) => {
                self.source_declaration_key::<PersistentTypeId>(id)
            }
            NominalDeclarationOwner::GenericTemplate(id) => {
                self.source_declaration_key::<PersistentGenericTypeId>(id)
            }
        }
    }

    fn source_resolution(
        &self,
        key: &SourceDeclarationKey,
        binding_root: Result<BindingTarget, BindingTargetError>,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        binding_root
            .map(|binding_root| ExternalTargetResolution {
                origin: key.origin(),
                binding_root,
            })
            .map_err(CrossConeHirReferenceAuthorityError::BindingTarget)
    }

    fn nominal_resolution(
        &self,
        owner: NominalDeclarationOwner,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let key = self.nominal_source_key(owner)?;
        self.source_resolution(&key, BindingTarget::type_name(&key))
    }

    fn function_resolution<I>(
        &self,
        id: I,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError>
    where
        I: PersistentId + 'static,
    {
        let key = self.source_declaration_key::<I>(id)?;
        let binding_root = if key.duplicate_signature().receiver_is_present() {
            BindingTarget::extension_function(&key)
        } else {
            BindingTarget::function(&key)
        };
        self.source_resolution(&key, binding_root)
    }

    fn constructor_resolution(
        &self,
        constructor: PersistentConstructorId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let key = self.source_declaration_key::<PersistentConstructorId>(constructor)?;
        let owner = match key.owners().owners().last() {
            Some(DefinitionOwnerAtom::Type(owner)) => NominalDeclarationOwner::Concrete(*owner),
            Some(DefinitionOwnerAtom::GenericType(owner)) => {
                NominalDeclarationOwner::GenericTemplate(*owner)
            }
            _ => return Err(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target }),
        };
        let owner_key = self.nominal_source_key(owner)?;
        let binding_root = BindingTarget::type_name(&owner_key)
            .map_err(CrossConeHirReferenceAuthorityError::BindingTarget)?;
        Ok(ExternalTargetResolution {
            origin: key.origin(),
            binding_root,
        })
    }

    fn property_resolution(
        &self,
        property: PropertyOwner,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        match property {
            PropertyOwner::Property(id) => {
                let key = self.source_declaration_key::<PersistentPropertyId>(id)?;
                self.source_resolution(&key, BindingTarget::property(&key))
            }
            PropertyOwner::ExtensionProperty(id) => {
                let key = self.source_declaration_key::<PersistentExtensionPropertyId>(id)?;
                self.source_resolution(&key, BindingTarget::extension_property(&key))
            }
        }
    }

    fn accessor_resolution(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentPropertyAccessorId, PropertyAccessorKey>(accessor)
            .map_err(CrossConeHirReferenceAuthorityError::Identity)?;
        self.property_resolution(key.owner())
    }

    fn variant_resolution(
        &self,
        variant: PersistentEnumVariantId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentEnumVariantId, EnumVariantIdentityKey>(variant)
            .map_err(CrossConeHirReferenceAuthorityError::Identity)?;
        let owner = key
            .source_owner()
            .ok_or(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target })?;
        let owner_key = self.nominal_source_key(owner)?;
        Ok(ExternalTargetResolution {
            origin: owner_key.origin(),
            binding_root: BindingTarget::enum_variant(variant),
        })
    }

    fn callable_resolution(
        &self,
        callable: CallableTemplateOrigin,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        match callable {
            CallableTemplateOrigin::Function(id) => {
                self.function_resolution::<PersistentFunctionId>(id)
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                self.function_resolution::<PersistentGenericFunctionId>(id)
            }
            CallableTemplateOrigin::Constructor(id) => self.constructor_resolution(id, target),
            CallableTemplateOrigin::Accessor(id) => self.accessor_resolution(id),
            CallableTemplateOrigin::VariantConstructor(id) => self.variant_resolution(id, target),
        }
    }

    fn callable_owner_resolution(
        &self,
        owner: CallableTemplateOwner,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        match owner {
            CallableTemplateOwner::Function(id) => {
                self.function_resolution::<PersistentFunctionId>(id)
            }
            CallableTemplateOwner::GenericFunction(id) => {
                self.function_resolution::<PersistentGenericFunctionId>(id)
            }
            CallableTemplateOwner::Constructor(id) => self.constructor_resolution(id, target),
            CallableTemplateOwner::Accessor(id) => self.accessor_resolution(id),
            CallableTemplateOwner::VariantConstructor(id) => self.variant_resolution(id, target),
            CallableTemplateOwner::Generated(id) => self.generated_callable_resolution(id, target),
        }
    }

    fn generated_callable_resolution(
        &self,
        generated: PersistentGeneratedCallableId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        let mut current = generated;
        let mut remaining = self.identities.identity_count();
        loop {
            if remaining == 0 {
                return Err(
                    CrossConeHirReferenceAuthorityError::GeneratedCallableCycle {
                        target: generated,
                    },
                );
            }
            remaining -= 1;
            let key = self
                .identities
                .canonical_key::<PersistentGeneratedCallableId, GeneratedCallableKey>(current)
                .map_err(CrossConeHirReferenceAuthorityError::Identity)?;
            let parent = match key.as_ref() {
                GeneratedCallableKey::Lexical { parent, .. }
                | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => parent.template(),
                _ => {
                    return Err(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot {
                        target,
                    });
                }
            };
            match parent {
                CallableTemplateOwner::Generated(next) => current = next,
                source => return self.callable_owner_resolution(source, target),
            }
        }
    }

    fn resolve_external_target(
        &self,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirReferenceAuthorityError> {
        match target {
            ExternalHirTargetV1::Nominal(owner) => self.nominal_resolution(owner),
            ExternalHirTargetV1::Callable(callable) => self.callable_resolution(callable, target),
            ExternalHirTargetV1::Property(property) => self.property_resolution(property),
            ExternalHirTargetV1::ObjectValue(id) => {
                let key = self.source_declaration_key::<PersistentObjectValueId>(id)?;
                self.source_resolution(&key, BindingTarget::object_value(&key))
            }
            ExternalHirTargetV1::TypeAlias(id) => {
                let key = self.source_declaration_key::<PersistentTypeAliasId>(id)?;
                self.source_resolution(&key, BindingTarget::type_alias(&key))
            }
            ExternalHirTargetV1::Field(id) => self.field_resolution(id, target),
            ExternalHirTargetV1::EnumVariantField(id) => self.variant_field_resolution(id, target),
            ExternalHirTargetV1::GeneratedCallable(id) => {
                self.generated_callable_resolution(id, target)
            }
        }
    }
}

impl ExternalHirReferenceSemanticAuthority<CrossConeHirReferenceAuthorityError>
    for CanonicalCrossConeRouteAuthority<'_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, CrossConeHirReferenceAuthorityError> {
        self.resolve_external_target(target)
            .map(|resolved| resolved.origin)
    }

    fn external_hir_target_binding_root(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, CrossConeHirReferenceAuthorityError> {
        self.resolve_external_target(target)
            .map(|resolved| resolved.binding_root)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalObjectFieldOwnerMismatch {
    pub field: scoop_identity::PersistentFieldId,
    pub property: PersistentPropertyId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirReferenceAuthorityError {
    Identity(scoop_identity::IdentityReferenceError),
    NoPublicBindingRoot {
        target: ExternalHirTargetV1,
    },
    ObjectFieldOwnerMismatch(Box<ExternalObjectFieldOwnerMismatch>),
    GeneratedCallableCycle {
        target: PersistentGeneratedCallableId,
    },
    BindingTarget(BindingTargetError),
}

impl fmt::Display for CrossConeHirReferenceAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::NoPublicBindingRoot { target } => {
                write!(
                    formatter,
                    "external HIR target {target:?} has no public binding root"
                )
            }
            Self::ObjectFieldOwnerMismatch(error) => write!(
                formatter,
                "object backing field {} and property {} have different source owners or providers",
                error.field, error.property,
            ),
            Self::GeneratedCallableCycle { target } => write!(
                formatter,
                "generated callable {target} has a cyclic lexical parent chain"
            ),
            Self::BindingTarget(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirReferenceAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::BindingTarget(error) => Some(error),
            Self::NoPublicBindingRoot { .. }
            | Self::ObjectFieldOwnerMismatch(_)
            | Self::GeneratedCallableCycle { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
