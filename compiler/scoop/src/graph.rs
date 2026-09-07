//! Resolved build graph: manifest/locator resolution, graph validation
//! and the canonical topological order (DESIGN section 1.4).
//!
//! This stage only reads manifests and bounded `.slib` graph summaries;
//! it grants no compile/link views and decides no cache hits. Scheduling
//! and caching layer on top of the produced graph.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::{ConeKind, ConeManifest, DeclaredDependency, DependencyLocator};
use scoop_slib::{SlibDecodeLimits, ValidatedGraphArtifact};

/// Build-resolution failure. Errors carry the manifest/artifact location
/// of the offending edge so callers can render manifest-relative
/// diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    Manifest(String),
    ArtifactRead {
        path: String,
        detail: String,
    },
    CoordinateMismatch {
        declared: Box<ConeCoordinate>,
        found: Box<ConeCoordinate>,
        at: String,
    },
    DuplicateIdentity {
        identity: ConeIdentity,
        first: String,
        second: String,
    },
    MultipleVersions {
        group: String,
        name: String,
        first: String,
        second: String,
    },
    ExecutableDependency {
        coordinate: Box<ConeCoordinate>,
        at: String,
    },
    AmbiguousArtifact {
        coordinate: Box<ConeCoordinate>,
        candidates: Vec<String>,
    },
    MissingDependency {
        declared: Box<ConeCoordinate>,
        at: String,
    },
    CoreSlot(String),
    Cycle {
        path: Vec<Box<ConeCoordinate>>,
    },
    RootKind(String),
    Io(String),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::Manifest(message) => write!(f, "manifest error: {message}"),
            GraphError::ArtifactRead { path, detail } => {
                write!(f, "cannot read artifact {path}: {detail}")
            }
            GraphError::CoordinateMismatch {
                declared,
                found,
                at,
            } => write!(
                f,
                "locator at {at} resolved to {found}, but the dependency declares {declared}"
            ),
            GraphError::DuplicateIdentity {
                identity,
                first,
                second,
            } => {
                write!(
                    f,
                    "Cone {identity} is provided by both {first} and {second}"
                )
            }
            GraphError::MultipleVersions {
                group,
                name,
                first,
                second,
            } => write!(
                f,
                "Cone {group}:{name} resolves to versions {first} and {second}"
            ),
            GraphError::ExecutableDependency { coordinate, at } => write!(
                f,
                "executable Cone {coordinate} cannot be a dependency (required at {at})"
            ),
            GraphError::AmbiguousArtifact {
                coordinate,
                candidates,
            } => write!(
                f,
                "search roots offer multiple distinct artifacts for {coordinate}: {}",
                candidates.join(", ")
            ),
            GraphError::MissingDependency { declared, at } => {
                write!(
                    f,
                    "dependency {declared} declared at {at} cannot be resolved"
                )
            }
            GraphError::CoreSlot(message) => write!(f, "trusted core slot: {message}"),
            GraphError::Cycle { path } => write!(
                f,
                "dependency cycle: {}",
                path.iter()
                    .map(|coordinate| coordinate.display())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
            GraphError::RootKind(message) => write!(f, "build root: {message}"),
            GraphError::Io(message) => write!(f, "IO error: {message}"),
        }
    }
}

impl std::error::Error for GraphError {}

/// The IO surface graph resolution needs. Implementations read manifests
/// and `.slib` bytes from disk; tests read from memory.
pub trait BuildInputs {
    /// Reads the manifest at a `path` locator.
    fn read_manifest(&mut self, path: &str) -> Result<ConeManifest, GraphError>;
    /// Reads whole artifact bytes at an `artifact` locator or search hit.
    fn read_artifact(&mut self, path: &str) -> Result<Vec<u8>, GraphError>;
    /// Lists search-root candidates for a coordinate, in root order:
    /// `<root>/<group>/<name>/<version>/cone.slib`.
    fn search_candidates(&mut self, coordinate: &ConeCoordinate)
    -> Result<Vec<String>, GraphError>;
    /// Reads the trusted sysroot core artifact, returning its path (for
    /// dependent artifact requests) and bytes.
    fn core_slot(&mut self) -> Result<(String, Vec<u8>), GraphError>;
}

