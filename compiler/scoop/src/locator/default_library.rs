use super::*;

pub(crate) fn locate_build_dependency(
    coordinate: &ConeCoordinate,
    search_roots: &[ArtifactSearchRoot],
    sysroot: &Path,
    target: scoop_lir::ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    match locate_from_search_roots(coordinate, search_roots, target, cache) {
        Ok(prebuilt) => Ok(LocatedDependencyClaim::Prebuilt(Box::new(prebuilt))),
        Err(DependencyLocatorError::ArtifactNotFound { .. })
            if coordinate.group() == "scoop" && *coordinate != ConeCoordinate::reserved_core() =>
        {
            locate_sysroot_dependency(coordinate, sysroot, target, cache)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn locate_sysroot_dependency(
    coordinate: &ConeCoordinate,
    sysroot: &Path,
    target: scoop_lir::ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    let path = scoop_toolchain::target_library_root(sysroot, target.target())
        .join(format!("{}.slib", coordinate.name()));
    locate_artifact_dependency(path.clone(), coordinate, target, cache).map_err(|error| {
        if matches!(&error, DependencyLocatorError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
        {
            DependencyLocatorError::SysrootArtifactNotFound {
                coordinate: coordinate.clone(),
                target: target.target().contract().canonical_triple(),
                path,
            }
        } else {
            error
        }
    })
}
