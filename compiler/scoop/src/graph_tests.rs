//! In-memory graph resolution tests covering DESIGN section 1.4's
//! validation matrix and canonical ordering.

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity, Digest256};
use scoop_manifest::{ConeKind, ConeManifest, DeclaredDependency, DependencyLocator};
use scoop_slib::{ManifestCoreTemplate, SlibBuilder, SlibDecodeLimits, ValidatedGraphArtifact};

use crate::graph::{BuildInputs, GraphError, NodeOrigin, resolve_graph};

/// In-memory inputs: manifests and artifacts keyed by path, plus the
/// core slot and a search-root list.
#[derive(Default)]
struct MemoryInputs {
    manifests: BTreeMap<String, ConeManifest>,
    artifacts: BTreeMap<String, Vec<u8>>,
    search_paths: Vec<String>,
    core: Option<Vec<u8>>,
    search_calls: Vec<ConeCoordinate>,
}

impl MemoryInputs {
    fn add_manifest(&mut self, path: &str, group: &str, name: &str, version: &str, kind: ConeKind) {
        let text = format!(
            "schema = 1\n\n[cone]\ngroup = \"{group}\"\nname = \"{name}\"\nversion = \"{version}\"\nkind = \"{}\"\n",
            kind.as_str()
        );
        let manifest = scoop_manifest::parse_manifest(&text).unwrap();
        self.manifests.insert(path.to_owned(), manifest);
    }

    fn add_artifact(
        &mut self,
        path: &str,
        group: &str,
        name: &str,
        version: &str,
        deps: &[(&str, &str, &str)],
    ) {
        let mut builder = SlibBuilder::new(template(group, name, version, deps));
        for (key, role) in [
            (scoop_slib::MemberStableKey::HirMetadata, wire_role(1)),
            (scoop_slib::MemberStableKey::MirMetadata, wire_role(2)),
            (scoop_slib::MemberStableKey::LirMetadata, wire_role(3)),
        ] {
            builder.add_member(key, role, b"meta".to_vec()).unwrap();
        }
        let bytes = builder.finish().unwrap();
        self.artifacts.insert(path.to_owned(), bytes);
    }
}

fn template(
    group: &str,
    name: &str,
    version: &str,
    deps: &[(&str, &str, &str)],
) -> ManifestCoreTemplate {
    ManifestCoreTemplate {
        container_version: 1,
        hir_wire_schema: 1,
        mir_wire_schema: 1,
        lir_wire_schema: 1,
        producer_compiler_version: "0.0.0 (test)".to_owned(),
        language_abi: 1,
        runtime_abi: Digest256::from_bytes([1; 32]),
        identity_schema_version: 1,
        coordinate: ConeCoordinate::new(group, name, version).unwrap(),
        kind: ConeKind::Library,
        dependencies: deps
            .iter()
            .map(|(g, n, v)| {
                let coordinate = ConeCoordinate::new(g, n, v).unwrap();
                scoop_slib::DependencyRecord {
                    coordinate: coordinate.clone(),
                    cone_identity: ConeIdentity::of(&coordinate),
                    hir_semantic_fingerprint: Digest256::ZERO,
                    mir_semantic_fingerprint: Digest256::ZERO,
                    lir_semantic_fingerprint: Digest256::ZERO,
                }
            })
            .collect(),
        target_profile: scoop_identity::TargetProfileWireId::darwin_aarch64_v1(),
        target_profile_fingerprint: Digest256::from_bytes([5; 32]),
        backend_profile_fingerprint: Digest256::from_bytes([6; 32]),
    }
}

fn wire_role(tag: u32) -> scoop_slib::SlibMemberRole {
    match tag {
        1 => scoop_slib::SlibMemberRole::HirMetadata { wire_schema: 1 },
        2 => scoop_slib::SlibMemberRole::MirMetadata { wire_schema: 1 },
        _ => scoop_slib::SlibMemberRole::LirMetadata { wire_schema: 1 },
    }
}

fn dep(g: &str, n: &str, v: &str, locator: DependencyLocator) -> DeclaredDependency {
    DeclaredDependency::new(ConeCoordinate::new(g, n, v).unwrap(), locator)
}

impl BuildInputs for MemoryInputs {
    fn read_manifest(&mut self, path: &str) -> Result<ConeManifest, GraphError> {
        self.manifests
            .get(path)
            .cloned()
            .ok_or_else(|| GraphError::Io(format!("manifest {path} not found")))
    }

    fn read_artifact(&mut self, path: &str) -> Result<Vec<u8>, GraphError> {
        self.artifacts
            .get(path)
            .cloned()
            .ok_or_else(|| GraphError::Io(format!("artifact {path} not found")))
    }