/// Where a resolved node's content comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeOrigin {
    /// A source Cone rooted at this manifest path; it must be compiled.
    Source { manifest_path: String },
    /// A prebuilt `.slib`; reuse still requires full validation at
    /// scheduling time.
    Prebuilt { artifact_path: String },
    /// The trusted sysroot core.
    Core { artifact_path: String },
}

/// One resolved graph node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNode {
    pub coordinate: ConeCoordinate,
    pub identity: ConeIdentity,
    pub kind: ConeKind,
    /// Direct dependency identities (implicit core edge included for
    /// non-core nodes).
    pub dependencies: Vec<ConeIdentity>,
    pub origin: NodeOrigin,
}

/// The validated graph plus its canonical order.
#[derive(Debug, Clone)]
pub struct ResolvedBuildGraph {
    nodes: BTreeMap<ConeIdentity, ResolvedNode>,
    /// Canonical topological order: dependencies first; each round picks
    /// the coordinate-byte-order minimum among ready nodes.
    order: Vec<ConeIdentity>,
    root: ConeIdentity,
}

impl ResolvedBuildGraph {
    pub fn root(&self) -> ConeIdentity {
        self.root
    }

    pub fn order(&self) -> &[ConeIdentity] {
        &self.order
    }

    pub fn node(&self, identity: &ConeIdentity) -> Option<&ResolvedNode> {
        self.nodes.get(identity)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ResolvedNode> {
        self.nodes.values()
    }
}

/// Resolves the build graph starting from the root Cone's manifest.
pub fn resolve_graph(
    root_manifest: ConeManifest,
    root_manifest_path: &str,
    inputs: &mut dyn BuildInputs,
) -> Result<ResolvedBuildGraph, GraphError> {
    let mut resolver = Resolver {
        inputs,
        nodes: BTreeMap::new(),
        versions: BTreeMap::new(),
    };

    let root_identity = ConeIdentity::of(root_manifest.coordinate());
    let root_kind = root_manifest.kind();
    resolver.add_source_node(root_manifest, root_manifest_path)?;

    // Root kind rules: an executable root is the graph's only
    // executable; a library root excludes executables entirely.
    let executable_count = resolver
        .nodes
        .values()
        .filter(|node| node.kind == ConeKind::Executable)
        .count();
    match (root_kind, executable_count) {
        (ConeKind::Executable, 1) => {}
        (ConeKind::Executable, _) => {
            return Err(GraphError::RootKind(
                "an executable build root must be the only executable in the graph".to_owned(),
            ));
        }
        (ConeKind::Library, 0) => {}
        (ConeKind::Library, _) => {
            return Err(GraphError::RootKind(
                "a library build root cannot depend on an executable Cone".to_owned(),
            ));
        }
    }

    let order = resolver.canonical_topological_order()?;
    Ok(ResolvedBuildGraph {
        nodes: resolver.nodes,
        order,
        root: root_identity,
    })
}

struct Resolver<'a> {
    inputs: &'a mut dyn BuildInputs,
    nodes: BTreeMap<ConeIdentity, ResolvedNode>,
    /// `(group, name)` → version, for the one-version rule.
    versions: BTreeMap<(String, String), String>,
}

