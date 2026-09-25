//! Typed closure authority used while producing one ordinary HIR interface.

use std::fmt;

use scoop_identity::{
    BindingTarget, BindingTargetError, CallableTemplateOrigin, CallableTemplateOwner, ConeIdentity,
    DefinitionOwnerAtom, GeneratedCallableKey, NominalDeclarationOwner, PersistentConstructorId,
    PersistentEnumVariantId, PersistentGeneratedCallableId, PersistentPropertyAccessorId,
    PropertyOwner, SourceDeclarationKey,
};

use super::ImportedSemanticWorld;
use crate::{
    CanonicalHirFoundation, CanonicalPublicExportBindingsV1, ExternalHirReferenceSemanticAuthority,
    ExternalHirTargetV1,
};

mod fields;
mod keys;
mod routes;

/// Production-side view of the current HIR foundation and its validated
/// imported semantic world.
///
/// Target ownership and public binding roots are resolved only through exact,
/// kind-specific persistent identity keys. The authority never searches by a
/// displayed name, FQN, symbol, or table position.
pub struct CrossConeHirProductionAuthority<'world, 'input> {
    current_foundation: &'world CanonicalHirFoundation,
    current_bindings: &'world CanonicalPublicExportBindingsV1,
    world: &'world ImportedSemanticWorld<'input>,
}

impl<'world, 'input> CrossConeHirProductionAuthority<'world, 'input> {
    pub const fn new(
        current_foundation: &'world CanonicalHirFoundation,
        current_bindings: &'world CanonicalPublicExportBindingsV1,
        world: &'world ImportedSemanticWorld<'input>,
    ) -> Self {
        Self {
            current_foundation,
            current_bindings,
            world,
        }
    }

