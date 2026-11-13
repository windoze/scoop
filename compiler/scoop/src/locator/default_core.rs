use super::{DependencyLocatorError, PrebuiltArtifactProjection};
use crate::ArtifactCacheRoot;
use scoop_identity::ConeCoordinate;
use scoop_lir::ValidatedLirTargetSelection;
use std::path::Path;

pub(crate) fn locate_default_core(
    library_root: &Path,
    target: ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<PrebuiltArtifactProjection, DependencyLocatorError> {
    super::default_library::locate_sysroot_dependency(
        &ConeCoordinate::reserved_core(),
        library_root,
        target,
        cache,
    )
}
