//! Shared declarations and template bodies collected before concretization.

use super::nominal_interfaces::SharedSourceRoots;
use crate::{
    CanonicalExportGenericCallableBodiesV1, ExportHir, GenericTemplateProductionError,
    SelectedImportedDependencySet,
};

/// One immutable projection reused by shape demands and interface publication.
#[derive(Debug)]
pub(crate) struct ExportSharedSource {
    pub(in crate::production) roots: SharedSourceRoots,
    pub(in crate::production) bodies: CanonicalExportGenericCallableBodiesV1,
}

impl ExportSharedSource {
    pub(crate) fn from_export(
        export: &ExportHir,
        imported: Option<&SelectedImportedDependencySet>,
    ) -> Result<Self, GenericTemplateProductionError> {
        let (roots, bodies) = SharedSourceRoots::with_callable_bodies(export, imported)?;
        Ok(Self { roots, bodies })
    }
}