    fn nominal_source_key(
        &self,
        owner: NominalDeclarationOwner,
        target: ExternalHirTargetV1,
    ) -> Result<&SourceDeclarationKey, CrossConeHirProductionAuthorityError> {
        match owner {
            NominalDeclarationOwner::Concrete(id) => self.source_type_key(id),
            NominalDeclarationOwner::GenericTemplate(id) => self.generic_type_key(id),
        }
        .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })
    }

    fn source_resolution(
        &self,
        key: &SourceDeclarationKey,
        binding_root: Result<BindingTarget, BindingTargetError>,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        Ok(ExternalTargetResolution {
            origin: key.origin(),
            binding_root: binding_root
                .map_err(CrossConeHirProductionAuthorityError::BindingTarget)?,
        })
    }

    fn nominal_resolution(
        &self,
        owner: NominalDeclarationOwner,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let key = self.nominal_source_key(owner, target)?;
        self.source_resolution(key, BindingTarget::type_name(key))
    }

    fn function_resolution(
        &self,
        key: Option<&SourceDeclarationKey>,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let key =
            key.ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
        let root = if key.duplicate_signature().receiver_is_present() {
            BindingTarget::extension_function(key)
        } else {
            BindingTarget::function(key)
        };
        self.source_resolution(key, root)
    }

    fn constructor_resolution(
        &self,
        constructor: PersistentConstructorId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let key = self
            .constructor_key(constructor)
            .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
        let owner = match key.owners().owners().last() {
            Some(DefinitionOwnerAtom::Type(owner)) => NominalDeclarationOwner::Concrete(*owner),
            Some(DefinitionOwnerAtom::GenericType(owner)) => {
                NominalDeclarationOwner::GenericTemplate(*owner)
            }
            _ => return Err(CrossConeHirProductionAuthorityError::NoPublicBindingRoot { target }),
        };
        let owner_key = self.nominal_source_key(owner, target)?;
        Ok(ExternalTargetResolution {
            origin: key.origin(),
            binding_root: BindingTarget::type_name(owner_key)
                .map_err(CrossConeHirProductionAuthorityError::BindingTarget)?,
        })
    }

    fn property_resolution(
        &self,
        property: PropertyOwner,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        match property {
            PropertyOwner::Property(id) => {
                let key = self
                    .property_key(id)
                    .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
                self.source_resolution(key, BindingTarget::property(key))
            }
            PropertyOwner::ExtensionProperty(id) => {
                let key = self
                    .extension_property_key(id)
                    .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
                self.source_resolution(key, BindingTarget::extension_property(key))
            }
        }
    }

    fn accessor_resolution(
        &self,
        accessor: PersistentPropertyAccessorId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let key = self
            .accessor_key(accessor)
            .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
        self.property_resolution(key.owner(), target)
    }

    fn variant_resolution(
        &self,
        variant: PersistentEnumVariantId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let key = self
            .variant_key(variant)
            .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
        let owner = key
            .source_owner()
            .ok_or(CrossConeHirProductionAuthorityError::NoPublicBindingRoot { target })?;
        let owner_key = self.nominal_source_key(owner, target)?;
        Ok(ExternalTargetResolution {
            origin: owner_key.origin(),
            binding_root: BindingTarget::enum_variant(variant),
        })
    }

    fn callable_resolution(
        &self,
        callable: CallableTemplateOrigin,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        match callable {
            CallableTemplateOrigin::Function(id) => {
                self.function_resolution(self.function_key(id), target)
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                self.function_resolution(self.generic_function_key(id), target)
            }
            CallableTemplateOrigin::Constructor(id) => self.constructor_resolution(id, target),
            CallableTemplateOrigin::Accessor(id) => self.accessor_resolution(id, target),
            CallableTemplateOrigin::VariantConstructor(id) => self.variant_resolution(id, target),
        }
    }

    fn callable_owner_resolution(
        &self,
        owner: CallableTemplateOwner,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        match owner {
            CallableTemplateOwner::Function(id) => {
                self.function_resolution(self.function_key(id), target)
            }
            CallableTemplateOwner::GenericFunction(id) => {
                self.function_resolution(self.generic_function_key(id), target)
            }
            CallableTemplateOwner::Constructor(id) => self.constructor_resolution(id, target),
            CallableTemplateOwner::Accessor(id) => self.accessor_resolution(id, target),
            CallableTemplateOwner::VariantConstructor(id) => self.variant_resolution(id, target),
            CallableTemplateOwner::Generated(id) => self.generated_callable_resolution(id, target),
        }
    }

    fn generated_callable_resolution(
        &self,
        generated: PersistentGeneratedCallableId,
        target: ExternalHirTargetV1,
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        let mut current = generated;
        let mut remaining = self.generated_callable_count().saturating_add(1);
        loop {
            if remaining == 0 {
                return Err(
                    CrossConeHirProductionAuthorityError::GeneratedCallableCycle {
                        target: generated,
                    },
                );
            }
            remaining -= 1;
            let key = self
                .generated_callable_key(current)
                .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
            let parent = match key {
                GeneratedCallableKey::Lexical { parent, .. }
                | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => parent.template(),
                GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                    return self.constructor_resolution(*constructor, target);
                }
                _ => {
                    return Err(CrossConeHirProductionAuthorityError::NoPublicBindingRoot {
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
    ) -> Result<ExternalTargetResolution, CrossConeHirProductionAuthorityError> {
        match target {
            ExternalHirTargetV1::Nominal(owner) => self.nominal_resolution(owner, target),
            ExternalHirTargetV1::Callable(callable) => self.callable_resolution(callable, target),
            ExternalHirTargetV1::Property(property) => self.property_resolution(property, target),
            ExternalHirTargetV1::ObjectValue(id) => {
                let key = self
                    .object_value_key(id)
                    .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
                self.source_resolution(key, BindingTarget::object_value(key))
            }
            ExternalHirTargetV1::TypeAlias(id) => {
                let key = self
                    .type_alias_key(id)
                    .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
                self.source_resolution(key, BindingTarget::type_alias(key))
            }
            ExternalHirTargetV1::Field(id) => self.field_resolution(id, target),
            ExternalHirTargetV1::EnumVariantField(id) => {
                let key = self
                    .variant_field_key(id)
                    .ok_or(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })?;
                self.variant_resolution(key.variant(), target)
            }
            ExternalHirTargetV1::GeneratedCallable(id) => {
                self.generated_callable_resolution(id, target)
            }
        }
    }
}

impl ExternalHirReferenceSemanticAuthority<CrossConeHirProductionAuthorityError>
    for CrossConeHirProductionAuthority<'_, '_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.world.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, CrossConeHirProductionAuthorityError> {
        self.resolve_external_target(target)
            .map(|resolved| resolved.origin)
    }

    fn external_hir_target_binding_root(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, CrossConeHirProductionAuthorityError> {
        self.resolve_external_target(target)
            .map(|resolved| resolved.binding_root)
    }
}

#[derive(Clone, Copy)]
struct ExternalTargetResolution {
    origin: ConeIdentity,
    binding_root: BindingTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirProductionAuthorityError {
    MissingCanonicalKey {
        target: ExternalHirTargetV1,
    },
    NoPublicBindingRoot {
        target: ExternalHirTargetV1,
    },
    InvalidObjectFieldOwner(scoop_identity::PersistentFieldId),
    GeneratedCallableCycle {
        target: PersistentGeneratedCallableId,
    },
    BindingTarget(BindingTargetError),
}

impl fmt::Display for CrossConeHirProductionAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCanonicalKey { target } => {
                write!(
                    formatter,
                    "external HIR target {target:?} has no canonical key"
                )
            }
            Self::NoPublicBindingRoot { target } => {
                write!(
                    formatter,
                    "external HIR target {target:?} has no public binding root"
                )
            }
            Self::InvalidObjectFieldOwner(field) => write!(
                formatter,
                "object backing field {field} and its property have different source owners or providers"
            ),
            Self::GeneratedCallableCycle { target } => write!(
                formatter,
                "generated callable {target} has a cyclic lexical parent chain"
            ),
            Self::BindingTarget(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirProductionAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindingTarget(error) => Some(error),
            Self::MissingCanonicalKey { .. }
            | Self::NoPublicBindingRoot { .. }
            | Self::InvalidObjectFieldOwner(_)
            | Self::GeneratedCallableCycle { .. } => None,
        }
    }
}
