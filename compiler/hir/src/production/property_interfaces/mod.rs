//! Projection of the complete public HIR property surface.

use std::collections::HashSet;

use scoop_identity::PropertyOwner as PersistentPropertyOwner;

use super::signatures::HirInterfaceSignatureProjector;
use crate::{
    CanonicalPropertyInterfacesV1, ExportHir, HirPropertyIdentity, PropertyCapabilityV1,
    PropertyInterfaceRecordV1,
};

mod accessors;
mod errors;
mod signature;

pub use errors::{
    ExportPropertyAccessorBuildError, PropertyInterfaceBuildError, PropertyNominalOwnerKind,
};

pub(in crate::production) use accessors::project_representation as source_property_representation;
use accessors::{project_accessors, project_public_access, project_representation};
use signature::project_property_signature;

impl CanonicalPropertyInterfacesV1 {
    /// Projects exactly the current Cone's public logical properties.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, PropertyInterfaceBuildError> {
        let projector = HirInterfaceSignatureProjector::new(export);
        let public_getters = export
            .public_surface
            .property_getters
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let public_setters = export
            .public_surface
            .property_setters
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut records = Vec::with_capacity(export.public_surface.properties.len());

        for &property_id in &export.public_surface.properties {
            let property = arena_get(&export.properties, property_id).ok_or_else(|| {
                PropertyInterfaceBuildError::UnknownPublicProperty(raw_index(property_id))
            })?;
            let identity = export.property_identities.get(property_id).ok_or_else(|| {
                PropertyInterfaceBuildError::MissingPropertyIdentity(raw_index(property_id))
            })?;
            let declaration = persistent_property_owner(identity);
            signature::validate_declaration_identity(export, declaration, identity)?;

            let signature = project_property_signature(export, &projector, property_id, property)?;
            let access = project_public_access(&property.access).ok_or(
                PropertyInterfaceBuildError::InvalidPublicAccess(declaration),
            )?;
            let accessors = project_accessors(
                export,
                property_id,
                declaration,
                property.capability,
                access,
                &public_getters,
                &public_setters,
            )?;
            let capability = match accessors.setter {
                None => PropertyCapabilityV1::read_only(accessors.getter_id),
                Some(setter) => PropertyCapabilityV1::try_read_write(
                    accessors.getter_id,
                    setter.id,
                    setter.access,
                )
                .map_err(|source| PropertyInterfaceBuildError::Capability {
                    property: declaration,
                    source,
                })?,
            };
            let representation = project_representation(
                property,
                accessors.getter,
                accessors.setter.map(|setter| setter.declaration),
            )
            .map_err(|detail| PropertyInterfaceBuildError::Representation {
                property: declaration,
                detail,
            })?;
            let value_type = projector
                .map_type(property.ty, &signature.binders)
                .map_err(|source| PropertyInterfaceBuildError::Signature {
                    property: declaration,
                    source,
                })?;
            let record = PropertyInterfaceRecordV1::try_new(
                declaration,
                signature.owner,
                signature.type_parameters,
                signature.receiver,
                value_type,
                capability,
                representation,
                access,
            )
            .map_err(|source| PropertyInterfaceBuildError::Record {
                property: declaration,
                source,
            })?;
            records.push(record);
        }

        Self::try_new(records).map_err(PropertyInterfaceBuildError::Table)
    }
}

fn persistent_property_owner(identity: &HirPropertyIdentity) -> PersistentPropertyOwner {
    match identity {
        HirPropertyIdentity::Ordinary(record) => PersistentPropertyOwner::Property(record.id()),
        HirPropertyIdentity::Extension(record) => {
            PersistentPropertyOwner::ExtensionProperty(record.id())
        }
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
