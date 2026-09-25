use std::path::Path;

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_manifest::{ManifestRootLocator, SingleFileLocator};
use scoop_mir::CanonicalMirFoundation;
use scoop_protocol::TargetSelectionRequestV1;
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ProducerRecord, probe_prebuilt_manifest_summary,
};
use scoop_wire::DecodeLimits;

use super::*;
use crate::discovery::{DiscoveredDependencyEdge, EdgeOrigin, ManifestLocatorKind};
use crate::{
    ArtifactCacheRoot, BuildGraphRequest, BuildLimitsProfileV1, BuildRootInput, DiagnosticsPolicy,
    PairedScoopcLocator, TrustedSysrootRoot,
};

fn write_manifest(root: &Path, name: &str, version: &str, kind: &str, dependencies: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"{version}\"\nkind = \"{kind}\"\n{dependencies}"
        ),
    )
    .unwrap();
}

fn write_core(sysroot: &Path) {
    let root = sysroot.join("lib").join("scoop.core");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
    )
    .unwrap();
}

fn request(root: &Path, sysroot: &Path) -> BuildGraphRequest {
    request_with_limits(root, sysroot, BuildLimitsProfileV1::M23_DEFAULT)
}

fn request_with_limits(
    root: &Path,
    sysroot: &Path,
    limits: BuildLimitsProfileV1,
) -> BuildGraphRequest {
    BuildGraphRequest::new(
        BuildRootInput::manifest(ManifestRootLocator::cone_directory(root)).unwrap(),
        vec![],
        ArtifactCacheRoot::new(sysroot.join("cache")).unwrap(),
        TrustedSysrootRoot::new(sysroot).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(sysroot.join("bin/scoopc")).unwrap(),
        DiagnosticsPolicy::Structured,
        limits,
    )
    .unwrap()
}

fn coordinate(name: &str, version: &str) -> ConeCoordinate {
    ConeCoordinate::new("test", name, version).unwrap()
}

fn foundation_artifact_with_core(coordinate: ConeCoordinate) -> Vec<u8> {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let seed = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("graph-test-seed").unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let fingerprints =
        probe_prebuilt_manifest_summary(seed.as_bytes(), DecodeLimits::M23_DEFAULT, selection)
            .unwrap()
            .semantic_fingerprints();
    let cone = ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
    let core = DependencyRecord::new(
        ConeCoordinate::reserved_core(),
        fingerprints.hir(),
        fingerprints.mir(),
        fingerprints.lir(),
    )
    .unwrap();
    IdentityFoundationArtifact::write(
        IdentityFoundationArtifactInput::new(
            ProducerRecord::new("graph-test").unwrap(),
            cone,
            selection,
            &hir,
            &mir,
            &lir,
        )
        .with_direct_dependencies(vec![core]),
    )
    .unwrap()
    .as_bytes()
    .to_vec()
}

fn identities_as_coordinates(graph: &ResolvedBuildGraph, values: &[ConeIdentity]) -> Vec<String> {
    values
        .iter()
        .map(|identity| graph.node_coordinate(*identity).unwrap().to_string())
        .collect()
}

#[test]
fn diamond_has_canonical_order_and_exact_direct_support_projection() {
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
        "1.0.0",
        "library",
        "[dependencies]\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared\" }\n",
    );
    write_manifest(
        &b,
        "b",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared\" }\n",
    );
    write_manifest(&shared, "shared", "1.0.0", "library", "");

    let graph = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(
        identities_as_coordinates(&graph, graph.dependency_first()),
        [
            "scoop:scoop.core:0.1.0",
            "test:shared:1.0.0",
            "test:a:1.0.0",
            "test:b:1.0.0",
            "test:root:1.0.0",
        ]
    );
    let root_inputs = graph.source_dependencies(graph.root_identity()).unwrap();
    assert_eq!(
        identities_as_coordinates(&graph, root_inputs.direct()),
        ["test:a:1.0.0", "test:b:1.0.0"]
    );
    assert_eq!(
        identities_as_coordinates(&graph, root_inputs.support()),
        ["test:shared:1.0.0"]
    );
}