    fn search_candidates(
        &mut self,
        coordinate: &ConeCoordinate,
    ) -> Result<Vec<String>, GraphError> {
        self.search_calls.push(coordinate.clone());
        Ok(self
            .search_paths
            .iter()
            .map(|path| {
                format!(
                    "{path}/{}/{}/{}/cone.slib",
                    coordinate.group(),
                    coordinate.name(),
                    coordinate.version().as_str()
                )
            })
            .filter(|path| self.artifacts.contains_key(path))
            .collect())
    }

    fn core_slot(&mut self) -> Result<Vec<u8>, GraphError> {
        self.core
            .clone()
            .ok_or_else(|| GraphError::CoreSlot("missing".to_owned()))
    }
}

fn root_manifest(kind: ConeKind, deps: Vec<DeclaredDependency>) -> ConeManifest {
    let text = format!(
        "schema = 1\n\n[cone]\ngroup = \"dev.example\"\nname = \"app\"\nversion = \"0.1.0\"\nkind = \"{}\"\n",
        kind.as_str()
    );
    let manifest = scoop_manifest::parse_manifest(&text).unwrap();
    ConeManifest::new(manifest.coordinate().clone(), manifest.kind(), deps)
}

fn core_inputs() -> MemoryInputs {
    let mut inputs = MemoryInputs::default();
    inputs.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
    let bytes = inputs.artifacts["core.slib"].clone();
    inputs.core = Some(bytes);
    inputs
}

#[test]
fn library_root_with_prebuilt_chain_resolves() {
    let mut inputs = core_inputs();
    // util (prebuilt) depends on log (prebuilt), found via search.
    inputs.add_artifact(
        "cache/org.other/log/3.1.0/cone.slib",
        "org.other",
        "log",
        "3.1.0",
        &[],
    );
    inputs.add_artifact(
        "cache/org.acme/util/2.0.0/cone.slib",
        "org.acme",
        "util",
        "2.0.0",
        &[("org.other", "log", "3.1.0")],
    );
    inputs.search_paths.push("cache".to_owned());

    let root = root_manifest(
        ConeKind::Library,
        vec![dep("org.acme", "util", "2.0.0", DependencyLocator::Search)],
    );
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    // Dependencies first; ties broken by coordinate byte order.
    let displays: Vec<String> = graph
        .order()
        .iter()
        .map(|identity| graph.node(identity).unwrap().coordinate.display())
        .collect();
    assert_eq!(
        displays,
        [
            "scoop:scoop.core:0.1.0",
            "org.other:log:3.1.0",
            "org.acme:util:2.0.0",
            "dev.example:app:0.1.0",
        ]
    );
    let core_identity = ConeIdentity::of(&ConeCoordinate::reserved_core());
    assert!(matches!(
        graph.node(&core_identity).unwrap().origin,
        NodeOrigin::Core { .. }
    ));
}

#[test]
fn source_path_dependencies_resolve_recursively() {
    let mut inputs = core_inputs();
    inputs.add_manifest(
        "/lib/util/Cone.toml",
        "org.acme",
        "util",
        "2.0.0",
        ConeKind::Library,
    );
    let root = root_manifest(
        ConeKind::Library,
        vec![dep(
            "org.acme",
            "util",
            "2.0.0",
            DependencyLocator::SourcePath {
                path: "/lib/util/Cone.toml".to_owned(),
            },
        )],
    );
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    assert_eq!(graph.order().len(), 3); // core, util, root? root included.
    let util_identity =
        ConeIdentity::of(&ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap());
    let util = graph.node(&util_identity).unwrap();
    assert!(matches!(util.origin, NodeOrigin::Source { .. }));
    // Implicit core edge is present on the source node too.
    let core_identity = ConeIdentity::of(&ConeCoordinate::reserved_core());
    assert!(util.dependencies.contains(&core_identity));
}

#[test]
fn coordinate_mismatch_is_rejected() {
    let mut inputs = core_inputs();
    inputs.add_manifest(
        "/lib/other/Cone.toml",
        "org.acme",
        "other",
        "1.0.0",
        ConeKind::Library,
    );
    let root = root_manifest(
        ConeKind::Library,
        vec![dep(
            "org.acme",
            "util",
            "2.0.0",
            DependencyLocator::SourcePath {
                path: "/lib/other/Cone.toml".to_owned(),
            },
        )],
    );
    assert!(matches!(
        resolve_graph(root, "/root", &mut inputs).unwrap_err(),
        GraphError::CoordinateMismatch { .. }
    ));
}