impl Resolver<'_> {
    fn add_source_node(
        &mut self,
        manifest: ConeManifest,
        manifest_path: &str,
    ) -> Result<ResolvedNode, GraphError> {
        let coordinate = manifest.coordinate().clone();
        let identity = ConeIdentity::of(&coordinate);
        self.check_new_identity(&coordinate, &identity, manifest_path)?;
        let mut dependencies: Vec<ConeIdentity> = Vec::new();
        for dependency in manifest.dependencies() {
            let resolved = self.resolve_dependency(dependency, manifest_path)?;
            if !dependencies.contains(&resolved) {
                dependencies.push(resolved);
            }
        }
        // Implicit core direct edge for every non-core Cone.
        if !coordinate.is_reserved_core() {
            let core_identity = self.resolve_core()?;
            if !dependencies.contains(&core_identity) {
                dependencies.push(core_identity);
            }
        }
        let node = ResolvedNode {
            kind: manifest.kind(),
            coordinate: coordinate.clone(),
            identity,
            dependencies,
            origin: NodeOrigin::Source {
                manifest_path: manifest_path.to_owned(),
            },
        };
        self.nodes.insert(node.identity, node.clone());
        Ok(node)
    }

    fn add_prebuilt_node(
        &mut self,
        artifact: ValidatedGraphArtifact,
        artifact_path: &str,
        origin: NodeOrigin,
    ) -> Result<ResolvedNode, GraphError> {
        let coordinate = artifact.coordinate().clone();
        let identity = ConeIdentity::of(&coordinate);
        if !coordinate.is_reserved_core() {
            self.check_new_identity(&coordinate, &identity, artifact_path)?;
        } else if origin_is_core(&origin) {
            // First registration of the core slot: still enforce the
            // one-version rule but skip the duplicate check against
            // itself.
            self.check_new_identity(&coordinate, &identity, artifact_path)
                .or_else(|error| match error {
                    GraphError::DuplicateIdentity { .. } => Ok(()),
                    other => Err(other),
                })?;
        }
        if artifact.kind() == ConeKind::Executable && !coordinate.is_reserved_core() {
            return Err(GraphError::ExecutableDependency {
                coordinate: Box::new(coordinate),
                at: artifact_path.to_owned(),
            });
        }
        let mut dependencies: Vec<ConeIdentity> = Vec::new();
        let mut records: Vec<(ConeCoordinate, ConeIdentity)> = Vec::new();
        for record in artifact.dependencies() {
            dependencies.push(record.cone_identity);
            records.push((record.coordinate.clone(), record.cone_identity));
        }
        if !coordinate.is_reserved_core() {
            let core_identity = self.resolve_core()?;
            if !dependencies.contains(&core_identity) {
                dependencies.push(core_identity);
            }
        }
        // Resolve the prebuilt node's transitive dependencies through
        // this graph's inputs (search roots), using the coordinates the
        // dependency records carry.
        for (dependency_coordinate, dependency_identity) in records {
            if !self.nodes.contains_key(&dependency_identity) {
                let dependency =
                    DeclaredDependency::new(dependency_coordinate, DependencyLocator::Search);
                self.resolve_dependency(&dependency, artifact_path)?;
            }
        }
        let node = ResolvedNode {
            kind: artifact.kind(),
            coordinate,
            identity,
            dependencies,
            origin,
        };
        self.nodes.insert(node.identity, node.clone());
        Ok(node)
    }

    fn check_new_identity(
        &mut self,
        coordinate: &ConeCoordinate,
        identity: &ConeIdentity,
        at: &str,
    ) -> Result<(), GraphError> {
        if let Some(existing) = self.nodes.get(identity) {
            let first = match &existing.origin {
                NodeOrigin::Source { manifest_path } => manifest_path.clone(),
                NodeOrigin::Prebuilt { artifact_path } | NodeOrigin::Core { artifact_path } => {
                    artifact_path.clone()
                }
            };
            return Err(GraphError::DuplicateIdentity {
                identity: *identity,
                first,
                second: at.to_owned(),
            });
        }
        let key = (coordinate.group().to_owned(), coordinate.name().to_owned());
        match self.versions.entry(key) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(coordinate.version().as_str().to_owned());
            }
            std::collections::btree_map::Entry::Occupied(slot) => {
                if slot.get() != coordinate.version().as_str() {
                    return Err(GraphError::MultipleVersions {
                        group: coordinate.group().to_owned(),
                        name: coordinate.name().to_owned(),
                        first: slot.get().clone(),
                        second: coordinate.version().as_str().to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    fn resolve_dependency(
        &mut self,
        dependency: &DeclaredDependency,
        at: &str,
    ) -> Result<ConeIdentity, GraphError> {
        let coordinate = dependency.coordinate();
        let identity = ConeIdentity::of(coordinate);
        if let Some(existing) = self.nodes.get(&identity) {
            return Ok(existing.identity);
        }
        match dependency.locator() {
            DependencyLocator::SourcePath { path } => {
                let manifest = self.inputs.read_manifest(path)?;
                if manifest.coordinate() != coordinate {
                    return Err(GraphError::CoordinateMismatch {
                        declared: Box::new(coordinate.clone()),
                        found: Box::new(manifest.coordinate().clone()),
                        at: at.to_owned(),
                    });
                }
                if manifest.kind() == ConeKind::Executable {
                    return Err(GraphError::ExecutableDependency {
                        coordinate: Box::new(coordinate.clone()),
                        at: at.to_owned(),
                    });
                }
                let node = self.add_source_node(manifest, path)?;
                Ok(node.identity)
            }
            DependencyLocator::ArtifactPath { path } => {
                let bytes = self.inputs.read_artifact(path)?;
                let artifact = open_graph_view(&bytes, path)?;
                if artifact.coordinate() != coordinate {
                    return Err(GraphError::CoordinateMismatch {
                        declared: Box::new(coordinate.clone()),
                        found: Box::new(artifact.coordinate().clone()),
                        at: at.to_owned(),
                    });
                }
                if artifact.kind() == ConeKind::Executable {
                    return Err(GraphError::ExecutableDependency {
                        coordinate: Box::new(coordinate.clone()),
                        at: at.to_owned(),
                    });
                }
                let node = self.add_prebuilt_node(
                    artifact,
                    path,
                    NodeOrigin::Prebuilt {
                        artifact_path: path.to_owned(),
                    },
                )?;
                Ok(node.identity)
            }
            DependencyLocator::Search => {
                let candidates = self.inputs.search_candidates(coordinate)?;
                let mut first: Option<(String, Vec<u8>)> = None;
                let mut fingerprints: BTreeSet<String> = BTreeSet::new();
                for candidate in &candidates {
                    let bytes = self.inputs.read_artifact(candidate)?;
                    let artifact = open_graph_view(&bytes, candidate)?;
                    if artifact.coordinate() != coordinate {
                        return Err(GraphError::CoordinateMismatch {
                            declared: Box::new(coordinate.clone()),
                            found: Box::new(artifact.coordinate().clone()),
                            at: candidate.clone(),
                        });
                    }
                    if artifact.kind() == ConeKind::Executable {
                        return Err(GraphError::ExecutableDependency {
                            coordinate: Box::new(coordinate.clone()),
                            at: candidate.clone(),
                        });
                    }
                    fingerprints.insert(artifact.artifact_fingerprint().as_digest().to_hex());
                    if first.is_none() {
                        first = Some((candidate.clone(), bytes));
                    }
                }
                let Some((path, bytes)) = first else {
                    return Err(GraphError::MissingDependency {
                        declared: Box::new(coordinate.clone()),
                        at: at.to_owned(),
                    });
                };
                if fingerprints.len() > 1 {
                    return Err(GraphError::AmbiguousArtifact {
                        coordinate: Box::new(coordinate.clone()),
                        candidates,
                    });
                }
                let artifact = open_graph_view(&bytes, &path)?;
                let node = self.add_prebuilt_node(
                    artifact,
                    &path.clone(),
                    NodeOrigin::Prebuilt {
                        artifact_path: path,
                    },
                )?;
                Ok(node.identity)
            }
        }
    }

    fn resolve_core(&mut self) -> Result<ConeIdentity, GraphError> {
        let core_coordinate = ConeCoordinate::reserved_core();
        let core_identity = ConeIdentity::of(&core_coordinate);
        if let Some(existing) = self.nodes.get(&core_identity) {
            return Ok(existing.identity);
        }
        let (core_path, bytes) = self
            .inputs
            .core_slot()
            .map_err(|error| GraphError::CoreSlot(error.to_string()))?;
        let artifact = open_graph_view(&bytes, &core_path)
            .map_err(|error| GraphError::CoreSlot(format!("invalid core artifact: {error}")))?;
        if !artifact.coordinate().is_reserved_core() {
            return Err(GraphError::CoreSlot(format!(
                "core slot carries {}",
                artifact.coordinate().display()
            )));
        }
        let node = self.add_prebuilt_node(
            artifact,
            &core_path.clone(),
            NodeOrigin::Core {
                artifact_path: core_path,
            },
        )?;
        Ok(node.identity)
    }

    /// Canonical topological order: repeatedly emit the coordinate-min
    /// node whose direct dependencies were all emitted.
    fn canonical_topological_order(&self) -> Result<Vec<ConeIdentity>, GraphError> {
        // Cycles are diagnosed with a full coordinate path first.
        self.detect_cycles()?;
        let mut emitted: BTreeSet<ConeIdentity> = BTreeSet::new();
        let mut order = Vec::with_capacity(self.nodes.len());
        while emitted.len() < self.nodes.len() {
            let ready = self
                .nodes
                .values()
                .filter(|node| !emitted.contains(&node.identity))
                .filter(|node| node.dependencies.iter().all(|dep| emitted.contains(dep)))
                .collect::<Vec<_>>();
            let Some(next) = ready
                .iter()
                .min_by(|left, right| left.coordinate.cmp(&right.coordinate))
                .copied()
            else {
                // Unreachable after cycle detection; a defensive cycle
                // error is the only consistent outcome.
                return Err(GraphError::Cycle { path: Vec::new() });
            };
            emitted.insert(next.identity);
            order.push(next.identity);
        }
        Ok(order)
    }

    fn detect_cycles(&self) -> Result<(), GraphError> {
        #[derive(Clone, Copy, PartialEq)]
        enum Color {
            White,
            Gray,
            Black,
        }
        let identities: Vec<ConeIdentity> = self.nodes.keys().copied().collect();
        let mut colors: BTreeMap<ConeIdentity, Color> = identities
            .iter()
            .map(|identity| (*identity, Color::White))
            .collect();
        for start in &identities {
            if colors[start] != Color::White {
                continue;
            }
            let mut stack: Vec<(ConeIdentity, usize)> = vec![(*start, 0)];
            let mut path: Vec<ConeIdentity> = vec![*start];
            colors.insert(*start, Color::Gray);
            while let Some((identity, next_edge)) = stack.last_mut() {
                let dependencies = &self.nodes[identity].dependencies;
                if *next_edge >= dependencies.len() {
                    colors.insert(*identity, Color::Black);
                    path.pop();
                    stack.pop();
                    continue;
                }
                let target = dependencies[*next_edge];
                *next_edge += 1;
                match colors[&target] {
                    Color::White => {
                        colors.insert(target, Color::Gray);
                        path.push(target);
                        stack.push((target, 0));
                    }
                    Color::Gray => {
                        let cycle_start = path.iter().position(|node| *node == target).unwrap_or(0);
                        let path_coordinates: Vec<Box<ConeCoordinate>> = path[cycle_start..]
                            .iter()
                            .map(|identity| self.nodes[identity].coordinate.clone())
                            .map(Box::new)
                            .collect();
                        return Err(GraphError::Cycle {
                            path: path_coordinates,
                        });
                    }
                    Color::Black => {}
                }
            }
        }
        Ok(())
    }
}

fn origin_is_core(origin: &NodeOrigin) -> bool {
    matches!(origin, NodeOrigin::Core { .. })
}

fn open_graph_view(bytes: &[u8], path: &str) -> Result<ValidatedGraphArtifact, GraphError> {
    ValidatedGraphArtifact::read(bytes, &SlibDecodeLimits::default()).map_err(|error| {
        GraphError::ArtifactRead {
            path: path.to_owned(),
            detail: error.to_string(),
        }
    })
}
