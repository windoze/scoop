use std::collections::BTreeMap;

use scoop_identity::{
    BindableEntity, CallableTemplateOrigin, DuplicateSignatureKey, OptionalSignatureType,
    PersistentEnumVariantId, PersistentObjectValueId, PersistentTypeAliasId, PropertyOwner,
};

use super::{
    ImportedBindingConflictKey, ImportedCallableDeclarationId, ImportedPropertyDeclarationId,
    ImportedSourceNominalId, ImportedTarget,
};
use crate::{
    ImportedSemanticEntityId, ImportedSemanticWorldBuildError, NominalSourceShapeV1,
    SourceNominalId, WorldConeId, semantic_world::provider::ImportedProvider,
};

pub(in crate::semantic_world) struct ImportedEntityIndex {
    nominals: BTreeMap<SourceNominalId, WorldConeId>,
    callables: BTreeMap<CallableTemplateOrigin, WorldConeId>,
    properties: BTreeMap<PropertyOwner, WorldConeId>,
    aliases: BTreeMap<PersistentTypeAliasId, WorldConeId>,
    object_values: BTreeMap<PersistentObjectValueId, WorldConeId>,
    enum_variants: BTreeMap<PersistentEnumVariantId, WorldConeId>,
    targets: BTreeMap<BindableEntity, ImportedTargetEntry>,
}

struct ImportedTargetEntry {
    provider: WorldConeId,
    target: ImportedTarget,
    conflict: ImportedBindingConflictKey,
}