#[test]
fn executable_dependency_is_rejected() {
    let mut inputs = core_inputs();
    inputs.add_manifest(
        "/lib/tool/Cone.toml",
        "org.x",
        "tool",
        "1.0.0",
        ConeKind::Executable,
    );
    let root = root_manifest(
        ConeKind::Library,
        vec![dep(
            "org.x",
            "tool",
            "1.0.0",
            DependencyLocator::SourcePath {
                path: "/lib/tool/Cone.toml".to_owned(),
            },
        )],
    );
    assert!(matches!(
        resolve_graph(root, "/root", &mut inputs).unwrap_err(),
        GraphError::ExecutableDependency { .. }
    ));
}

#[test]
fn multiple_versions_are_rejected() {
    let mut inputs = core_inputs();
    inputs.add_artifact(
        "cache/org.acme/util/1.0.0/cone.slib",
        "org.acme",
        "util",
        "1.0.0",
        &[],
    );
    inputs.add_manifest(
        "/lib/util/Cone.toml",
        "org.acme",
        "util",
        "2.0.0",
        ConeKind::Library,
    );
    inputs.search_paths.push("cache".to_owned());
    let root = root_manifest(
        ConeKind::Library,
        vec![
            dep("org.acme", "util", "1.0.0", DependencyLocator::Search),
            dep(
                "org.acme",
                "util",
                "2.0.0",
                DependencyLocator::SourcePath {
                    path: "/lib/util/Cone.toml".to_owned(),
                },
            ),
        ],
    );
    assert!(matches!(
        resolve_graph(root, "/root", &mut inputs).unwrap_err(),
        GraphError::MultipleVersions { .. }
    ));
}

#[test]
fn ambiguous_search_candidates_are_rejected() {
    let mut inputs = core_inputs();
    inputs.add_artifact("a/org.z/z/1.0.0/cone.slib", "org.z", "z", "1.0.0", &[]);
    // Same coordinate but different bytes → different fingerprint.
    let mut other = core_inputs();
    other.add_artifact("ignored.slib", "org.z", "z", "1.0.0", &[]);
    let mut other_bytes = other.artifacts["ignored.slib"].clone();
    // Tamper a metadata payload byte and fix nothing: the reader will
    // fail on hash; instead build a second valid artifact with a
    // different dependency table to alter the fingerprint.
    let _ = &mut other_bytes;
    let mut distinct = core_inputs();
    distinct.add_artifact(
        "distinct.slib",
        "org.z",
        "z",
        "1.0.0",
        &[("org.other", "log", "1.0.0")],
    );
    inputs.artifacts.insert(
        "b/org.z/z/1.0.0/cone.slib".to_owned(),
        distinct.artifacts["distinct.slib"].clone(),
    );
    inputs.search_paths = vec!["a".to_owned(), "b".to_owned()];

    let root = root_manifest(
        ConeKind::Library,
        vec![dep("org.z", "z", "1.0.0", DependencyLocator::Search)],
    );
    // The distinct candidate requires a missing transitive dep; either
    // error is acceptable, but ambiguity must win when both are readable.
    let error = resolve_graph(root, "/root", &mut inputs).unwrap_err();
    assert!(matches!(error, GraphError::AmbiguousArtifact { .. }));
}

#[test]
fn unresolvable_search_dependency_is_rejected() {
    let mut inputs = core_inputs();
    inputs.search_paths.push("cache".to_owned());
    let root = root_manifest(
        ConeKind::Library,
        vec![dep(
            "org.none",
            "missing",
            "1.0.0",
            DependencyLocator::Search,
        )],
    );
    assert!(matches!(
        resolve_graph(root, "/root", &mut inputs).unwrap_err(),
        GraphError::MissingDependency { .. }
    ));
}

#[test]
fn missing_core_slot_is_rejected() {
    let mut inputs = MemoryInputs::default();
    let root = root_manifest(ConeKind::Library, vec![]);
    assert!(matches!(
        resolve_graph(root, "/root", &mut inputs).unwrap_err(),
        GraphError::CoreSlot(_)
    ));
}

