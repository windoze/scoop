//! Scheduler tests with a recording fake compiler, programmable reuse
//! gate and in-memory cache (DESIGN sections 1.4 and 5.5).

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity, Digest256};
use scoop_manifest::{ConeKind, ConeManifest, DeclaredDependency, DependencyLocator};
use scoop_slib::{ManifestCoreTemplate, MemberStableKey, SlibBuilder, SlibMemberRole};

use crate::graph::{BuildInputs, GraphError, resolve_graph};
use crate::graph_tests::{MemoryInputs, root_manifest};
use crate::schedule::{
    ArtifactCache, BuildTask, CompilerKeyIdentity, DependencyLayers, ReuseGate, ScheduleError,
    ScheduleInputs, ScoopcRunner, schedule_build,
};

/// Fake compiler: records tasks, returns valid synthetic artifacts,
/// optionally fails selected coordinates.
struct RecordingRunner {
    invocations: Vec<BuildTask>,
    fail: Vec<String>,
}

impl RecordingRunner {
    fn new() -> Self {
        RecordingRunner {
            invocations: Vec::new(),
            fail: Vec::new(),
        }
    }

    fn artifact_for_owned(coordinate: &str, deps: &[(String, String, String)]) -> Vec<u8> {
        let (group, name, version) = split_display(coordinate);
        let borrowed: Vec<(&str, &str, &str)> = deps
            .iter()
            .map(|(g, n, v)| (g.as_str(), n.as_str(), v.as_str()))
            .collect();
        let mut builder = SlibBuilder::new(core_template(group, name, version, &borrowed));
        for (key, role) in [
            (
                MemberStableKey::HirMetadata,
                SlibMemberRole::HirMetadata { wire_schema: 1 },
            ),
            (
                MemberStableKey::MirMetadata,
                SlibMemberRole::MirMetadata { wire_schema: 1 },
            ),
            (
                MemberStableKey::LirMetadata,
                SlibMemberRole::LirMetadata { wire_schema: 1 },
            ),
        ] {
            builder.add_member(key, role, b"meta".to_vec()).unwrap();
        }
        builder.finish().unwrap()
    }
}

fn split_display(display: &str) -> (&str, &str, &str) {
    let (group, rest) = display.split_once(':').unwrap();
    let (name, version) = rest.split_once(':').unwrap();
    (group, name, version)
}

