//! Owned declaration data used by dependency type resolution.

use std::sync::Arc;

use scoop_identity::{
    CborIdentityRecord, DeclarationName, EnumVariantFieldSelector, FieldIdentityView,
    PersistentTypeId,
};

use super::{ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError};
use crate::{NativeBoundaryCAbiV1, NominalInterfaceRecordV1, SourceNominalId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedNominalDeclaration {
    pub identity: crate::HirSourceNominalIdentity,
    pub origin: crate::ExportDefinitionSourceV1,
    pub interface: NominalInterfaceRecordV1,
    pub field_sources: Vec<ImportedNominalFieldSource>,
    pub variant_sources: Vec<ImportedEnumVariantSource>,
    pub c_abi: NativeBoundaryCAbiV1,
    pub dispatch_slots: Vec<
        CborIdentityRecord<
            scoop_identity::PersistentDispatchSlotId,
            scoop_identity::DispatchSlotKey,
        >,
    >,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedNominalFieldSource {
    pub name: String,
    pub storage: crate::NominalFieldStorage,
    pub origin: crate::ExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedEnumVariantSource {
    pub name: String,
    pub fields: Vec<ImportedEnumVariantFieldSource>,
    pub origin: crate::ExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedEnumVariantFieldSource {
    pub name: String,
    pub origin: crate::ExportDefinitionSourceV1,
}

impl ImportedNominalDeclaration {
    pub const fn owner(&self) -> SourceNominalId {
        self.interface.declaration()
    }

    pub fn name(&self) -> &str {
        match self.identity.declaration().name() {
            DeclarationName::Named(name) => name.as_str(),
            _ => unreachable!("source nominal declarations have names"),
        }
    }
}

impl ImportedDependencySelectionPlan {
    /// Description generation also needs types of uncalled constructors and
    /// computed properties, which do not participate in the storage layout.
    pub fn description_member_types(
        &self,
        owner: SourceNominalId,
    ) -> impl Iterator<Item = &scoop_identity::SignatureTypeKey> {
        let members = self.catalog.indexes.nominals.get(&owner);
        members
            .into_iter()
            .flat_map(|members| &members.properties)
            .map(|property| self.catalog.properties[property].interface.value_type())
            .chain(
                members
                    .into_iter()
                    .flat_map(|members| &members.constructors)
                    .flat_map(|declaration| {
                        self.catalog.callables[declaration]
                            .interface
                            .parameters()
                            .parameters()
                            .iter()
                            .map(|parameter| parameter.value_type())
                    }),
            )
    }

    /// Queries declaration identity directly, including private storage support.
    /// Source visibility is checked by the caller's binding or member lookup.
    pub fn nominal(&self, id: PersistentTypeId) -> Option<&Arc<ImportedNominalDeclaration>> {
        self.nominal_declaration(SourceNominalId::Concrete(id))
    }

    pub fn nominal_declaration(
        &self,
        owner: SourceNominalId,
    ) -> Option<&Arc<ImportedNominalDeclaration>> {
        self.catalog.nominals.get(&owner)
    }

    pub fn nominal_declarations(&self) -> impl Iterator<Item = &Arc<ImportedNominalDeclaration>> {
        self.catalog.nominals.values()
    }

    /// Lexical scope visibility does not require a generic owner's machine type.
    pub fn nominal_visibility(
        &self,
        owner: SourceNominalId,
    ) -> Option<crate::DeclaredVisibilityV1> {
        self.catalog.nominal_visibilities.get(&owner).copied()
    }

    pub fn nested_nominal(
        &self,
        owner: SourceNominalId,
        name: &str,
    ) -> Option<&Arc<ImportedNominalDeclaration>> {
        self.nominal_declaration(owner)?
            .interface
            .declaration_details()
            .children()
            .values()
            .iter()
            .filter_map(|child| self.nominal_declaration(*child))
            .find(|declaration| declaration.name() == name)
    }

    pub fn singleton_owner(
        &self,
        value: scoop_identity::PersistentObjectValueId,
    ) -> Option<&Arc<ImportedNominalDeclaration>> {
        self.catalog.nominals.values().find(|declaration| {
            matches!(declaration.interface.source_shape(), crate::NominalSourceShapeV1::Object(shape) if shape.value() == value)
        })
    }
}

pub(super) fn declarations(
    provider: &crate::semantic_world::ImportedProvider<'_>,
) -> Result<Vec<Arc<ImportedNominalDeclaration>>, ImportedDependencySelectionPlanBuildError> {
    use ImportedDependencySelectionPlanBuildError as Error;
    let foundation = provider.foundation();
    let canonical = foundation.canonical_for_semantic_authority();
    let definition_origin = |subject| {
        canonical
            .definition_origin(subject)
            .map(|record| crate::ExportDefinitionSourceV1::new(record.origin().clone()))
            .ok_or(Error::MissingDefinitionOrigin(subject))
    };
    provider
        .interface()
        .nominal_interfaces()
        .all_records()
        .map(|interface| {
            let owner = interface.declaration();
            let subject = match owner {
                SourceNominalId::Concrete(id) => scoop_identity::DefinitionOriginSubject::Type(id),
                SourceNominalId::GenericTemplate(id) => {
                    scoop_identity::DefinitionOriginSubject::GenericType(id)
                }
            };
            let origin = canonical
                .definition_origin(subject)
                .ok_or(Error::MissingDefinitionOrigin(subject))?;
            let identity = match owner {
                SourceNominalId::Concrete(id) => crate::HirSourceNominalIdentity::Concrete(
                    canonical
                        .type_source_nominal_records()
                        .iter()
                        .find(|record| record.id() == id)
                        .ok_or(Error::MissingNominal(owner))?
                        .clone(),
                ),
                SourceNominalId::GenericTemplate(id) => crate::HirSourceNominalIdentity::Generic(
                    canonical
                        .type_source_generic_records()
                        .iter()
                        .find(|record| record.id() == id)
                        .ok_or(Error::MissingNominal(owner))?
                        .clone(),
                ),
            };
            let field_sources = interface
                .source_shape()
                .declared_fields()
                .iter()
                .map(|field| {
                    let (_, key) = canonical
                        .field_by_bytes(field.field().as_array())
                        .ok_or(Error::MissingNominalField(field.field()))?;
                    let storage = crate::NominalFieldStorage::from_key(key);
                    let name = match storage {
                        crate::NominalFieldStorage::PropertyBacking(property)
                        | crate::NominalFieldStorage::PropertyDelegate(property) => {
                            let (_, key) = canonical
                                .property_by_bytes(property.as_array())
                                .ok_or(Error::MissingNominalField(field.field()))?;
                            match key.name() {
                                DeclarationName::Named(name) => name.as_str().to_owned(),
                                _ => unreachable!("source properties have names"),
                            }
                        }
                        crate::NominalFieldStorage::Declared => match key.view() {
                            FieldIdentityView::SourceDeclared { name, .. } => {
                                name.as_str().to_owned()
                            }
                            _ => unreachable!("declared storage has a source field key"),
                        },
                        crate::NominalFieldStorage::Generated => format!("{:?}", field.field()),
                        crate::NominalFieldStorage::EnclosingInstance => "$outer".into(),
                        crate::NominalFieldStorage::CapturedEnvironment => "$captures".into(),
                    };
                    Ok(ImportedNominalFieldSource {
                        name,
                        storage,
                        origin: definition_origin(scoop_identity::DefinitionOriginSubject::Field(
                            field.field(),
                        ))?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let c_abi = foundation
                .native_boundary_type(match owner {
                    SourceNominalId::Concrete(id) => {
                        crate::NativeBoundaryNominalOwner::Concrete(id)
                    }
                    SourceNominalId::GenericTemplate(id) => {
                        crate::NativeBoundaryNominalOwner::GenericTemplate(id)
                    }
                })
                .map_or(NativeBoundaryCAbiV1::SourceRepresentation, |record| {
                    record.c_abi()
                });
            let variant_sources = match interface.source_shape() {
                crate::NominalSourceShapeV1::Enum(shape) => shape
                    .variants()
                    .iter()
                    .map(|variant| {
                        let (_, key) = canonical
                            .enum_variant_by_bytes(variant.variant().as_array())
                            .ok_or(Error::MissingEnumVariant(variant.variant()))?;
                        let name = key
                            .source_name()
                            .ok_or(Error::MissingEnumVariant(variant.variant()))?
                            .as_str()
                            .to_owned();
                        let fields = variant
                            .fields()
                            .iter()
                            .map(|field| {
                                let (_, key) = canonical
                                    .enum_variant_field_by_bytes(field.field().as_array())
                                    .ok_or(Error::MissingEnumField(field.field()))?;
                                let name = match key.selector() {
                                    EnumVariantFieldSelector::Named(name) => {
                                        name.as_str().to_owned()
                                    }
                                    EnumVariantFieldSelector::Positional { declaration_index } => {
                                        format!("_{declaration_index}")
                                    }
                                };
                                Ok(ImportedEnumVariantFieldSource {
                                    name,
                                    origin: definition_origin(
                                        scoop_identity::DefinitionOriginSubject::EnumVariantField(
                                            field.field(),
                                        ),
                                    )?,
                                })
                            })
                            .collect::<Result<Vec<_>, Error>>()?;
                        Ok(ImportedEnumVariantSource {
                            name,
                            fields,
                            origin: definition_origin(
                                scoop_identity::DefinitionOriginSubject::EnumVariant(
                                    variant.variant(),
                                ),
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>, Error>>()?,
                _ => Vec::new(),
            };
            Ok(Arc::new(ImportedNominalDeclaration {
                dispatch_slots: interface
                    .declaration_details()
                    .dispatch_order()
                    .declared_slots()
                    .map(|slot| {
                        canonical
                            .dispatch_slot_record(slot)
                            .cloned()
                            .ok_or(Error::MissingDispatchSlot(slot))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                identity,
                origin: crate::ExportDefinitionSourceV1::new(origin.origin().clone()),
                interface: interface.clone(),
                field_sources,
                variant_sources,
                c_abi,
            }))
        })
        .collect()
}
