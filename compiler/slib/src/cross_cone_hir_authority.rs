//! Canonical identity authority used while validating cross-Cone HIR surfaces.

use std::borrow::Cow;

mod callable;
mod callable_source;
mod const_value;
mod definition_source;
mod errors;
mod property;
mod type_alias;

pub use callable_source::*;
pub use const_value::*;
pub use definition_source::*;
pub use errors::*;

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CoreHirInterfaceBranchV1, CoreTypeDefinitionV1,
    CrossConeHirInterfaceSectionV1, ExportBindingSourceV1, NominalInterfaceSemanticAuthority,
    NominalInterfaceShapeAuthority, NominalSourceShapeSemanticAuthority, OdrFreeHirFoundation,
    PublicDeclarationOwnerV1, PublicMemberRefV1, PublicNominalKindV1, PublicNominalShapeV1,
    SourceNominalId,
};
use scoop_identity::{
    BindableEntity, CallableTemplateOrigin, ConeIdentity, DefinitionOwnerAtom, EnumVariantFieldKey,
    EnumVariantIdentityKey, ExportBindingKey, FieldIdentityKey, NominalDeclarationOwner,
    PersistentConstructorId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExportBindingId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentObjectValueId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey,
    SourceDeclarationKey, ValidatedIdentityGraph,
};

/// A dependency provider whose nominal table has already passed canonical
/// identity validation.
#[derive(Clone, Copy)]
pub(crate) struct ValidatedNominalProviderView<'a> {
    pub(crate) identity: ConeIdentity,
    pub(crate) core: &'a CoreBootstrapInterfaceSectionV1,
    pub(crate) interface: &'a CrossConeHirInterfaceSectionV1,
}

/// Per-provider authority for the nominal portion of the general HIR surface.
///
/// The identity graph supplies exact canonical keys. Provider sections are
/// selected by the origin carried by those keys, so this type never searches
/// by display name, FQN, or raw digest. Callers supply only the current
/// provider's transitive dependency closure.
pub(crate) struct CanonicalCrossConeHirSurfaceAuthority<'a> {
    current: ConeIdentity,
    identities: &'a ValidatedIdentityGraph,
    current_foundation: &'a OdrFreeHirFoundation,
    current_core: &'a CoreBootstrapInterfaceSectionV1,
    current_interface: &'a CrossConeHirInterfaceSectionV1,
    dependencies: Vec<ValidatedNominalProviderView<'a>>,
}

impl<'a> CanonicalCrossConeHirSurfaceAuthority<'a> {
    pub(crate) fn new(
        current: ConeIdentity,
        identities: &'a ValidatedIdentityGraph,
        current_foundation: &'a OdrFreeHirFoundation,
        current_core: &'a CoreBootstrapInterfaceSectionV1,
        current_interface: &'a CrossConeHirInterfaceSectionV1,
        dependencies: Vec<ValidatedNominalProviderView<'a>>,
    ) -> Self {
        Self {
            current,
            identities,
            current_foundation,
            current_core,
            current_interface,
            dependencies,
        }
    }

    fn trusted_core(&self) -> Option<&CoreBootstrapInterfaceSectionV1> {
        if self.current == ConeIdentity::CORE {
            Some(self.current_core)
        } else {
            self.dependencies
                .iter()
                .find(|provider| provider.identity == ConeIdentity::CORE)
                .map(|provider| provider.core)
        }
    }

    fn require_current(
        &self,
        entity: &'static str,
        actual: ConeIdentity,
    ) -> Result<(), CrossConeHirNominalAuthorityError> {
        if actual == self.current {
            Ok(())
        } else {
            Err(CrossConeHirNominalAuthorityError::ForeignDeclaration {
                entity,
                expected: self.current,
                actual,
            })
        }
    }

