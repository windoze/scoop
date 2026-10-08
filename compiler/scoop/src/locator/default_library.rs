use super::*;

pub(crate) fn locate_build_dependency(
    coordinate: &ConeCoordinate,
    search_roots: &[ArtifactSearchRoot],
    sysroot: &Path,
    target: scoop_lir::ValidatedLirTargetSelection,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    match locate_from_search_roots(coordinate, search_roots, target) {
        Ok(prebuilt) => Ok(LocatedDependencyClaim::Prebuilt(Box::new(prebuilt))),
        Err(DependencyLocatorError::ArtifactNotFound { .. })
            if coordinate.group() == "scoop" && *coordinate != ConeCoordinate::reserved_core() =>
        {
            let manifest =
                load_dependency_manifest(coordinate, sysroot.join("lib").join(coordinate.name()))?;
            Ok(LocatedDependencyClaim::Source(Box::new(
                ManifestSourceProjection { manifest },
            )))
        }
        Err(error) => Err(error),
    }
}
