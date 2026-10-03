//! Constructor, field, storage, and generated-value identities.

use scoop_identity::{GeneratedCallableKey, PersistentGeneratedCallableId};

use super::{DefaultEntityProjector, arena_get};
use crate::{
    DefaultClassConstructorIdV1, DefaultConstructorRefV1, DefaultEnumVariantFieldRefV1,
    DefaultEnumVariantRefV1, DefaultFieldRefV1, EnumVariantApplication,
    EnumVariantFieldApplication, HirClassConstructorIdentity, HirSignatureBinder,
    LexicalDefinitionRoot,
};

impl DefaultEntityProjector<'_> {
    pub(in crate::production::default_templates) fn struct_constructor(
        &self,
        application: crate::StructConstructorApplicationId,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultConstructorRefV1, super::super::DefaultEntityProjectionError> {
        let application = arena_get(&self.export.struct_constructor_applications, application)
            .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                kind: "struct constructor application",
                index: super::super::raw_index(application),
            })?;
        let owner = arena_get(&self.export.struct_applications, application.owner).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "struct application",
                index: super::super::raw_index(application.owner),
            },
        )?;
        Ok(DefaultConstructorRefV1::Struct {
            declaration: match application.constructor {
                crate::StructConstructorDefinition::Local(constructor) => {
                    self.struct_constructor_id(constructor)?
                }
                crate::StructConstructorDefinition::Template(template) => {
                    self.export.imported_constructor_templates[template].declaration
                }
            },
            owner_type: self.type_key(owner.canonical_type, binders)?,
        })
    }

    pub(in crate::production::default_templates) fn class_constructor(
        &self,
        application: crate::ClassConstructorApplicationId,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultConstructorRefV1, super::super::DefaultEntityProjectionError> {
        let application = arena_get(&self.export.class_constructor_applications, application)
            .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                kind: "class constructor application",
                index: super::super::raw_index(application),
            })?;
        let owner = arena_get(&self.export.class_applications, application.owner).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "class application",
                index: super::super::raw_index(application.owner),
            },
        )?;
        Ok(DefaultConstructorRefV1::Class {
            declaration: match application.constructor {
                crate::ClassConstructorDefinition::Local(constructor) => {
                    self.class_constructor_id(constructor)?
                }
                crate::ClassConstructorDefinition::Template(template) => {
                    DefaultClassConstructorIdV1::Source(
                        self.export.imported_constructor_templates[template].declaration,
                    )
                }
            },
            owner_type: self.type_key(owner.canonical_type, binders)?,
        })
    }

    pub(in crate::production::default_templates) fn variant(
        &self,
        variant: EnumVariantApplication,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultEnumVariantRefV1, super::super::DefaultEntityProjectionError> {
        Ok(DefaultEnumVariantRefV1::new(
            variant.variant,
            self.type_key(variant.owner, binders)?,
        ))
    }

    pub(in crate::production::default_templates) fn variant_field(
        &self,
        field: EnumVariantFieldApplication,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultEnumVariantFieldRefV1, super::super::DefaultEntityProjectionError> {
        Ok(DefaultEnumVariantFieldRefV1::new(
            field.field,
            self.type_key(field.variant.owner, binders)?,
        ))
    }

    pub(in crate::production::default_templates) fn field(
        &self,
        field: crate::FieldRef,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultFieldRefV1, super::super::DefaultEntityProjectionError> {
        match field {
            crate::FieldRef::ClassField { owner, field } => Ok(DefaultFieldRefV1::Class {
                declaration: field,
                owner_type: self.type_key(owner, binders)?,
            }),
            crate::FieldRef::StructField { owner, field } => Ok(DefaultFieldRefV1::Struct {
                declaration: field,
                owner_type: self.type_key(owner, binders)?,
            }),
            crate::FieldRef::TupleIndex(declaration_index) => {
                Ok(DefaultFieldRefV1::Tuple { declaration_index })
            }
        }
    }

    pub(in crate::production::default_templates) fn global_property(
        &self,
        global: crate::GlobalId,
    ) -> Result<scoop_identity::PersistentPropertyId, super::super::DefaultEntityProjectionError>
    {
        let global = arena_get(&self.export.globals, global).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "global",
                index: super::super::raw_index(global),
            },
        )?;
        let property = self.export.property_identities.get(global.property).ok_or(
            super::super::DefaultEntityProjectionError::MissingIdentity {
                kind: "property",
                index: super::super::raw_index(global.property),
            },
        )?;
        property.ordinary_id().ok_or(
            super::super::DefaultEntityProjectionError::ExpectedOrdinaryProperty {
                property: super::super::raw_index(global.property),
            },
        )
    }

    pub(in crate::production::default_templates) fn property_id(
        &self,
        property: crate::PropertyId,
    ) -> Result<scoop_identity::PersistentPropertyId, super::super::DefaultEntityProjectionError>
    {
        self.export
            .property_identities
            .get(property)
            .ok_or(
                super::super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "property",
                    index: super::super::raw_index(property),
                },
            )?
            .ordinary_id()
            .ok_or(
                super::super::DefaultEntityProjectionError::ExpectedOrdinaryProperty {
                    property: super::super::raw_index(property),
                },
            )
    }

    pub(in crate::production::default_templates) fn singleton_id(
        &self,
        target: crate::SingletonValueTarget,
    ) -> Result<scoop_identity::PersistentObjectValueId, super::super::DefaultEntityProjectionError>
    {
        let value = match target {
            crate::SingletonValueTarget::Local(value) => value,
            crate::SingletonValueTarget::Dependency(value) => return Ok(value),
        };
        arena_get(&self.export.singleton_values, value).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "singleton value",
                index: super::super::raw_index(value),
            },
        )?;
        self.export
            .object_value_identities
            .get(value)
            .map(|identity| identity.id())
            .ok_or(
                super::super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "singleton value",
                    index: super::super::raw_index(value),
                },
            )
    }

    pub(in crate::production::default_templates) fn callback_id(
        &self,
        id: crate::ForeignCallbackRegistrationId,
    ) -> Result<
        scoop_identity::PersistentCallbackRegistrationId,
        super::super::DefaultEntityProjectionError,
    > {
        arena_get(&self.export.foreign_callback_registrations, id).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "callback registration",
                index: super::super::raw_index(id),
            },
        )?;
        Ok(self.export.callback_registration_identities[id].id())
    }

    pub(in crate::production::default_templates) fn initialization_id(
        &self,
        id: crate::InitializationUnitId,
    ) -> Result<
        scoop_identity::PersistentInitializationUnitId,
        super::super::DefaultEntityProjectionError,
    > {
        arena_get(&self.export.initialization_units, id).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "initialization unit",
                index: super::super::raw_index(id),
            },
        )?;
        Ok(self.export.initialization_unit_identities[id].id())
    }

    pub(in crate::production::default_templates) fn callable_reference_invoke(
        &self,
        root: crate::CallableReferenceRoot,
        path: &scoop_identity::StructuralDefinitionPath,
    ) -> Result<PersistentGeneratedCallableId, super::super::DefaultEntityProjectionError> {
        let key = self.callable_reference_key(root, path)?;
        PersistentGeneratedCallableId::from_key(&key)
            .map_err(super::super::DefaultEntityProjectionError::GeneratedIdentity)
    }

    pub(in crate::production::default_templates) fn callable_reference_key(
        &self,
        root: crate::CallableReferenceRoot,
        path: &scoop_identity::StructuralDefinitionPath,
    ) -> Result<GeneratedCallableKey, super::super::DefaultEntityProjectionError> {
        let root = match root {
            crate::CallableReferenceRoot::Source(root) => root,
            crate::CallableReferenceRoot::Persistent(parent) => {
                return Ok(GeneratedCallableKey::CallableReferenceInvoke {
                    parent,
                    path: path.clone(),
                });
            }
        };
        let sites =
            self.export
                .local_functions
                .values()
                .filter_map(|declaration| {
                    declaration
                        .source()
                        .map(|(function, root)| (function, root, &declaration.definition_path))
                })
                .chain(self.export.lambdas.values().filter_map(|declaration| {
                    declaration
                        .definition
                        .source()
                        .map(|(function, root)| (function, root, &declaration.definition_path))
                }))
                .chain(
                    self.export
                        .anonymous_functions
                        .values()
                        .filter_map(|declaration| {
                            declaration.definition.source().map(|(function, root)| {
                                (function, root, &declaration.definition_path)
                            })
                        }),
                );
        let parent = sites
            .filter(|(_, candidate_root, candidate_path)| {
                *candidate_root == root
                    && candidate_path.segments().len() < path.segments().len()
                    && path.segments().starts_with(candidate_path.segments())
            })
            .max_by_key(|(_, _, candidate_path)| candidate_path.segments().len())
            .map_or(root, |(function, _, _)| {
                LexicalDefinitionRoot::Function(function)
            });
        Ok(GeneratedCallableKey::CallableReferenceInvoke {
            parent: self.lexical_parent(parent)?,
            path: path.clone(),
        })
    }

    pub(in crate::production::default_templates) fn struct_constructor_id(
        &self,
        constructor: crate::StructConstructorId,
    ) -> Result<scoop_identity::PersistentConstructorId, super::super::DefaultEntityProjectionError>
    {
        arena_get(&self.export.struct_constructors, constructor).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "struct constructor",
                index: super::super::raw_index(constructor),
            },
        )?;
        self.export
            .constructor_identities
            .get_struct(constructor)
            .map(|identity| identity.id())
            .ok_or(
                super::super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "struct constructor",
                    index: super::super::raw_index(constructor),
                },
            )
    }

    pub(in crate::production::default_templates) fn class_constructor_id(
        &self,
        constructor: crate::ClassConstructorId,
    ) -> Result<DefaultClassConstructorIdV1, super::super::DefaultEntityProjectionError> {
        arena_get(&self.export.class_constructors, constructor).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "class constructor",
                index: super::super::raw_index(constructor),
            },
        )?;
        match self
            .export
            .constructor_identities
            .get_class(constructor)
            .ok_or(
                super::super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "class constructor",
                    index: super::super::raw_index(constructor),
                },
            )? {
            HirClassConstructorIdentity::Source(record) => {
                Ok(DefaultClassConstructorIdV1::Source(record.id()))
            }
            HirClassConstructorIdentity::ZeroArgumentAdapter { record, .. } => {
                Ok(DefaultClassConstructorIdV1::Generated(record.id()))
            }
        }
    }

    pub(in crate::production::default_templates) fn variant_id(
        &self,
        variant: crate::EnumVariantRef,
    ) -> Result<scoop_identity::PersistentEnumVariantId, super::super::DefaultEntityProjectionError>
    {
        self.export
            .enum_member_identities
            .get_variant(variant)
            .map(|identity| identity.id())
            .ok_or(
                super::super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "enum variant",
                    index: variant.local_index(),
                },
            )
    }
}