    fn provider_interface(
        &self,
        origin: ConeIdentity,
    ) -> Result<&'a CrossConeHirInterfaceSectionV1, CrossConeHirNominalAuthorityError> {
        if origin == self.current {
            return Ok(self.current_interface);
        }
        self.dependencies
            .iter()
            .find(|provider| provider.identity == origin)
            .map(|provider| provider.interface)
            .ok_or(CrossConeHirNominalAuthorityError::UnreachableProvider { origin })
    }

    fn source_nominal_key(
        &self,
        declaration: SourceNominalId,
    ) -> Result<SourceDeclarationKey, CrossConeHirNominalAuthorityError> {
        let key = match declaration {
            NominalDeclarationOwner::Concrete(id) => self
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id),
            NominalDeclarationOwner::GenericTemplate(id) => self
                .identities
                .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id),
        }
        .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        Ok(key.as_ref().clone())
    }

    fn nominal_shape(
        &self,
        declaration: SourceNominalId,
    ) -> Result<PublicNominalShapeV1, CrossConeHirNominalAuthorityError> {
        let key = self.source_nominal_key(declaration)?;
        let origin = key.origin();
        let expected_kind =
            PublicNominalKindV1::try_from(key.declaration_kind()).map_err(|_| {
                CrossConeHirNominalAuthorityError::InvalidNominalDeclarationKind {
                    declaration,
                    actual: key.declaration_kind(),
                }
            })?;
        let expected_arity = key.duplicate_signature().type_parameter_count();
        if origin == ConeIdentity::CORE {
            let core = self
                .trusted_core()
                .ok_or(CrossConeHirNominalAuthorityError::MissingTrustedCore)?;
            let CoreHirInterfaceBranchV1::Core(core) = core.core_interface() else {
                return Err(CrossConeHirNominalAuthorityError::InvalidTrustedCore);
            };
            let definition = match declaration {
                SourceNominalId::Concrete(id) => CoreTypeDefinitionV1::Type(id),
                SourceNominalId::GenericTemplate(id) => CoreTypeDefinitionV1::GenericType(id),
            };
            if !core
                .type_targets()
                .targets()
                .iter()
                .any(|target| target.definition() == definition)
            {
                return Err(
                    CrossConeHirNominalAuthorityError::MissingCoreNominalAuthority { declaration },
                );
            }
            return Ok(PublicNominalShapeV1::new(expected_kind, expected_arity));
        }
        let interface = self.provider_interface(origin)?;
        let record = interface.nominal_interfaces().get(declaration).ok_or(
            CrossConeHirNominalAuthorityError::MissingNominalInterface {
                origin,
                declaration,
            },
        )?;
        if record.kind() != expected_kind {
            return Err(CrossConeHirNominalAuthorityError::NominalKindMismatch {
                declaration,
                expected: expected_kind,
                actual: record.kind(),
            });
        }
        let actual_arity = record.type_parameters().len_u32();
        if actual_arity != expected_arity {
            return Err(CrossConeHirNominalAuthorityError::NominalArityMismatch {
                declaration,
                expected: expected_arity,
                actual: actual_arity,
            });
        }
        Ok(PublicNominalShapeV1::new(expected_kind, expected_arity))
    }

    fn source_key_owner(
        &self,
        entity: &'static str,
        key: &SourceDeclarationKey,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        if key.duplicate_signature().receiver_is_present() {
            if key.owners().owners().is_empty() {
                return Ok(PublicDeclarationOwnerV1::Extension);
            }
            return Err(CrossConeHirNominalAuthorityError::NestedExtension {
                entity,
                owner_depth: key.owners().owners().len(),
            });
        }
        match key.owners().owners().last() {
            None => Ok(PublicDeclarationOwnerV1::TopLevel),
            Some(DefinitionOwnerAtom::Type(owner)) => Ok(PublicDeclarationOwnerV1::Nominal(
                NominalDeclarationOwner::Concrete(*owner),
            )),
            Some(DefinitionOwnerAtom::GenericType(owner)) => Ok(PublicDeclarationOwnerV1::Nominal(
                NominalDeclarationOwner::GenericTemplate(*owner),
            )),
            Some(owner) => Err(CrossConeHirNominalAuthorityError::InvalidDeclarationOwner {
                entity,
                owner: owner.clone(),
            }),
        }
    }

    fn require_current_nominal_owner(
        &self,
        entity: &'static str,
        owner: PublicDeclarationOwnerV1,
    ) -> Result<(), CrossConeHirNominalAuthorityError> {
        let PublicDeclarationOwnerV1::Nominal(owner) = owner else {
            return Ok(());
        };
        let key = self.source_nominal_key(owner)?;
        self.require_current(entity, key.origin())
    }

    fn declaration_owner_arity(
        &self,
        owner: PublicDeclarationOwnerV1,
    ) -> Result<u32, CrossConeHirNominalAuthorityError> {
        match owner {
            PublicDeclarationOwnerV1::Nominal(declaration) => self
                .nominal_shape(declaration)
                .map(|shape| shape.type_parameter_arity()),
            PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => Ok(0),
        }
    }

    fn function_key(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<SourceDeclarationKey, CrossConeHirNominalAuthorityError> {
        let key = match declaration {
            CallableTemplateOrigin::Function(id) => self
                .identities
                .canonical_key::<PersistentFunctionId, SourceDeclarationKey>(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                self.identities
                    .canonical_key::<PersistentGenericFunctionId, SourceDeclarationKey>(id)
            }
            CallableTemplateOrigin::Constructor(id) => self
                .identities
                .canonical_key::<PersistentConstructorId, SourceDeclarationKey>(id),
            CallableTemplateOrigin::Accessor(_) | CallableTemplateOrigin::VariantConstructor(_) => {
                return Err(
                    CrossConeHirNominalAuthorityError::MissingSourceDeclarationKey { declaration },
                );
            }
        }
        .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        Ok(key.as_ref().clone())
    }

    fn property_key(
        &self,
        declaration: PropertyOwner,
    ) -> Result<SourceDeclarationKey, CrossConeHirNominalAuthorityError> {
        let key = match declaration {
            PropertyOwner::Property(id) => self
                .identities
                .canonical_key::<PersistentPropertyId, SourceDeclarationKey>(id),
            PropertyOwner::ExtensionProperty(id) => self
                .identities
                .canonical_key::<PersistentExtensionPropertyId, SourceDeclarationKey>(id),
        }
        .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        Ok(key.as_ref().clone())
    }

    fn accessor_owner(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentPropertyAccessorId, PropertyAccessorKey>(accessor)
            .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        let property = self.property_key(key.owner())?;
        self.require_current("property accessor", property.origin())?;
        self.source_key_owner("property accessor", &property)
    }

    fn variant_owner(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentEnumVariantId, EnumVariantIdentityKey>(variant)
            .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        let owner = key
            .source_owner()
            .ok_or(CrossConeHirNominalAuthorityError::GeneratedEnumVariant { variant })?;
        let owner_key = self.source_nominal_key(owner)?;
        self.require_current("enum variant", owner_key.origin())?;
        Ok(PublicDeclarationOwnerV1::Nominal(owner))
    }

    fn binding_target_owner(
        &self,
        binding: PersistentExportBindingId,
        target: BindableEntity,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        match target {
            BindableEntity::Type(id) => {
                let key = self
                    .identities
                    .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id)
                    .map_err(CrossConeHirNominalAuthorityError::Identity)?;
                self.require_current("nested type binding", key.origin())?;
                self.source_key_owner("nested type binding", &key)
            }
            BindableEntity::GenericType(id) => {
                let key = self
                    .identities
                    .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id)
                    .map_err(CrossConeHirNominalAuthorityError::Identity)?;
                self.require_current("nested generic type binding", key.origin())?;
                self.source_key_owner("nested generic type binding", &key)
            }
            BindableEntity::ObjectValue(id) => {
                let key = self
                    .identities
                    .canonical_key::<PersistentObjectValueId, SourceDeclarationKey>(id)
                    .map_err(CrossConeHirNominalAuthorityError::Identity)?;
                self.require_current("nested object binding", key.origin())?;
                self.source_key_owner("nested object binding", &key)
            }
            BindableEntity::Function(id) => self
                .function_key(CallableTemplateOrigin::Function(id))
                .and_then(|key| {
                    self.require_current("nested function binding", key.origin())?;
                    self.source_key_owner("nested function binding", &key)
                }),
            BindableEntity::GenericFunction(id) => self
                .function_key(CallableTemplateOrigin::GenericFunction(id))
                .and_then(|key| {
                    self.require_current("nested generic function binding", key.origin())?;
                    self.source_key_owner("nested generic function binding", &key)
                }),
            BindableEntity::Property(id) => self
                .property_key(PropertyOwner::Property(id))
                .and_then(|key| {
                    self.require_current("nested property binding", key.origin())?;
                    self.source_key_owner("nested property binding", &key)
                }),
            BindableEntity::ExtensionProperty(id) => self
                .property_key(PropertyOwner::ExtensionProperty(id))
                .and_then(|key| {
                    self.require_current("nested extension property binding", key.origin())?;
                    self.source_key_owner("nested extension property binding", &key)
                }),
            BindableEntity::TypeAlias(id) => {
                let key = self
                    .identities
                    .canonical_key::<PersistentTypeAliasId, SourceDeclarationKey>(id)
                    .map_err(CrossConeHirNominalAuthorityError::Identity)?;
                self.require_current("nested type-alias binding", key.origin())?;
                self.source_key_owner("nested type-alias binding", &key)
            }
            BindableEntity::EnumVariant(variant) => self.variant_owner(variant),
        }
        .map_err(
            |error| CrossConeHirNominalAuthorityError::NestedBindingTarget {
                binding,
                source: Box::new(error),
            },
        )
    }
}

