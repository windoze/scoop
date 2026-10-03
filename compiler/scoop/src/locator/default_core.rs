use super::{
    DependencyLocatorError, LocatedDependencyClaim, LocatorIoOperation, ManifestSourceProjection,
    load_dependency_manifest, locate_artifact_dependency,
};
use scoop_identity::ConeCoordinate;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_toolchain::TrustedCoreSlotLayoutV1;
use std::path::Path;

pub(crate) fn locate_default_core(
    sysroot: &Path,
    target: ValidatedLirTargetSelection,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    let layout = TrustedCoreSlotLayoutV1::new(sysroot, target);
    let coordinate = ConeCoordinate::reserved_core();
    match std::fs::symlink_metadata(layout.source_root()) {
        Ok(_) => load_dependency_manifest(&coordinate, layout.source_root().to_path_buf()).map(
            |manifest| {
                LocatedDependencyClaim::Source(Box::new(ManifestSourceProjection { manifest }))
            },
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            locate_artifact_dependency(layout.artifact().to_path_buf(), &coordinate, target)
        }
        Err(source) => Err(DependencyLocatorError::Io {
            operation: LocatorIoOperation::Inspect,
            path: layout.source_root().to_path_buf(),
            source,
        }),
    }
}
