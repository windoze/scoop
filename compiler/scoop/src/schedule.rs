//! Build scheduling and caching over the resolved graph (DESIGN
//! sections 1.4 and 5.5).
//!
//! Nodes are processed strictly in the graph's canonical order. A source
//! node either reuses a cache entry or invokes the single-Cone compiler
//! exactly once; every artifact — prebuilt, cached or freshly built —
//! must pass the reuse gate (the production gate validates both the
//! Compile and Link views) before dependents may consume it. A failed
//! node stops its dependents and the whole build.

use core::fmt;
use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity, Digest256, DomainHasher};
use scoop_manifest::ConeManifest;
use scoop_slib::ManifestCore;

use crate::graph::{BuildInputs, GraphError, NodeOrigin, ResolvedBuildGraph};

pub const CACHE_KEY_DOMAIN: &[u8] = b"scoop-cone-cache-key-v1";

/// Diagnostic crossing the scheduler boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerDiagnostic {
    pub message: String,
}

impl fmt::Display for SchedulerDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SchedulerDiagnostic {}

/// Scheduling failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    Graph(GraphError),
    /// A prebuilt artifact failed the reuse gate; the design reports
    /// stale dependencies rather than rebuilding other Cones' inputs.
    StalePrebuilt {
        coordinate: String,
        detail: String,
    },
    CompilerFailed {
        coordinate: String,
        diagnostics: Vec<SchedulerDiagnostic>,
    },
    CompilerOutputRejected {
        coordinate: String,
        detail: String,
    },
    Aborted {
        failed: String,
    },
    Io(String),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleError::Graph(error) => write!(f, "{error}"),
            ScheduleError::StalePrebuilt { coordinate, detail } => write!(
                f,
                "prebuilt artifact for {coordinate} is not reusable: {detail}"
            ),
            ScheduleError::CompilerFailed {
                coordinate,
                diagnostics,
            } => {
                write!(f, "compiling {coordinate} failed:")?;
                for diagnostic in diagnostics {
                    write!(f, "\n  {diagnostic}")?;
                }
                Ok(())
            }
            ScheduleError::CompilerOutputRejected { coordinate, detail } => {
                write!(f, "compiler output for {coordinate} was rejected: {detail}")
            }
            ScheduleError::Aborted { failed } => {
                write!(f, "build aborted because {failed} failed")
            }
            ScheduleError::Io(message) => write!(f, "IO error: {message}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

impl From<GraphError> for ScheduleError {
    fn from(error: GraphError) -> Self {
        ScheduleError::Graph(error)
    }
}

/// The single-Cone compiler boundary. The production implementation
/// spawns `scoopc`; tests record invocations.
pub trait ScoopcRunner {
    /// Compiles the Cone described by `request`; on success returns the
    /// published artifact bytes.
    fn build(&mut self, request: &BuildTask) -> Result<Vec<u8>, Vec<SchedulerDiagnostic>>;
}

/// Reuse gate: an artifact may back a graph node only after full
/// validation. The production gate constructs both the Compile and Link
/// views from the same envelope; tests simulate failures.
pub trait ReuseGate {
    fn validate(&mut self, bytes: &[u8], expected: &ConeCoordinate) -> Result<(), String>;
}

/// Content-addressed artifact cache.
pub trait ArtifactCache {
    fn lookup(&mut self, key: &Digest256) -> Option<Vec<u8>>;
    fn store(&mut self, key: &Digest256, bytes: &[u8]);
}

/// One compilation task handed to the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTask {
    pub coordinate: String,
    pub manifest_path: String,
    pub core_slib: String,
    pub direct_slibs: Vec<String>,
    pub support_slibs: Vec<String>,
    pub out_slib: String,
}

