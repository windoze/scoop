//! Shared declarations and template bodies rooted in source and actual storage.

use std::collections::BTreeSet;

use super::nominal_interfaces::SharedSourceRoots;
use crate::{
    CanonicalExportGenericCallableBodiesV1, ExportHir, GenericTemplateProductionError,
    SelectedImportedDependencySet, SourceNominalId,
};

/// One immutable projection reused by shape demands and interface publication.
#[derive(Debug)]
pub struct ExportSharedSource {
    pub(in crate::production) roots: SharedSourceRoots,
    pub(in crate::production) bodies: CanonicalExportGenericCallableBodiesV1,
    pub(in crate::production) initializations: crate::CanonicalExportGenericInitializationsV1,
    pub(in crate::production) delegates: crate::CanonicalExportGenericDelegatesV1,
}

impl ExportSharedSource {
    pub(crate) fn from_export(
        export: &ExportHir,
        imported: Option<&SelectedImportedDependencySet>,
    ) -> Result<Self, GenericTemplateProductionError> {
        let (roots, bodies, initializations, delegates) =
            SharedSourceRoots::with_callable_bodies(export, imported, &[])?;
        Ok(Self {
            roots,
            bodies,
            initializations,
            delegates,
        })
    }

    pub fn with_materialized_nominals(
        &self,
        export: &ExportHir,
        nominals: &[SourceNominalId],
        imported: Option<&SelectedImportedDependencySet>,
    ) -> Result<Option<Self>, GenericTemplateProductionError> {
        let mut additional = BTreeSet::new();
        for &owner in nominals {
            if self.roots.nominals.values().binary_search(&owner).is_err() {
                additional.insert(owner);
            }
        }
        if additional.is_empty() {
            return Ok(None);
        }
        let additional = additional.into_iter().collect::<Vec<_>>();
        let (roots, bodies, initializations, delegates) =
            SharedSourceRoots::with_callable_bodies(export, imported, &additional)?;
        Ok(Some(Self {
            roots,
            bodies,
            initializations,
            delegates,
        }))
    }
}
