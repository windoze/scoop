//! Owned declaration data used by dependency type resolution.

use std::sync::Arc;

use scoop_identity::{
    CborIdentityRecord, DeclarationName, FieldIdentityView, PersistentTypeId, SourceDeclarationKey,
};

use super::{ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError};
use crate::{NativeBoundaryCAbiV1, NominalInterfaceRecordV1, SourceNominalId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedNominalDeclaration {
    pub identity: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    pub interface: NominalInterfaceRecordV1,
    pub field_names: Vec<String>,
    pub c_abi: NativeBoundaryCAbiV1,
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
            Ok(Arc::new(ImportedNominalDeclaration {
                identity,
                interface: interface.clone(),
                field_names,
                c_abi,
            }))
        })
        .collect()
}