impl NominalInterfaceShapeAuthority<CrossConeHirNominalAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, CrossConeHirNominalAuthorityError> {
        self.nominal_shape(NominalDeclarationOwner::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, CrossConeHirNominalAuthorityError> {
        self.nominal_shape(NominalDeclarationOwner::GenericTemplate(declaration))
    }
}

impl NominalSourceShapeSemanticAuthority<CrossConeHirNominalAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn struct_field_key(
        &mut self,
        field: PersistentFieldId,
    ) -> Result<Cow<'_, FieldIdentityKey>, CrossConeHirNominalAuthorityError> {
        self.identities
            .canonical_key::<PersistentFieldId, FieldIdentityKey>(field)
            .map(|key| Cow::Owned(key.as_ref().clone()))
            .map_err(CrossConeHirNominalAuthorityError::Identity)
    }

    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<Cow<'_, EnumVariantIdentityKey>, CrossConeHirNominalAuthorityError> {
        self.identities
            .canonical_key::<PersistentEnumVariantId, EnumVariantIdentityKey>(variant)
            .map(|key| Cow::Owned(key.as_ref().clone()))
            .map_err(CrossConeHirNominalAuthorityError::Identity)
    }

    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<Cow<'_, EnumVariantFieldKey>, CrossConeHirNominalAuthorityError> {
        self.identities
            .canonical_key::<PersistentEnumVariantFieldId, EnumVariantFieldKey>(field)
            .map(|key| Cow::Owned(key.as_ref().clone()))
            .map_err(CrossConeHirNominalAuthorityError::Identity)
    }

    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<Cow<'_, SourceDeclarationKey>, CrossConeHirNominalAuthorityError> {
        self.identities
            .canonical_key::<PersistentObjectValueId, SourceDeclarationKey>(value)
            .map(|key| Cow::Owned(key.as_ref().clone()))
            .map_err(CrossConeHirNominalAuthorityError::Identity)
    }
}

