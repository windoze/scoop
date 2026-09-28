//! Shared declarations and template bodies rooted in source and actual storage.

use std::collections::BTreeSet;

use super::nominal_interfaces::SharedSourceRoots;
use crate::{
    CanonicalExportGenericCallableBodiesV1, ExportHir, GenericTemplateProductionError,
    HirSourceNominalIdentity, SelectedImportedDependencySet, SourceNominalId, concrete,
};

/// One immutable projection reused by shape demands and interface publication.
#[derive(Debug)]
pub(crate) struct ExportSharedSource {
    pub(in crate::production) roots: SharedSourceRoots,
    pub(in crate::production) bodies: CanonicalExportGenericCallableBodiesV1,
    pub(in crate::production) initializations: crate::CanonicalExportGenericInitializationsV1,
}

impl ExportSharedSource {
    pub(crate) fn from_export(
        export: &ExportHir,
        imported: Option<&SelectedImportedDependencySet>,
    ) -> Result<Self, GenericTemplateProductionError> {
        let (roots, bodies, initializations) =
            SharedSourceRoots::with_callable_bodies(export, imported, &[])?;
        Ok(Self {
            roots,
            bodies,
            initializations,
        })
    }

    pub(crate) fn with_materialized_types(
        &self,
        export: &ExportHir,
        local: &concrete::Module,
        types: &[concrete::TypeId],
        imported: Option<&SelectedImportedDependencySet>,
    ) -> Result<Option<Self>, GenericTemplateProductionError> {
        let mut additional = BTreeSet::new();
        for ty in types {
            let origin = match local.types[*ty].kind {
                concrete::TypeKind::Struct(id) => &local.structs[id].origin,
                concrete::TypeKind::Class(id) => local
                    .objects
                    .iter()
                    .find(|(_, object)| object.backing_class == id)
                    .map_or(&local.classes[id].origin, |(_, object)| &object.origin),
                concrete::TypeKind::Enum(id) => &local.enums[id].origin,
                concrete::TypeKind::Interface(id) => &local.interfaces[id].origin,
                _ => continue,
            };
            let Some(source) = origin.source() else {
                continue;
            };
            if source.declaration().origin() != export.cone {
                continue;
            }
            let owner = match source {
                HirSourceNominalIdentity::Concrete(record) => {
                    SourceNominalId::Concrete(record.id())
                }
                HirSourceNominalIdentity::Generic(record) => {
                    SourceNominalId::GenericTemplate(record.id())
                }
            };
            if self.roots.nominals.values().binary_search(&owner).is_err() {
                additional.insert(owner);
            }
        }
        if additional.is_empty() {
            return Ok(None);
        }
        let additional = additional.into_iter().collect::<Vec<_>>();
        let (roots, bodies, initializations) =
            SharedSourceRoots::with_callable_bodies(export, imported, &additional)?;
        Ok(Some(Self {
            roots,
            bodies,
            initializations,
        }))
    }
}