/// Inputs the scheduler needs beyond graph resolution: sources for cache
/// keys and prebuilt bytes by artifact path.
pub trait ScheduleInputs: BuildInputs {
    /// Sorted `(relative path, text)` of one source Cone.
    fn read_sources(&mut self, manifest_path: &str) -> Result<Vec<(String, String)>, GraphError>;
    /// Whole bytes of a prebuilt/core artifact by path.
    fn artifact_bytes(&mut self, path: &str) -> Result<Vec<u8>, GraphError>;
    /// Toolchain identity fields entering the cache key.
    fn compiler_identity(&mut self) -> Result<CompilerKeyIdentity, GraphError>;
}

/// Cache-key portion of the toolchain identity: language/schema/runtime
/// ABI and target, matched exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerKeyIdentity {
    pub language_abi: u32,
    pub runtime_abi: Digest256,
    pub identity_schema_version: u32,
    pub container_version: u32,
    pub hir_wire_schema: u32,
    pub mir_wire_schema: u32,
    pub lir_wire_schema: u32,
    pub target_profile_fingerprint: Digest256,
}

/// The cache key of one Cone build (DESIGN 1.4): manifest semantic
/// fields, all source digests, toolchain identity, target fingerprint,
/// and the conservative baseline — every direct dependency's three-layer
/// semantic fingerprints.
pub struct CacheKeyInput<'a> {
    pub manifest: &'a ConeManifest,
    pub sources: &'a [(String, String)],
    pub compiler: &'a CompilerKeyIdentity,
    /// Direct dependency identities and their three-layer fingerprints.
    pub dependencies: &'a [(ConeIdentity, DependencyLayers)],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencyLayers {
    pub hir: Digest256,
    pub mir: Digest256,
    pub lir: Digest256,
}

impl DependencyLayers {
    /// Interim derivation used until M23's Merkle semantic fingerprints
    /// land: the digests of the artifact's three metadata member
    /// payloads. Any metadata change rebuilds dependents, which is the
    /// conservative direction; the switch to Merkle fingerprints in the
    /// fingerprint task only turns over cache entries.
    pub fn from_manifest_core(core: &ManifestCore) -> Self {
        let mut layers = DependencyLayers {
            hir: Digest256::ZERO,
            mir: Digest256::ZERO,
            lir: Digest256::ZERO,
        };
        use scoop_slib::MemberStableKey;
        for member in &core.members {
            let digest = member.sha256;
            match member.stable_key {
                MemberStableKey::HirMetadata => layers.hir = digest,
                MemberStableKey::MirMetadata => layers.mir = digest,
                MemberStableKey::LirMetadata => layers.lir = digest,
                _ => {}
            }
        }
        layers
    }
}

pub fn compute_cache_key(input: &CacheKeyInput<'_>) -> Digest256 {
    use scoop_identity::CborWriter;
    let mut hasher = DomainHasher::new(CACHE_KEY_DOMAIN);

    // Manifest semantic projection: coordinate, kind, dependency
    // coordinates in declaration order.
    let mut writer = CborWriter::new();
    writer.map(4);
    writer.field(1).text(input.manifest.coordinate().group());
    writer.field(2).text(input.manifest.coordinate().name());
    writer
        .field(3)
        .text(input.manifest.coordinate().version().as_str());
    writer.field(4).unsigned(match input.manifest.kind() {
        scoop_manifest::ConeKind::Library => 1,
        scoop_manifest::ConeKind::Executable => 2,
    });
    writer.field(5);
    writer.array(input.manifest.dependencies().len() as u64);
    for dependency in input.manifest.dependencies() {
        writer.array(3);
        writer.text(dependency.coordinate().group());
        writer.text(dependency.coordinate().name());
        writer.text(dependency.coordinate().version().as_str());
    }
    hasher = hasher.field(&writer.into_bytes());

    // Source digests in sorted path order.
    let mut writer = CborWriter::new();
    writer.array(input.sources.len() as u64);
    for (path, text) in input.sources {
        let digest = {
            use sha2::Digest as _;
            let mut raw = [0u8; 32];
            raw.copy_from_slice(&sha2::Sha256::digest(text.as_bytes()));
            Digest256::from_bytes(raw)
        };
        writer.array(2);
        writer.text(path);
        writer.bytes(digest.as_bytes());
    }
    hasher = hasher.field(&writer.into_bytes());

    // Toolchain identity.
    let mut writer = CborWriter::new();
    writer.map(8);
    writer.field(1).unsigned(input.compiler.language_abi as u64);
    writer.field(2).bytes(input.compiler.runtime_abi.as_bytes());
    writer
        .field(3)
        .unsigned(input.compiler.identity_schema_version as u64);
    writer
        .field(4)
        .unsigned(input.compiler.container_version as u64);
    writer
        .field(5)
        .unsigned(input.compiler.hir_wire_schema as u64);
    writer
        .field(6)
        .unsigned(input.compiler.mir_wire_schema as u64);
    writer
        .field(7)
        .unsigned(input.compiler.lir_wire_schema as u64);
    writer
        .field(8)
        .bytes(input.compiler.target_profile_fingerprint.as_bytes());
    hasher = hasher.field(&writer.into_bytes());

    // Conservative baseline: all direct dependencies, all three layers.
    let mut writer = CborWriter::new();
    writer.array(input.dependencies.len() as u64);
    for (identity, layers) in input.dependencies {
        writer.array(4);
        writer.bytes(identity.as_bytes());
        writer.bytes(layers.hir.as_bytes());
        writer.bytes(layers.mir.as_bytes());
        writer.bytes(layers.lir.as_bytes());
    }
    hasher = hasher.field(&writer.into_bytes());

    hasher.finish()
}