impl ImportedEntityIndex {
    pub(in crate::semantic_world) fn build(
        providers: &[ImportedProvider<'_>],
    ) -> Result<Self, ImportedSemanticWorldBuildError> {
        let mut index = Self {
            nominals: BTreeMap::new(),
            callables: BTreeMap::new(),
            properties: BTreeMap::new(),
            aliases: BTreeMap::new(),
            object_values: BTreeMap::new(),
            enum_variants: BTreeMap::new(),
            targets: BTreeMap::new(),
        };
        for provider in providers {
            index.insert_provider(provider)?;
        }
        Ok(index)
    }

    fn insert_provider(
        &mut self,
        provider: &ImportedProvider<'_>,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        self.insert_nominals(provider)?;
        self.insert_callables(provider)?;
        self.insert_properties(provider)?;
        self.insert_aliases(provider)
    }

    fn insert_nominals(
        &mut self,
        provider: &ImportedProvider<'_>,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        for record in provider.interface().nominal_interfaces().records() {
            let declaration = record.declaration();
            let imported = import_nominal_id(provider, declaration).ok_or(
                ImportedSemanticWorldBuildError::MissingEntityIdentity {
                    provider: provider.identity(),
                    entity: ImportedSemanticEntityId::Nominal(declaration),
                },
            )?;
            insert_unique(
                &mut self.nominals,
                declaration,
                provider,
                ImportedSemanticEntityId::Nominal(declaration),
            )?;
            match imported {
                ImportedSourceNominalId::Concrete(id) => {
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::Nominal(declaration),
                        BindableEntity::Type(id.persistent()),
                        ImportedTarget::Type(id),
                        ImportedBindingConflictKey::Type,
                    )?;
                }
                ImportedSourceNominalId::GenericTemplate(id) => {
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::Nominal(declaration),
                        BindableEntity::GenericType(id.persistent()),
                        ImportedTarget::GenericType(id),
                        ImportedBindingConflictKey::Type,
                    )?;
                }
            }
            self.insert_nominal_shape_entities(provider, record.source_shape())?;
        }
        Ok(())
    }

    fn insert_nominal_shape_entities(
        &mut self,
        provider: &ImportedProvider<'_>,
        shape: &NominalSourceShapeV1,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        match shape {
            NominalSourceShapeV1::Object(shape) => {
                let id = shape.value();
                let imported = provider.foundation().identity(id).ok_or(
                    ImportedSemanticWorldBuildError::MissingEntityIdentity {
                        provider: provider.identity(),
                        entity: ImportedSemanticEntityId::ObjectValue(id),
                    },
                )?;
                insert_unique(
                    &mut self.object_values,
                    id,
                    provider,
                    ImportedSemanticEntityId::ObjectValue(id),
                )?;
                self.insert_target(
                    provider,
                    ImportedSemanticEntityId::ObjectValue(id),
                    BindableEntity::ObjectValue(id),
                    ImportedTarget::ObjectValue(imported),
                    ImportedBindingConflictKey::Value,
                )?;
            }
            NominalSourceShapeV1::Enum(shape) => {
                for variant in shape.variants() {
                    let id = variant.variant();
                    let imported = provider.foundation().identity(id).ok_or(
                        ImportedSemanticWorldBuildError::MissingEntityIdentity {
                            provider: provider.identity(),
                            entity: ImportedSemanticEntityId::EnumVariant(id),
                        },
                    )?;
                    insert_unique(
                        &mut self.enum_variants,
                        id,
                        provider,
                        ImportedSemanticEntityId::EnumVariant(id),
                    )?;
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::EnumVariant(id),
                        BindableEntity::EnumVariant(id),
                        ImportedTarget::EnumVariant(imported),
                        ImportedBindingConflictKey::Value,
                    )?;
                }
            }
            NominalSourceShapeV1::Class
            | NominalSourceShapeV1::Interface
            | NominalSourceShapeV1::Struct(_)
            | NominalSourceShapeV1::Intrinsic(_) => return Ok(()),
        }
        Ok(())
    }

    fn insert_callables(
        &mut self,
        provider: &ImportedProvider<'_>,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        for record in provider.interface().callable_interfaces().records() {
            let declaration = record.declaration();
            let conflict = ImportedBindingConflictKey::Overload(DuplicateSignatureKey::Function {
                type_parameter_count: record.type_parameters().len_u32(),
                receiver: OptionalSignatureType::from_option(record.receiver().cloned()),
                parameters: record
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.value_type().clone())
                    .collect(),
            });
            let imported = import_callable_id(provider, declaration).ok_or(
                ImportedSemanticWorldBuildError::MissingEntityIdentity {
                    provider: provider.identity(),
                    entity: ImportedSemanticEntityId::Callable(declaration),
                },
            )?;
            insert_unique(
                &mut self.callables,
                declaration,
                provider,
                ImportedSemanticEntityId::Callable(declaration),
            )?;
            match imported {
                ImportedCallableDeclarationId::Function(id) => {
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::Callable(declaration),
                        BindableEntity::Function(id.persistent()),
                        ImportedTarget::Function(id),
                        conflict.clone(),
                    )?;
                }
                ImportedCallableDeclarationId::GenericFunction(id) => {
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::Callable(declaration),
                        BindableEntity::GenericFunction(id.persistent()),
                        ImportedTarget::GenericFunction(id),
                        conflict,
                    )?;
                }
                ImportedCallableDeclarationId::Constructor(_)
                | ImportedCallableDeclarationId::Accessor(_)
                | ImportedCallableDeclarationId::VariantConstructor(_) => {}
            }
        }
        Ok(())
    }

    fn insert_properties(
        &mut self,
        provider: &ImportedProvider<'_>,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        for record in provider.interface().property_interfaces().records() {
            let declaration = record.declaration();
            let imported = import_property_id(provider, declaration).ok_or(
                ImportedSemanticWorldBuildError::MissingEntityIdentity {
                    provider: provider.identity(),
                    entity: ImportedSemanticEntityId::Property(declaration),
                },
            )?;
            insert_unique(
                &mut self.properties,
                declaration,
                provider,
                ImportedSemanticEntityId::Property(declaration),
            )?;
            match imported {
                ImportedPropertyDeclarationId::Property(id) => {
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::Property(declaration),
                        BindableEntity::Property(id.persistent()),
                        ImportedTarget::Property(id),
                        ImportedBindingConflictKey::Value,
                    )?;
                }
                ImportedPropertyDeclarationId::ExtensionProperty(id) => {
                    let conflict =
                        ImportedBindingConflictKey::Overload(DuplicateSignatureKey::Property {
                            type_parameter_count: record.type_parameters().len_u32(),
                            receiver: OptionalSignatureType::from_option(
                                record.receiver().cloned(),
                            ),
                        });
                    self.insert_target(
                        provider,
                        ImportedSemanticEntityId::Property(declaration),
                        BindableEntity::ExtensionProperty(id.persistent()),
                        ImportedTarget::ExtensionProperty(id),
                        conflict,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn insert_aliases(
        &mut self,
        provider: &ImportedProvider<'_>,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        for record in provider.interface().type_aliases().records() {
            let alias = record.alias();
            let imported = provider.foundation().identity(alias).ok_or(
                ImportedSemanticWorldBuildError::MissingEntityIdentity {
                    provider: provider.identity(),
                    entity: ImportedSemanticEntityId::TypeAlias(alias),
                },
            )?;
            if provider.alias_expansions().get(alias).is_none() {
                return Err(ImportedSemanticWorldBuildError::MissingAliasExpansion {
                    provider: provider.identity(),
                    alias,
                });
            }
            insert_unique(
                &mut self.aliases,
                alias,
                provider,
                ImportedSemanticEntityId::TypeAlias(alias),
            )?;
            self.insert_target(
                provider,
                ImportedSemanticEntityId::TypeAlias(alias),
                BindableEntity::TypeAlias(alias),
                ImportedTarget::TypeAlias(imported),
                ImportedBindingConflictKey::Type,
            )?;
        }
        Ok(())
    }

    fn insert_target(
        &mut self,
        provider: &ImportedProvider<'_>,
        entity: ImportedSemanticEntityId,
        persistent: BindableEntity,
        imported: ImportedTarget,
        conflict: ImportedBindingConflictKey,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        if let Some(first) = self.targets.insert(
            persistent,
            ImportedTargetEntry {
                provider: provider.id(),
                target: imported,
                conflict,
            },
        ) {
            return Err(ImportedSemanticWorldBuildError::DuplicateEntityAuthority {
                entity,
                first: first.provider,
                second: provider.identity(),
            });
        }
        Ok(())
    }

    pub(in crate::semantic_world) fn import_target(
        &self,
        target: BindableEntity,
    ) -> Option<ImportedTarget> {
        self.targets.get(&target).map(|entry| entry.target)
    }

    pub(in crate::semantic_world) fn import_target_with_conflict(
        &self,
        target: BindableEntity,
    ) -> Option<(ImportedTarget, &ImportedBindingConflictKey)> {
        self.targets
            .get(&target)
            .map(|entry| (entry.target, &entry.conflict))
    }

    pub(in crate::semantic_world) fn nominal_provider(
        &self,
        id: SourceNominalId,
    ) -> Option<WorldConeId> {
        self.nominals.get(&id).copied()
    }

    pub(in crate::semantic_world) fn callable_provider(
        &self,
        id: CallableTemplateOrigin,
    ) -> Option<WorldConeId> {
        self.callables.get(&id).copied()
    }

    pub(in crate::semantic_world) fn property_provider(
        &self,
        id: PropertyOwner,
    ) -> Option<WorldConeId> {
        self.properties.get(&id).copied()
    }

    pub(in crate::semantic_world) fn alias_provider(
        &self,
        id: PersistentTypeAliasId,
    ) -> Option<WorldConeId> {
        self.aliases.get(&id).copied()
    }

    pub(in crate::semantic_world) fn object_value_provider(
        &self,
        id: PersistentObjectValueId,
    ) -> Option<WorldConeId> {
        self.object_values.get(&id).copied()
    }

    pub(in crate::semantic_world) fn enum_variant_provider(
        &self,
        id: PersistentEnumVariantId,
    ) -> Option<WorldConeId> {
        self.enum_variants.get(&id).copied()
    }
}

fn insert_unique<I: Ord>(
    map: &mut BTreeMap<I, WorldConeId>,
    id: I,
    provider: &ImportedProvider<'_>,
    entity: ImportedSemanticEntityId,
) -> Result<(), ImportedSemanticWorldBuildError> {
    if let Some(first) = map.insert(id, provider.id()) {
        return Err(ImportedSemanticWorldBuildError::DuplicateEntityAuthority {
            entity,
            first,
            second: provider.identity(),
        });
    }
    Ok(())
}

pub(in crate::semantic_world) fn import_nominal_id(
    provider: &ImportedProvider<'_>,
    declaration: SourceNominalId,
) -> Option<ImportedSourceNominalId> {
    match declaration {
        SourceNominalId::Concrete(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedSourceNominalId::Concrete),
        SourceNominalId::GenericTemplate(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedSourceNominalId::GenericTemplate),
    }
}

pub(in crate::semantic_world) fn import_callable_id(
    provider: &ImportedProvider<'_>,
    declaration: CallableTemplateOrigin,
) -> Option<ImportedCallableDeclarationId> {
    match declaration {
        CallableTemplateOrigin::Function(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedCallableDeclarationId::Function),
        CallableTemplateOrigin::GenericFunction(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedCallableDeclarationId::GenericFunction),
        CallableTemplateOrigin::Constructor(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedCallableDeclarationId::Constructor),
        CallableTemplateOrigin::Accessor(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedCallableDeclarationId::Accessor),
        CallableTemplateOrigin::VariantConstructor(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedCallableDeclarationId::VariantConstructor),
    }
}

pub(in crate::semantic_world) fn import_property_id(
    provider: &ImportedProvider<'_>,
    declaration: PropertyOwner,
) -> Option<ImportedPropertyDeclarationId> {
    match declaration {
        PropertyOwner::Property(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedPropertyDeclarationId::Property),
        PropertyOwner::ExtensionProperty(id) => provider
            .foundation()
            .identity(id)
            .map(ImportedPropertyDeclarationId::ExtensionProperty),
    }
}
