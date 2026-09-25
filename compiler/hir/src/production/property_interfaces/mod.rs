//! Projection of public properties and necessary source declarations.

use super::nominal_interfaces::SharedSourceRoots;
use super::signatures::HirInterfaceSignatureProjector;
use crate::{
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, ExportHir, HirPropertyIdentity,
    PropertyInterfaceRecordV1,
};
use scoop_identity::PropertyOwner as PersistentPropertyOwner;
use scoop_wire::WirePath;
use std::collections::HashSet;

mod accessors;
mod declaration;
mod errors;
mod signature;
mod support;

pub(in crate::production) use accessors::project_representation as source_property_representation;
pub use errors::{
    ExportPropertyAccessorBuildError, PropertyInterfaceBuildError, PropertyNominalOwnerKind,
};

impl CanonicalPropertyInterfacesV1 {
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, PropertyInterfaceBuildError> {
        let roots = SharedSourceRoots::from_export_hir(export)
            .map_err(PropertyInterfaceBuildError::Nominals)?;
        let nominals =
            CanonicalNominalInterfacesV1::from_export_hir_with_source_roots(export, &roots)
                .map_err(PropertyInterfaceBuildError::Nominals)?;
        Self::from_export_hir_with_nominals(export, &nominals, &roots)
    }

    pub(in crate::production) fn from_export_hir_with_nominals(
        export: &ExportHir,
        nominals: &CanonicalNominalInterfacesV1,
        roots: &SharedSourceRoots,
    ) -> Result<Self, PropertyInterfaceBuildError> {
        use PropertyInterfaceBuildError as Error;
        let path = WirePath::root().field(4);
        let projector = HirInterfaceSignatureProjector::new(export);
        let surface = &export.public_surface;

        let public_getters = surface
            .property_getters
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let public_setters = surface
            .property_setters
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut required = nominals
            .declared_source_properties()
            .map_err(Error::Inventory)?
            .into_keys()
            .collect::<std::collections::BTreeSet<_>>();
        for property in &roots.top_level_properties {
            required.insert(*property);
        }
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, surface.properties.len(), &path)
            .map_err(Error::Resource)?;
        for &id in &surface.properties {
            let data = declaration::project(export, &projector, id)?;
            required.remove(&data.declaration());
            let (access, setter) = accessors::public_lookup(
                export,
                id,
                data.declaration(),
                &public_getters,
                &public_setters,
            )?;
            let property = data.declaration();
            records.push(
                PropertyInterfaceRecordV1::from_declaration(data, access, setter)
                    .map_err(|source| Error::Record { property, source })?,
            );
        }
        let support = support::project(export, &projector, required)?;
        let table = Self::with_support(records, support).map_err(Error::Table)?;
        table
            .validate_member_declaration_inventory(nominals)
            .map_err(Error::Inventory)?;
        Ok(table)
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
