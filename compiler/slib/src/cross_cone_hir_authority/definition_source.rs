//! Foundation-backed authority for exported definition locations.

use scoop_hir::{ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1};
use scoop_identity::ConeIdentity;

use super::CanonicalCrossConeHirSurfaceAuthority;
pub use scoop_hir::DefinitionSourceLocationValidationError as CrossConeHirDefinitionSourceAuthorityError;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn validate_current_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), CrossConeHirDefinitionSourceAuthorityError> {
        self.current_foundation
            .validate_definition_source_location(self.current, source)
    }
}

impl ExportDefinitionSourceSemanticAuthority<CrossConeHirDefinitionSourceAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), CrossConeHirDefinitionSourceAuthorityError> {
        self.validate_current_definition_source(source)
    }
}