#[test]
fn canonical_cycle_diagnostic_orders_sccs_and_edges() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    for name in ["a", "b", "c", "d"] {
        let dependency = match name {
            "a" => "b",
            "b" => "a",
            "c" => "d",
            "d" => "c",
            _ => unreachable!(),
        };
        write_manifest(
            &temp.path().join(name),
            name,
            "1.0.0",
            "library",
            &format!(
                "[dependencies]\n\"test:{dependency}\" = {{ version = \"1.0.0\", path = \"../{dependency}\" }}\n"
            ),
        );
    }
    write_manifest(
        &root,
        "root",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:c\" = { version = \"1.0.0\", path = \"../c\" }\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n",
    );

    let error = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap_err();
    let ResolveBuildGraphError::Cycles(cycles) = error else {
        panic!("expected cycles, found {error}");
    };
    assert_eq!(cycles.len(), 2);
    let paths: Vec<Vec<String>> = cycles
        .iter()
        .map(|cycle| {
            let mut coordinates = vec![cycle.steps()[0].dependent_coordinate().to_string()];
            coordinates.extend(
                cycle
                    .steps()
                    .iter()
                    .map(|step| step.dependency_coordinate().to_string()),
            );
            coordinates
        })
        .collect();
    assert_eq!(
        paths,
        [
            vec!["test:a:1.0.0", "test:b:1.0.0", "test:a:1.0.0"],
            vec!["test:c:1.0.0", "test:d:1.0.0", "test:c:1.0.0"],
        ]
    );
    assert!(
        cycles
            .iter()
            .all(|cycle| { cycle.steps().iter().all(|step| !step.origins().is_empty()) })
    );
}

#[test]
fn multiple_versions_report_every_incoming_claim_canonically() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    write_manifest(
        &root,
        "root",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:shared\" = { version = \"2.0.0\", path = \"../shared-v2\" }\n",
    );
    write_manifest(
        &b,
        "b",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-v1\" }\n",
    );
    write_manifest(
        &temp.path().join("shared-v1"),
        "shared",
        "1.0.0",
        "library",
        "",
    );
    write_manifest(
        &temp.path().join("shared-v2"),
        "shared",
        "2.0.0",
        "library",
        "",
    );

    let error = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap_err();
    let ResolveBuildGraphError::MultipleVersions(conflicts) = error else {
        panic!("expected multiple versions, found {error}");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].group(), "test");
    assert_eq!(conflicts[0].name(), "shared");
    assert_eq!(
        conflicts[0]
            .claims()
            .iter()
            .map(|claim| claim.coordinate().version())
            .collect::<Vec<_>>(),
        ["1.0.0", "2.0.0"]
    );
    assert!(
        conflicts[0]
            .claims()
            .iter()
            .all(|claim| claim.incoming_origins().len() == 1)
    );
}

#[test]
fn executable_root_and_single_file_are_valid_roots_only() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    write_manifest(&root, "root", "1.0.0", "executable", "");
    let manifest = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(manifest.root_kind(), RequestedConeKind::Executable);

    let source = temp.path().join("main.scoop");
    std::fs::write(&source, "fun main() = Unit\n").unwrap();
    let single_file = BuildGraphRequest::new(
        BuildRootInput::single_file(SingleFileLocator::from_path(&source).unwrap()),
        vec![],
        ArtifactCacheRoot::new(sysroot.join("cache")).unwrap(),
        TrustedSysrootRoot::new(&sysroot).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(sysroot.join("bin/scoopc")).unwrap(),
        DiagnosticsPolicy::Structured,
        BuildLimitsProfileV1::M23_DEFAULT,
    )
    .unwrap()
    .load_root()
    .unwrap()
    .discover()
    .unwrap()
    .resolve()
    .unwrap();
    assert_eq!(single_file.node_count(), 2);
    assert_eq!(single_file.edge_count(), 1);
    assert_eq!(single_file.root_identity(), ConeIdentity::SINGLE_FILE);
    assert_eq!(single_file.root_kind(), RequestedConeKind::Executable);
    let inputs = single_file
        .source_dependencies(ConeIdentity::SINGLE_FILE)
        .unwrap();
    assert!(inputs.direct().is_empty());
    assert!(inputs.support().is_empty());
}

#[test]
fn chain_support_excludes_direct_and_trusted_core() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    write_manifest(
        &root,
        "root",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n",
    );
    write_manifest(
        &a,
        "a",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
    );
    write_manifest(&b, "b", "1.0.0", "library", "");
    let graph = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap();
    let inputs = graph.source_dependencies(graph.root_identity()).unwrap();
    assert_eq!(
        inputs.direct(),
        &[coordinate("a", "1.0.0").identity().unwrap()]
    );
    assert_eq!(
        inputs.support(),
        &[coordinate("b", "1.0.0").identity().unwrap()]
    );
}