fn core_template(
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

impl ScoopcRunner for RecordingRunner {
    fn build(
        &mut self,
        task: &BuildTask,
    ) -> Result<Vec<u8>, Vec<crate::schedule::SchedulerDiagnostic>> {
        self.invocations.push(task.clone());
        if self.fail.contains(&task.coordinate) {
            return Err(vec![crate::schedule::SchedulerDiagnostic {
                message: "synthetic failure".to_owned(),
            }]);
        }
        // Derive dependencies from the task's direct paths.
        let deps: Vec<(String, String, String)> = task
            .direct_slibs
            .iter()
            .filter_map(|path| path.strip_prefix("cache/"))
            .filter_map(|rest| rest.strip_suffix("/cone.slib"))
            .filter(|display| *display != "scoop:scoop.core:0.1.0")
            .map(|display| {
                let (g, n, v) = split_display(display);
                ((*g).to_owned(), (*n).to_owned(), (*v).to_owned())
            })
            .collect();
        Ok(Self::artifact_for_owned(&task.coordinate, &deps))
    }
}

/// Gate that validates the graph view and can be told to fail selected
/// coordinates.
struct ProgrammableGate {
    fail: Vec<String>,
}

impl ReuseGate for ProgrammableGate {
    fn validate(&mut self, bytes: &[u8], expected: &ConeCoordinate) -> Result<(), String> {
        if self.fail.contains(&expected.display()) {
            return Err("gate rejection".to_owned());
        }
        let view = scoop_slib::ValidatedGraphArtifact::read(
            bytes,
            &scoop_slib::SlibDecodeLimits::strict(),
        )
        .map_err(|error| error.to_string())?;
        if view.coordinate() != expected {
            return Err(format!(
                "coordinate {} != {}",
                view.coordinate().display(),
                expected.display()
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct MemoryCache {
    entries: BTreeMap<String, Vec<u8>>,
}

impl ArtifactCache for MemoryCache {
    fn lookup(&mut self, key: &Digest256) -> Option<Vec<u8>> {
        self.entries.get(&key.to_hex()).cloned()
    }

    fn store(&mut self, key: &Digest256, bytes: &[u8]) {
        self.entries.insert(key.to_hex(), bytes.to_vec());
    }
}

/// Schedule inputs over the graph-test memory harness.
struct MemoryScheduleInputs {
    base: MemoryInputs,
    sources: BTreeMap<String, Vec<(String, String)>>,
}

impl BuildInputs for MemoryScheduleInputs {
    fn read_manifest(&mut self, path: &str) -> Result<ConeManifest, GraphError> {
        self.base.read_manifest(path)
    }
    fn read_artifact(&mut self, path: &str) -> Result<Vec<u8>, GraphError> {
        self.base.read_artifact(path)
    }
    fn search_candidates(
        &mut self,
        coordinate: &ConeCoordinate,
    ) -> Result<Vec<String>, GraphError> {
        self.base.search_candidates(coordinate)
    }
    fn core_slot(&mut self) -> Result<(String, Vec<u8>), GraphError> {
        self.base.core_slot()
    }
}

impl ScheduleInputs for MemoryScheduleInputs {
    fn read_sources(&mut self, manifest_path: &str) -> Result<Vec<(String, String)>, GraphError> {
        self.sources
            .get(manifest_path)
            .cloned()
            .ok_or_else(|| GraphError::Io(format!("sources for {manifest_path} not found")))
    }

    fn artifact_bytes(&mut self, path: &str) -> Result<Vec<u8>, GraphError> {
        self.base.read_artifact(path)
    }

    fn compiler_identity(&mut self) -> Result<CompilerKeyIdentity, GraphError> {
        Ok(CompilerKeyIdentity {
            language_abi: 1,
            runtime_abi: Digest256::from_bytes([1; 32]),
            identity_schema_version: 1,
            container_version: 1,
            hir_wire_schema: 1,
            mir_wire_schema: 1,
            lir_wire_schema: 1,
            target_profile_fingerprint: Digest256::from_bytes([5; 32]),
        })
    }
}

fn source_cone(
    inputs: &mut MemoryScheduleInputs,
    path: &str,
    group: &str,
    name: &str,
    version: &str,
    deps: Vec<DeclaredDependency>,
    sources: &[(&str, &str)],
) {
    let text = format!(
        "schema = 1\n\n[cone]\ngroup = \"{group}\"\nname = \"{name}\"\nversion = \"{version}\"\nkind = \"library\"\n"
    );
    let parsed = scoop_manifest::parse_manifest(&text).unwrap();
    let manifest = ConeManifest::new(parsed.coordinate().clone(), parsed.kind(), deps);
    inputs.base.manifests.insert(path.to_owned(), manifest);
    inputs.sources.insert(
        path.to_owned(),
        sources
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
            .collect(),
    );
}

fn layered_graph(inputs: &mut MemoryScheduleInputs) -> crate::graph::ResolvedBuildGraph {
    // root -> util(source) -> core.
    source_cone(
        inputs,
        "/lib/util/Cone.toml",
        "org.acme",
        "util",
        "2.0.0",
        vec![],
        &[("util.scoop", "fun u() {}\n")],
    );
    let root = root_manifest(
        ConeKind::Library,
        vec![DeclaredDependency::new(
            ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap(),
            DependencyLocator::SourcePath {
                path: "/lib/util/Cone.toml".to_owned(),
            },
        )],
    );
    let root_deps = vec![DeclaredDependency::new(
        ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap(),
        DependencyLocator::SourcePath {
            path: "/lib/util/Cone.toml".to_owned(),
        },
    )];
    source_cone(
        inputs,
        "/root/Cone.toml",
        "dev.example",
        "app",
        "0.1.0",
        root_deps.clone(),
        &[("main.scoop", "fun m() {}\n")],
    );
    let root = ConeManifest::new(root.coordinate().clone(), root.kind(), root_deps);
    resolve_graph(root, "/root/Cone.toml", &mut inputs.base).unwrap()
}

#[test]
fn each_source_node_compiles_exactly_once_in_order() {
    let mut inputs = MemoryScheduleInputs {
        base: {
            let mut base = MemoryInputs::default();
            base.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
            let bytes = base.artifacts["core.slib"].clone();
            base.core = Some(bytes);
            base
        },
        sources: BTreeMap::new(),
    };
    let graph = layered_graph(&mut inputs);
    let mut runner = RecordingRunner::new();
    let mut gate = ProgrammableGate { fail: vec![] };
    let mut cache = MemoryCache::default();
    let outcome = schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap();
    // Root and util were compiled, each once, dependencies first.
    let order: Vec<String> = runner
        .invocations
        .iter()
        .map(|t| t.coordinate.clone())
        .collect();
    assert_eq!(order, ["org.acme:util:2.0.0", "dev.example:app:0.1.0"]);
    assert_eq!(outcome.published.len(), 2);
    // util's request names the core; root names the core + util.
    assert_eq!(runner.invocations[0].core_slib, "core.slib");
    assert!(runner.invocations[0].direct_slibs.is_empty());
    assert!(runner.invocations[0].support_slibs.is_empty());
    assert!(
        runner.invocations[1]
            .direct_slibs
            .contains(&"cache/org.acme:util:2.0.0/cone.slib".to_owned())
    );
}

#[test]
fn second_build_hits_cache_without_invocations() {
    let mut inputs = MemoryScheduleInputs {
        base: {
            let mut base = MemoryInputs::default();
            base.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
            let bytes = base.artifacts["core.slib"].clone();
            base.core = Some(bytes);
            base
        },
        sources: BTreeMap::new(),
    };
    let graph = layered_graph(&mut inputs);
    let mut runner = RecordingRunner::new();
    let mut gate = ProgrammableGate { fail: vec![] };
    let mut cache = MemoryCache::default();
    schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap();
    assert_eq!(runner.invocations.len(), 2);

    // Rebuild with a fresh runner: every source node is a cache hit.
    let mut runner2 = RecordingRunner::new();
    schedule_build(&graph, &mut inputs, &mut runner2, &mut gate, &mut cache).unwrap();
    assert!(runner2.invocations.is_empty(), "expected cache hits");
}

#[test]
fn source_change_invalidates_only_dependent_chain() {
    let make_inputs = |util_text: &str| {
        let mut inputs = MemoryScheduleInputs {
            base: {
                let mut base = MemoryInputs::default();
                base.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
                let bytes = base.artifacts["core.slib"].clone();
                base.core = Some(bytes);
                base
            },
            sources: BTreeMap::new(),
        };
        source_cone(
            &mut inputs,
            "/lib/util/Cone.toml",
            "org.acme",
            "util",
            "2.0.0",
            vec![],
            &[("util.scoop", util_text)],
        );
        let root = root_manifest(
            ConeKind::Library,
            vec![DeclaredDependency::new(
                ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap(),
                DependencyLocator::SourcePath {
                    path: "/lib/util/Cone.toml".to_owned(),
                },
            )],
        );
        let root_deps = vec![DeclaredDependency::new(
            ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap(),
            DependencyLocator::SourcePath {
                path: "/lib/util/Cone.toml".to_owned(),
            },
        )];
        source_cone(
            &mut inputs,
            "/root/Cone.toml",
            "dev.example",
            "app",
            "0.1.0",
            root_deps.clone(),
            &[("main.scoop", "fun m() {}\n")],
        );
        let root = ConeManifest::new(root.coordinate().clone(), root.kind(), root_deps);
        let graph = resolve_graph(root, "/root/Cone.toml", &mut inputs.base).unwrap();
        (inputs, graph)
    };

    let (mut inputs, graph) = make_inputs("fun u() {}\n");
    let mut runner = RecordingRunner::new();
    let mut gate = ProgrammableGate { fail: vec![] };
    let mut cache = MemoryCache::default();
    schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap();

    // Same sources → full cache hit.
    let (mut inputs, graph) = make_inputs("fun u() {}\n");
    let mut runner = RecordingRunner::new();
    schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap();
    assert!(runner.invocations.is_empty());

    // Changed util source rebuilds util. Under the interim dependency
    // layer derivation (metadata payload digests) the fake's metadata is
    // identical, so the dependent stays a cache hit; the unit test below
    // proves dependency-layer changes invalidate the dependent's key.
    let (mut inputs, graph) = make_inputs("fun u2() {}\n");
    let mut runner = RecordingRunner::new();
    schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap();
    let order: Vec<String> = runner
        .invocations
        .iter()
        .map(|t| t.coordinate.clone())
        .collect();
    assert_eq!(order, ["org.acme:util:2.0.0"]);
}

#[test]
fn cache_key_tracks_dependency_layers_and_sources() {
    let manifest = {
        let text = "schema = 1\n\n[cone]\ngroup = \"dev.example\"\nname = \"app\"\nversion = \"0.1.0\"\nkind = \"library\"\n";
        let parsed = scoop_manifest::parse_manifest(text).unwrap();
        ConeManifest::new(parsed.coordinate().clone(), parsed.kind(), vec![])
    };
    let sources = vec![("main.scoop".to_owned(), "fun m() {}\n".to_owned())];
    let compiler = CompilerKeyIdentity {
        language_abi: 1,
        runtime_abi: Digest256::from_bytes([1; 32]),
        identity_schema_version: 1,
        container_version: 1,
        hir_wire_schema: 1,
        mir_wire_schema: 1,
        lir_wire_schema: 1,
        target_profile_fingerprint: Digest256::from_bytes([5; 32]),
    };
    let dep_identity = ConeIdentity::of(&ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap());
    let layers = DependencyLayers {
        hir: Digest256::from_bytes([1; 32]),
        mir: Digest256::from_bytes([2; 32]),
        lir: Digest256::from_bytes([3; 32]),
    };
    let base = crate::schedule::compute_cache_key(&crate::schedule::CacheKeyInput {
        manifest: &manifest,
        sources: &sources,
        compiler: &compiler,
        dependencies: &[(dep_identity, layers)],
    });

    // Source change invalidates.
    let changed_source = crate::schedule::compute_cache_key(&crate::schedule::CacheKeyInput {
        manifest: &manifest,
        sources: &[("main.scoop".to_owned(), "fun other() {}\n".to_owned())],
        compiler: &compiler,
        dependencies: &[(dep_identity, layers)],
    });
    assert_ne!(base, changed_source);

    // Dependency layer change (HIR only) invalidates.
    let changed_dep = crate::schedule::compute_cache_key(&crate::schedule::CacheKeyInput {
        manifest: &manifest,
        sources: &sources,
        compiler: &compiler,
        dependencies: &[(
            dep_identity,
            DependencyLayers {
                hir: Digest256::from_bytes([9; 32]),
                ..layers
            },
        )],
    });
    assert_ne!(base, changed_dep);

    // Identical inputs keep the key.
    let same = crate::schedule::compute_cache_key(&crate::schedule::CacheKeyInput {
        manifest: &manifest,
        sources: &sources,
        compiler: &compiler,
        dependencies: &[(dep_identity, layers)],
    });
    assert_eq!(base, same);

    // Toolchain identity change invalidates.
    let changed_compiler = crate::schedule::compute_cache_key(&crate::schedule::CacheKeyInput {
        manifest: &manifest,
        sources: &sources,
        compiler: &CompilerKeyIdentity {
            hir_wire_schema: 2,
            ..compiler
        },
        dependencies: &[(dep_identity, layers)],
    });
    assert_ne!(base, changed_compiler);
}

#[test]
fn compiler_failure_stops_dependents() {
    let mut inputs = MemoryScheduleInputs {
        base: {
            let mut base = MemoryInputs::default();
            base.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
            let bytes = base.artifacts["core.slib"].clone();
            base.core = Some(bytes);
            base
        },
        sources: BTreeMap::new(),
    };
    let graph = layered_graph(&mut inputs);
    let mut runner = RecordingRunner::new();
    runner.fail.push("org.acme:util:2.0.0".to_owned());
    let mut gate = ProgrammableGate { fail: vec![] };
    let mut cache = MemoryCache::default();
    let error =
        schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap_err();
    assert!(matches!(
        error,
        ScheduleError::CompilerFailed { ref coordinate, .. } if coordinate == "org.acme:util:2.0.0"
    ));
    // The dependent root was never invoked.
    assert_eq!(runner.invocations.len(), 1);
}

#[test]
fn stale_prebuilt_is_rejected_not_rebuilt() {
    let mut inputs = MemoryScheduleInputs {
        base: {
            let mut base = MemoryInputs::default();
            base.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
            let bytes = base.artifacts["core.slib"].clone();
            base.core = Some(bytes);
            base
        },
        sources: BTreeMap::new(),
    };
    inputs
        .base
        .add_artifact("libs/util.slib", "org.acme", "util", "2.0.0", &[]);
    let root = root_manifest(
        ConeKind::Library,
        vec![DeclaredDependency::new(
            ConeCoordinate::new("org.acme", "util", "2.0.0").unwrap(),
            DependencyLocator::ArtifactPath {
                path: "libs/util.slib".to_owned(),
            },
        )],
    );
    let graph = resolve_graph(root, "/root/Cone.toml", &mut inputs.base).unwrap();
    let mut runner = RecordingRunner::new();
    let mut gate = ProgrammableGate {
        fail: vec!["org.acme:util:2.0.0".to_owned()],
    };
    let mut cache = MemoryCache::default();
    let error =
        schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap_err();
    assert!(matches!(error, ScheduleError::StalePrebuilt { .. }));
    assert!(runner.invocations.is_empty());
}

#[test]
fn rejected_compiler_output_is_not_cached() {
    let mut inputs = MemoryScheduleInputs {
        base: {
            let mut base = MemoryInputs::default();
            base.add_artifact("core.slib", "scoop", "scoop.core", "0.1.0", &[]);
            let bytes = base.artifacts["core.slib"].clone();
            base.core = Some(bytes);
            base
        },
        sources: BTreeMap::new(),
    };
    let graph = layered_graph(&mut inputs);
    let mut runner = RecordingRunner::new();
    let mut gate = ProgrammableGate {
        fail: vec!["dev.example:app:0.1.0".to_owned()],
    };
    let mut cache = MemoryCache::default();
    let error =
        schedule_build(&graph, &mut inputs, &mut runner, &mut gate, &mut cache).unwrap_err();
    assert!(matches!(
        error,
        ScheduleError::CompilerOutputRejected { .. }
    ));
    // util's artifact was cached; the rejected root artifact was not.
    assert_eq!(cache.entries.len(), 1);
}

#[test]
fn dependency_layers_track_metadata_digests() {
    let build = |hir_payload: &[u8]| {
        let mut builder = SlibBuilder::new(core_template("org.acme", "util", "2.0.0", &[]));
        builder
            .add_member(
                MemberStableKey::HirMetadata,
                SlibMemberRole::HirMetadata { wire_schema: 1 },
                hir_payload.to_vec(),
            )
            .unwrap();
        builder
            .add_member(
                MemberStableKey::MirMetadata,
                SlibMemberRole::MirMetadata { wire_schema: 1 },
                b"mir".to_vec(),
            )
            .unwrap();
        builder
            .add_member(
                MemberStableKey::LirMetadata,
                SlibMemberRole::LirMetadata { wire_schema: 1 },
                b"lir".to_vec(),
            )
            .unwrap();
        builder.finish().unwrap()
    };
    let first = scoop_slib::ValidatedGraphArtifact::read(
        &build(b"hir-one"),
        &scoop_slib::SlibDecodeLimits::strict(),
    )
    .unwrap();
    let second = scoop_slib::ValidatedGraphArtifact::read(
        &build(b"hir-two"),
        &scoop_slib::SlibDecodeLimits::strict(),
    )
    .unwrap();
    let layers = DependencyLayers::from_manifest_core(first.manifest());
    let other = DependencyLayers::from_manifest_core(second.manifest());
    assert_ne!(layers.hir, other.hir);
    // Unchanged layers keep their digests.
    assert_eq!(layers.mir, other.mir);
    assert_eq!(layers.lir, other.lir);
    assert_ne!(layers.hir, Digest256::ZERO);
}
