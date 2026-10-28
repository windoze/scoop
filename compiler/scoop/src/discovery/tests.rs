use super::*;
use scoop_protocol::TargetSelectionRequestV1;
use scoop_slib::{ConeKind, ConeRecord, ConeSourceForm};

fn write_core(sysroot: &std::path::Path) {
    let root = sysroot.join("src").join("scoop.core");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
            root.join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
        )
        .unwrap();
    let selection = crate::test_artifacts::host_target();
    let directory = scoop_toolchain::target_library_root(sysroot, selection.target());
    std::fs::create_dir_all(&directory).unwrap();
    let artifact = crate::test_artifacts::manifest_archive(
        selection,
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "core",
        Vec::new(),
    );
    std::fs::write(directory.join("scoop.core.slib"), artifact.as_bytes()).unwrap();
    let standard = ConeCoordinate::new("scoop", "scoop.std", "0.1.0").unwrap();
    std::fs::write(
        directory.join("scoop.std.slib"),
        manifest_artifact(standard, "std"),
    )
    .unwrap();
}

fn request(
    root: &std::path::Path,
    sysroot: &std::path::Path,
    search_roots: Vec<ArtifactSearchRoot>,
) -> BuildGraphRequest {
    BuildGraphRequest::new(
        crate::BuildRootInput::single_file(SingleFileLocator::from_path(root).unwrap()),
        search_roots,
        ArtifactCacheRoot::new(sysroot.with_extension("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new(scoop_toolchain::host_target_triple().unwrap().into())
            .unwrap(),
        PairedScoopcLocator::new(sysroot.join("bin/scoopc")).unwrap(),
        DiagnosticsPolicy::Structured,
    )
    .unwrap()
}

fn manifest_artifact(coordinate: ConeCoordinate, producer: &str) -> Vec<u8> {
    let cone = ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
    crate::test_artifacts::manifest_archive(
        crate::test_artifacts::host_target(),
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        cone,
        producer,
        Vec::new(),
    )
    .into_bytes()
}

#[test]
fn default_core_uses_the_common_dependency_coordinate_check() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root.scoop");
    let sysroot = temp.path().join("sysroot");
    std::fs::write(&root, "fun main() {}\n").unwrap();
    write_core(&sysroot);
    let path = scoop_toolchain::target_library_root(
        &sysroot,
        crate::test_artifacts::host_target().target(),
    )
    .join("scoop.core.slib");
    std::fs::write(
        path,
        manifest_artifact(
            ConeCoordinate::new("test", "different-library", "1.0.0").unwrap(),
            "different",
        ),
    )
    .unwrap();
    let error = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap_err();
    assert!(matches!(error,
        BuildGraphDiscoveryError::Locator(ref source)
            if matches!(source.as_ref(), DependencyLocatorError::CoordinateMismatch { expected, actual, .. }
                if expected.as_ref() == &ConeCoordinate::reserved_core()
                && actual.name() == "different-library")
    ));
    assert_eq!(
        crate::ClassifyBuildFailure::classification(&error).code(),
        Some(crate::BuildDiagnosticCode::LOCATOR_COORDINATE_MISMATCH)
    );
}