/// Outcome of one scheduled build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleOutcome {
    /// Identity → published artifact bytes, in completion order.
    pub published: Vec<(ConeIdentity, Vec<u8>)>,
}

/// Runs the build: for every node in canonical order, obtain validated
/// artifact bytes from the core slot, prebuilt inputs, the cache, or
/// exactly one compiler invocation.
pub fn schedule_build(
    graph: &ResolvedBuildGraph,
    inputs: &mut dyn ScheduleInputs,
    runner: &mut dyn ScoopcRunner,
    gate: &mut dyn ReuseGate,
    cache: &mut dyn ArtifactCache,
) -> Result<ScheduleOutcome, ScheduleError> {
    // Artifacts ready for dependents, keyed by identity.
    let mut ready: BTreeMap<ConeIdentity, Vec<u8>> = BTreeMap::new();
    // Identity → path used when passing artifacts to the compiler.
    let mut paths: BTreeMap<ConeIdentity, String> = BTreeMap::new();
    let mut published = Vec::new();

    for identity in graph.order() {
        let node = graph.node(identity).expect("order covers nodes");
        let bytes = match &node.origin {
            NodeOrigin::Core { artifact_path } => {
                let bytes = inputs
                    .artifact_bytes(artifact_path)
                    .map_err(ScheduleError::Graph)?;
                gate.validate(&bytes, &node.coordinate).map_err(|detail| {
                    ScheduleError::StalePrebuilt {
                        coordinate: node.coordinate.display(),
                        detail,
                    }
                })?;
                bytes
            }
            NodeOrigin::Prebuilt { artifact_path } => {
                let bytes = inputs
                    .artifact_bytes(artifact_path)
                    .map_err(ScheduleError::Graph)?;
                gate.validate(&bytes, &node.coordinate).map_err(|detail| {
                    ScheduleError::StalePrebuilt {
                        coordinate: node.coordinate.display(),
                        detail,
                    }
                })?;
                bytes
            }
            NodeOrigin::Source { manifest_path } => {
                let manifest = inputs
                    .read_manifest(manifest_path)
                    .map_err(ScheduleError::Graph)?;
                let sources = inputs
                    .read_sources(manifest_path)
                    .map_err(ScheduleError::Graph)?;
                let compiler = inputs.compiler_identity().map_err(ScheduleError::Graph)?;
                let mut dependencies = Vec::new();
                for dependency_identity in &node.dependencies {
                    let bytes =
                        ready
                            .get(dependency_identity)
                            .ok_or_else(|| ScheduleError::Aborted {
                                failed: dependency_identity.to_string(),
                            })?;
                    let view = scoop_slib::ValidatedGraphArtifact::read(
                        bytes,
                        &scoop_slib::SlibDecodeLimits::default(),
                    )
                    .map_err(|error| {
                        ScheduleError::CompilerOutputRejected {
                            coordinate: node.coordinate.display(),
                            detail: error.to_string(),
                        }
                    })?;
                    dependencies.push((
                        *dependency_identity,
                        DependencyLayers::from_manifest_core(view.manifest()),
                    ));
                }
                let key = compute_cache_key(&CacheKeyInput {
                    manifest: &manifest,
                    sources: &sources,
                    compiler: &compiler,
                    dependencies: &dependencies,
                });
                let cache_path = format!("cache/{}/cone.slib", node.coordinate.display());
                if let Some(cached) = cache.lookup(&key) {
                    gate.validate(&cached, &node.coordinate).map_err(|detail| {
                        ScheduleError::CompilerOutputRejected {
                            coordinate: node.coordinate.display(),
                            detail: format!("cached artifact: {detail}"),
                        }
                    })?;
                    paths.insert(*identity, cache_path);
                    ready.insert(*identity, cached);
                    continue;
                }
                let core_path = paths[&ConeIdentity::of(&ConeCoordinate::reserved_core())].clone();
                let direct: Vec<String> = manifest
                    .dependencies()
                    .iter()
                    .map(|dependency| paths[&ConeIdentity::of(dependency.coordinate())].clone())
                    .collect();
                let direct_ids: std::collections::BTreeSet<ConeIdentity> = manifest
                    .dependencies()
                    .iter()
                    .map(|dependency| ConeIdentity::of(dependency.coordinate()))
                    .collect();
                let support: Vec<String> = graph
                    .order()
                    .iter()
                    .filter(|other| **other != *identity)
                    .filter(|other| !direct_ids.contains(*other))
                    .filter(|other| **other != ConeIdentity::of(&ConeCoordinate::reserved_core()))
                    .filter(|other| node.dependencies.contains(other))
                    .filter(|other| paths.contains_key(*other))
                    .map(|other| paths[other].clone())
                    .collect();
                let task = BuildTask {
                    coordinate: node.coordinate.display(),
                    manifest_path: manifest_path.to_owned(),
                    core_slib: core_path,
                    direct_slibs: direct,
                    support_slibs: support,
                    out_slib: format!("out/{}/cone.slib", node.coordinate.display()),
                };
                let bytes =
                    runner
                        .build(&task)
                        .map_err(|diagnostics| ScheduleError::CompilerFailed {
                            coordinate: node.coordinate.display(),
                            diagnostics,
                        })?;
                // Parent-side revalidation before publishing.
                let view = scoop_slib::ValidatedGraphArtifact::read(
                    &bytes,
                    &scoop_slib::SlibDecodeLimits::default(),
                )
                .map_err(|error| ScheduleError::CompilerOutputRejected {
                    coordinate: node.coordinate.display(),
                    detail: error.to_string(),
                })?;
                if view.coordinate() != &node.coordinate {
                    return Err(ScheduleError::CompilerOutputRejected {
                        coordinate: node.coordinate.display(),
                        detail: format!("output carries {}", view.coordinate().display()),
                    });
                }
                gate.validate(&bytes, &node.coordinate).map_err(|detail| {
                    ScheduleError::CompilerOutputRejected {
                        coordinate: node.coordinate.display(),
                        detail,
                    }
                })?;
                cache.store(&key, &bytes);
                paths.insert(*identity, cache_path);
                ready.insert(*identity, bytes.clone());
                published.push((*identity, bytes));
                continue;
            }
        };
        // Core/prebuilt publication: record path and bytes for
        // dependents; the compiler consumes their existing paths.
        let path = match &node.origin {
            NodeOrigin::Core { artifact_path } | NodeOrigin::Prebuilt { artifact_path } => {
                artifact_path.clone()
            }
            NodeOrigin::Source { .. } => unreachable!("handled above"),
        };
        paths.insert(*identity, path);
        ready.insert(*identity, bytes);
    }
    Ok(ScheduleOutcome { published })
}