impl NominalInterfaceSemanticAuthority<CrossConeHirNominalAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn nominal_declaration_key(
        &mut self,
        declaration: SourceNominalId,
    ) -> Result<SourceDeclarationKey, CrossConeHirNominalAuthorityError> {
        let key = self.source_nominal_key(declaration)?;
        self.require_current("nominal interface", key.origin())?;
        let owner = self.source_key_owner("nominal interface", &key)?;
        self.require_current_nominal_owner("nominal owner", owner)?;
        Ok(key)
    }

    fn constructor_owner(
        &mut self,
        constructor: PersistentConstructorId,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        let key = self.function_key(CallableTemplateOrigin::Constructor(constructor))?;
        self.require_current("constructor", key.origin())?;
        self.source_key_owner("constructor", &key)
    }

    fn member_owner(
        &mut self,
        member: PublicMemberRefV1,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        match member {
            PublicMemberRefV1::Callable(declaration) => match declaration {
                CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_) => {
                    let key = self.function_key(declaration)?;
                    self.require_current("callable member", key.origin())?;
                    self.source_key_owner("callable member", &key)
                }
                CallableTemplateOrigin::Accessor(accessor) => self.accessor_owner(accessor),
                CallableTemplateOrigin::Constructor(_) => {
                    Err(CrossConeHirNominalAuthorityError::ConstructorInMemberSet)
                }
                CallableTemplateOrigin::VariantConstructor(_) => {
                    Err(CrossConeHirNominalAuthorityError::VariantConstructorInMemberSet)
                }
            },
            PublicMemberRefV1::Property(declaration) => {
                let key = self.property_key(declaration)?;
                self.require_current("property member", key.origin())?;
                self.source_key_owner("property member", &key)
            }
        }
    }

    fn nested_binding_owner(
        &mut self,
        binding: PersistentExportBindingId,
    ) -> Result<PublicDeclarationOwnerV1, CrossConeHirNominalAuthorityError> {
        let key = self
            .identities
            .canonical_key::<PersistentExportBindingId, ExportBindingKey>(binding)
            .map_err(CrossConeHirNominalAuthorityError::Identity)?;
        if key.exporter() != self.current {
            return Err(
                CrossConeHirNominalAuthorityError::NestedBindingExporterMismatch {
                    binding,
                    expected: self.current,
                    actual: key.exporter(),
                },
            );
        }
        let record = self
            .current_interface
            .public_bindings()
            .get(binding)
            .ok_or(CrossConeHirNominalAuthorityError::MissingNestedBindingRecord { binding })?;
        if !matches!(
            record.source(),
            ExportBindingSourceV1::DeclaredCurrent { .. }
        ) {
            return Err(CrossConeHirNominalAuthorityError::ReexportedNestedBinding { binding });
        }
        self.binding_target_owner(binding, key.target())
    }
}

fn optional_signature(value: &scoop_identity::OptionalSignatureType) -> Option<SignatureTypeKey> {
    match value {
        scoop_identity::OptionalSignatureType::Absent => None,
        scoop_identity::OptionalSignatureType::Present(value) => Some(value.as_ref().clone()),
    }
}