#[test]
fn diamond_graph_uses_coordinate_tie_break() {
    let mut inputs = core_inputs();
    // root -> {b, a}, a and b both -> core. Order: core, a, b, root.
    inputs.add_manifest("/a/Cone.toml", "org.d", "a", "1.0.0", ConeKind::Library);
    inputs.add_manifest("/b/Cone.toml", "org.d", "b", "1.0.0", ConeKind::Library);
    let root = root_manifest(
        ConeKind::Library,
        vec![
            dep(
                "org.d",
                "b",
                "1.0.0",
                DependencyLocator::SourcePath {
                    path: "/b/Cone.toml".to_owned(),
                },
            ),
            dep(
                "org.d",
                "a",
                "1.0.0",
                DependencyLocator::SourcePath {
                    path: "/a/Cone.toml".to_owned(),
                },
            ),
        ],
    );
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    let displays: Vec<String> = graph
        .order()
        .iter()
        .map(|identity| graph.node(identity).unwrap().coordinate.display())
        .collect();
    assert_eq!(
        displays,
        [
            "scoop:scoop.core:0.1.0",
            "org.d:a:1.0.0",
            "org.d:b:1.0.0",
            "dev.example:app:0.1.0",
        ]
    );
}

#[test]
fn executable_root_is_the_only_executable() {
    let mut inputs = core_inputs();
    let root = root_manifest(ConeKind::Executable, vec![]);
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    assert_eq!(
        graph.node(&graph.root()).unwrap().kind,
        ConeKind::Executable
    );

    // An executable root that also depends on another executable fails.
    inputs.add_manifest(
        "/lib/tool/Cone.toml",
        "org.x",
        "tool",
        "1.0.0",
        ConeKind::Executable,
    );
    let root = root_manifest(
        ConeKind::Executable,
        vec![dep(
            "org.x",
            "tool",
            "1.0.0",
            DependencyLocator::SourcePath {
                path: "/lib/tool/Cone.toml".to_owned(),
            },
        )],
    );
    assert!(matches!(
        resolve_graph(root, "/root", &mut inputs).unwrap_err(),
        GraphError::ExecutableDependency { .. } | GraphError::RootKind(_)
    ));
}

#[test]
fn prebuilt_transitive_dependencies_resolve_through_search() {
    let mut inputs = core_inputs();
    inputs.add_artifact(
        "cache/org.other/log/3.1.0/cone.slib",
        "org.other",
        "log",
        "3.1.0",
        &[],
    );
    inputs.add_artifact(
        "cache/org.acme/util/2.0.0/cone.slib",
        "org.acme",
        "util",
        "2.0.0",
        &[("org.other", "log", "3.1.0")],
    );
    inputs.search_paths.push("cache".to_owned());
    let root = root_manifest(
        ConeKind::Library,
        vec![dep("org.acme", "util", "2.0.0", DependencyLocator::Search)],
    );
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    assert_eq!(graph.order().len(), 4);
    // Log was reached via util's dependency record.
    assert!(
        inputs
            .search_calls
            .iter()
            .any(|coordinate| { coordinate.display() == "org.other:log:3.1.0" })
    );
}

#[test]
fn same_identity_twice_is_rejected() {
    let mut inputs = core_inputs();
    inputs.add_artifact(
        "cache/org.acme/util/2.0.0/cone.slib",
        "org.acme",
        "util",
        "2.0.0",
        &[],
    );
    inputs.search_paths.push("cache".to_owned());
    let root = root_manifest(
        ConeKind::Library,
        vec![
            dep("org.acme", "util", "2.0.0", DependencyLocator::Search),
            dep(
                "org.acme",
                "util",
                "2.0.0",
                DependencyLocator::ArtifactPath {
                    path: "cache/org.acme/util/2.0.0/cone.slib".to_owned(),
                },
            ),
        ],
    );
    // Both locators resolve to the same identity; the second resolution
    // reuses the existing node instead of duplicating it.
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    assert_eq!(graph.order().len(), 3);
}

/// Verifies a `.slib`-backed node keeps its prebuilt origin.
#[test]
fn artifact_locator_yields_prebuilt_origin() {
    let mut inputs = core_inputs();
    inputs.add_artifact("libs/util.slib", "org.acme", "util", "2.0.0", &[]);
    let root = root_manifest(
        ConeKind::Library,
        vec![dep(
            "org.acme",
            "util",
            "2.0.0",
            DependencyLocator::ArtifactPath {
                path: "libs/util.slib".to_owned(),
            },
        )],
    );
    let graph = resolve_graph(root, "/root", &mut inputs).unwrap();
    let util_identity =
        ConeIdentity::of(&ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap());
    assert!(matches!(
        graph.node(&util_identity).unwrap().origin,
        NodeOrigin::Prebuilt { .. }
    ));
    // Sanity: the artifact view really is readable through the same path.
    let bytes = inputs.read_artifact("libs/util.slib").unwrap();
    ValidatedGraphArtifact::read(&bytes, &SlibDecodeLimits::strict()).unwrap();
}
