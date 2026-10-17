use super::{DependencyLocatorError, LocatedDependencyClaim};
use crate::ArtifactCacheRoot;
use scoop_identity::ConeCoordinate;
use scoop_lir::ValidatedLirTargetSelection;
use std::path::Path;

pub(crate) fn locate_default_core(
    sysroot: &Path,
    target: ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    super::default_library::locate_sysroot_dependency(
        &ConeCoordinate::reserved_core(),
        sysroot,
        target,
        cache,
    )
}
