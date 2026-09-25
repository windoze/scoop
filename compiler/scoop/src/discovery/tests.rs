use super::*;
use scoop_hir::CanonicalHirFoundation;
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_manifest::ManifestRootLocator;
use scoop_mir::CanonicalMirFoundation;
use scoop_protocol::TargetSelectionRequestV1;
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ProducerRecord,
};

fn write_manifest(root: &std::path::Path, name: &str, dependencies: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
            root.join("Cone.toml"),
            format!(
                "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"library\"\n{dependencies}"
            ),
        )
        .unwrap();
}

fn write_core(sysroot: &std::path::Path) {
    let root = sysroot.join("lib").join("scoop.core");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
            root.join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
        )
        .unwrap();
}

fn request(
    root: &std::path::Path,
    sysroot: &std::path::Path,
    search_roots: Vec<ArtifactSearchRoot>,
) -> BuildGraphRequest {
    request_with_limits(root, sysroot, search_roots)
}

fn request_with_limits(
    root: &std::path::Path,
    sysroot: &std::path::Path,
    search_roots: Vec<ArtifactSearchRoot>,
) -> BuildGraphRequest {
    BuildGraphRequest::new(
        crate::BuildRootInput::manifest(ManifestRootLocator::cone_directory(root)).unwrap(),
        search_roots,
        ArtifactCacheRoot::new(sysroot.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(sysroot.join("bin/scoopc")).unwrap(),
        DiagnosticsPolicy::Structured,
    )
    .unwrap()
}

fn foundation_artifact(coordinate: ConeCoordinate, producer: &str) -> Vec<u8> {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let cone = ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new(producer).unwrap(),
        cone,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap()
    .as_bytes()
    .to_vec()
}

#[test]
fn source_chain_is_discovered_before_any_source_tree_read() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "[dependencies]\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(&b, "b", "");

    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edge_count(), 5);
    assert_eq!(
        graph.node_representation(
            ConeCoordinate::new("test", "b", "1.0.0")
                .unwrap()
                .identity()
                .unwrap()
        ),
        Some(DiscoveredNodeRepresentation::ManifestSource)
    );
}

#[test]
fn diamond_source_claims_must_resolve_to_one_real_root() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let shared_a = temp.path().join("shared-a");
    let shared_b = temp.path().join("shared-b");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-a\" }\n",
    );
    write_manifest(
        &b,
        "b",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-b\" }\n",
    );
    write_manifest(&shared_a, "shared", "");
    write_manifest(&shared_b, "shared", "");

    assert!(matches!(
        request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover(),
        Err(BuildGraphDiscoveryError::ConflictingSourceLocator { .. })
    ));
}

#[cfg(unix)]
#[test]
fn source_symlink_aliases_to_one_real_root_merge() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let shared = temp.path().join("shared");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-alias-a\" }\n",
    );
    write_manifest(
        &b,
        "b",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-alias-b\" }\n",
    );
    write_manifest(&shared, "shared", "");
    symlink(&shared, temp.path().join("shared-alias-a")).unwrap();
    symlink(&shared, temp.path().join("shared-alias-b")).unwrap();

    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    let shared_identity = ConeCoordinate::new("test", "shared", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    assert_eq!(graph.node_count(), 5);
    assert_eq!(
        graph.node_representation(shared_identity),
        Some(DiscoveredNodeRepresentation::ManifestSource)
    );
}

#[test]
fn identical_artifact_claims_merge_but_different_fingerprints_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", artifact = \"../shared-a.slib\" }\n",
    );
    write_manifest(
        &b,
        "b",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", artifact = \"../shared-b.slib\" }\n",
    );
    let shared = ConeCoordinate::new("test", "shared", "1.0.0").unwrap();
    let bytes = foundation_artifact(shared.clone(), "same");
    std::fs::write(temp.path().join("shared-a.slib"), &bytes).unwrap();
    std::fs::write(temp.path().join("shared-b.slib"), &bytes).unwrap();

    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(
        graph.prebuilt_candidate_count(shared.identity().unwrap()),
        Some(2)
    );

    std::fs::write(
        temp.path().join("shared-b.slib"),
        foundation_artifact(shared, "different"),
    )
    .unwrap();
    assert!(matches!(
        request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover(),
        Err(BuildGraphDiscoveryError::AmbiguousArtifact { .. })
    ));
}

#[test]
fn source_and_artifact_claims_never_select_by_discovery_order() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let shared_source = temp.path().join("shared");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared\" }\n",
    );
    write_manifest(
        &b,
        "b",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", artifact = \"../shared.slib\" }\n",
    );
    write_manifest(&shared_source, "shared", "");
    let shared = ConeCoordinate::new("test", "shared", "1.0.0").unwrap();
    std::fs::write(
        temp.path().join("shared.slib"),
        foundation_artifact(shared, "artifact"),
    )
    .unwrap();

    assert!(matches!(
        request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover(),
        Err(BuildGraphDiscoveryError::ConflictingNodeRepresentation { .. })
    ));
}

#[test]
fn default_core_uses_the_common_dependency_coordinate_check() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let sysroot = temp.path().join("sysroot");
    write_manifest(&root, "root", "");
    write_manifest(&sysroot.join("lib/scoop.core"), "different-library", "");
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

mod core_locators;

mod core_dependencies;