#[test]
fn prebuilt_node_must_bind_its_recorded_core_edge() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:prebuilt\" = { version = \"1.0.0\", artifact = \"../prebuilt.slib\" }\n",
    );
    let prebuilt_coordinate = coordinate("prebuilt", "1.0.0");
    std::fs::write(
        temp.path().join("prebuilt.slib"),
        foundation_artifact_with_core(prebuilt_coordinate.clone()),
    )
    .unwrap();

    let graph = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(
        identities_as_coordinates(&graph, graph.dependency_first()),
        [
            "scoop:scoop.core:0.1.0",
            "test:prebuilt:1.0.0",
            "test:root:1.0.0",
        ]
    );
    let inputs = graph.source_dependencies(graph.root_identity()).unwrap();
    assert_eq!(inputs.direct(), &[prebuilt_coordinate.identity().unwrap()]);
    assert!(inputs.support().is_empty());
    assert!(
        graph
            .edges()
            .find(|edge| {
                edge.dependent() == prebuilt_coordinate.identity().unwrap()
                    && edge.dependency() == ConeIdentity::CORE
            })
            .unwrap()
            .expected_semantic()
            .is_some()
    );
}

#[test]
fn self_cycle_uses_the_same_canonical_cycle_shape() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    write_manifest(&root, "root", "1.0.0", "library", "");
    let discovered = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    let root_identity = discovered.root_identity();
    let root_coordinate = discovered.node_coordinate(root_identity).unwrap().clone();
    let mut parts = discovered.into_parts();

    parts.edges.insert(
        (root_identity, root_identity),
        DiscoveredDependencyEdge::new(
            root_identity,
            root_identity,
            root_coordinate,
            EdgeOrigin::ManifestDeclaration {
                manifest: root.join("Cone.toml"),
                span: 0..0,
                locator_kind: ManifestLocatorKind::SourcePath,
            },
            None,
        ),
    );

    let error = GraphResolver::new(parts).resolve().unwrap_err();
    let ResolveBuildGraphError::Cycles(cycles) = error else {
        panic!("expected self cycle, found {error}");
    };
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].steps().len(), 1);
    assert_eq!(cycles[0].steps()[0].dependent(), root_identity);
    assert_eq!(cycles[0].steps()[0].dependency(), root_identity);
}

#[test]
fn unreachable_nodes_are_rejected_in_coordinate_order() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    let a = temp.path().join("a");
    write_manifest(
        &root,
        "root",
        "1.0.0",
        "library",
        "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n",
    );
    write_manifest(&a, "a", "1.0.0", "library", "");
    let discovered = request(&root, &sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    let root_identity = discovered.root_identity();
    let a_identity = coordinate("a", "1.0.0").identity().unwrap();
    let mut parts = discovered.into_parts();
    parts.edges.remove(&(root_identity, a_identity));

    let error = GraphResolver::new(parts).resolve().unwrap_err();
    let ResolveBuildGraphError::UnreachableNodes(coordinates) = error else {
        panic!("expected unreachable node, found {error}");
    };
    assert_eq!(coordinates, [coordinate("a", "1.0.0")]);
}

#[test]
fn core_manifest_is_an_ordinary_build_root_without_a_default_sysroot() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    write_core(&source);
    let root = temp.path().join("edited-core");
    std::fs::rename(source.join("lib/scoop.core"), &root).unwrap();
    let missing_sysroot = temp.path().join("missing-sysroot");
    let discovered = request(&root, &missing_sysroot)
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(discovered.root_identity(), ConeIdentity::CORE);
    assert_eq!(discovered.node_count(), 1);
    assert_eq!(discovered.edge_count(), 0);
    assert_eq!(
        discovered.node_representation(ConeIdentity::CORE),
        Some(crate::DiscoveredNodeRepresentation::ManifestSource)
    );
    let resolved = discovered.resolve().unwrap();
    assert_eq!(resolved.dependency_first(), &[ConeIdentity::CORE]);
    assert_eq!(
        resolved.node_representation(ConeIdentity::CORE),
        Some(ResolvedNodeRepresentation::ManifestSource)
    );
    assert!(!missing_sysroot.exists());
}
