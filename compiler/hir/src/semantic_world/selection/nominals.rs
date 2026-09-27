//! Owned declaration data used by dependency type resolution.

use std::sync::Arc;

use scoop_identity::{
    CborIdentityRecord, DeclarationName, EnumVariantFieldSelector, FieldIdentityView,
    PersistentTypeId, SourceDeclarationKey,
};

use super::{ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError};
use crate::{NativeBoundaryCAbiV1, NominalInterfaceRecordV1, SourceNominalId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedNominalDeclaration {
    pub identity: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    pub interface: NominalInterfaceRecordV1,
    pub field_names: Vec<String>,
    pub variant_names: Vec<ImportedEnumVariantNames>,
    pub c_abi: NativeBoundaryCAbiV1,
    pub dispatch_slots: Vec<
        CborIdentityRecord<
            scoop_identity::PersistentDispatchSlotId,
            scoop_identity::DispatchSlotKey,
        >,
    >,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedEnumVariantNames {
    pub name: String,
    pub fields: Vec<String>,
}

impl ImportedNominalDeclaration {
    pub fn name(&self) -> &str {
        match self.identity.key().name() {
            DeclarationName::Named(name) => name.as_str(),
            _ => unreachable!("source nominal declarations have names"),
        }
    }
}

impl ImportedDependencySelectionPlan {
    /// Queries declaration identity directly, including private storage support.
    /// Source visibility is checked by the caller's binding or member lookup.
    pub fn nominal(&self, id: PersistentTypeId) -> Option<&Arc<ImportedNominalDeclaration>> {
        self.catalog.nominals.get(&id)
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
    provider
        .interface()
        .nominal_interfaces()
        .all_records()
        .filter_map(|interface| match interface.declaration() {
            SourceNominalId::Concrete(id) => Some((id, interface)),
            SourceNominalId::GenericTemplate(_) => None,
        })
        .map(|(id, interface)| {
            let identity = canonical
                .type_source_nominal_records()
                .iter()
                .find(|record| record.id() == id)
                .ok_or(Error::MissingNominal(id))?
                .clone();
            let field_names = interface
                .source_shape()
                .declared_fields()
                .iter()
                .map(|field| {
                    let (_, key) = canonical
                        .field_by_bytes(field.field().as_array())
                        .ok_or(Error::MissingNominalField(field.field()))?;
                    Ok(match key.view() {
                        FieldIdentityView::SourceDeclared { name, .. } => name.as_str().to_owned(),
                        FieldIdentityView::SourcePropertyBacking { property, .. }
                        | FieldIdentityView::SourcePropertyDelegate { property, .. } => {
                            let (_, key) = canonical
                                .property_by_bytes(property.as_array())
                                .ok_or(Error::MissingNominalField(field.field()))?;
                            match key.name() {
                                DeclarationName::Named(name) => name.as_str().to_owned(),
                                _ => unreachable!("source properties have names"),
                            }
                        }
                        FieldIdentityView::Generated { .. } => format!("{:?}", field.field()),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let c_abi = foundation
                .native_boundary_type(crate::NativeBoundaryNominalOwner::Concrete(id))
                .map_or(NativeBoundaryCAbiV1::SourceRepresentation, |record| {
                    record.c_abi()
                });
            let variant_names = match interface.source_shape() {
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
                                Ok(match key.selector() {
                                    EnumVariantFieldSelector::Named(name) => {
                                        name.as_str().to_owned()
                                    }
                                    EnumVariantFieldSelector::Positional { declaration_index } => {
                                        format!("_{declaration_index}")
                                    }
                                })
                            })
                            .collect::<Result<Vec<_>, Error>>()?;
                        Ok(ImportedEnumVariantNames { name, fields })
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
                interface: interface.clone(),
                field_names,
                variant_names,
                c_abi,
            }))
        })
        .collect()
}
