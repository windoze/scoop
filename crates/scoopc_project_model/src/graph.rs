//! Stage-independent source cone identity, graph data, and topology checks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miette::{Result, miette};
use scoopc_ids::{StableCanonicalKey, canonical_record};
use scoopc_source::SourceFile;

use crate::manifest::{ConeKind, ConeManifest, ConeNativeBuildConfig};

/// Cone（编译包/分发单元）标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConeId(u32);

impl ConeId {
    pub const DEFAULT: ConeId = ConeId(0);

    /// Construct a cone id from the raw per-project integer value.
    pub const fn new(raw: u32) -> ConeId {
        ConeId(raw)
    }

    /// Return the raw per-project integer value for diagnostics and stable sorting.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Resolver-visible cone metadata attached to every indexed source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConeInfo {
    pub id: ConeId,
    pub kind: ConeKind,
}

impl ConeInfo {
    pub const DEFAULT: ConeInfo = ConeInfo {
        id: ConeId::DEFAULT,
        kind: ConeKind::Bin,
    };
}

/// Semantic cone identity derived from `Cone.toml` instead of `ConeId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StableConeKey {
    name: String,
    version: String,
}

impl StableConeKey {
    /// Construct a semantic cone key from manifest name/version fields.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    /// Construct a semantic cone key from a parsed manifest.
    pub fn from_manifest(manifest: &ConeManifest) -> Self {
        Self::new(&manifest.cone.name, &manifest.cone.version)
    }

    /// Construct the synthetic key used for virtual single-file inputs.
    pub fn for_virtual_source_path(path: &Path) -> Self {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .unwrap_or("virtual-cone");
        Self::new(name, "0.0.0")
    }

    /// Return the manifest name component.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the manifest version component.
    pub fn version(&self) -> &str {
        &self.version
    }
}

impl StableCanonicalKey for StableConeKey {
    fn canonical_text(&self) -> String {
        canonical_record("cone", [self.name.clone(), self.version.clone()])
    }
}

/// The synthetic consumer cone id used for virtual or explicitly requested roots.
pub const CONSUMER_CONE_ID: ConeId = ConeId::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConeRole {
    SysrootAuto,
    LocalDependency,
    Consumer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConeTrust {
    Untrusted,
    TrustedSyslib,
}

impl SourceConeTrust {
    /// Whether sources in this cone should be treated as trusted syslib inputs.
    pub fn is_trusted_syslib(self) -> bool {
        self == Self::TrustedSyslib
    }
}

/// Authoritative cone metadata for a source file after the source cone graph is flattened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConeInfo {
    pub id: ConeId,
    pub kind: ConeKind,
    pub stable_key: StableConeKey,
    pub trust: SourceConeTrust,
}

impl SourceConeInfo {
    /// Build source metadata from the owning graph node.
    pub fn from_node(node: &SourceConeNode) -> Self {
        Self {
            id: node.id,
            kind: node.kind,
            stable_key: StableConeKey::from_manifest(&node.manifest),
            trust: node.trust,
        }
    }

    /// Return the resolver-facing cone metadata for this source.
    pub fn resolver_info(&self) -> ConeInfo {
        ConeInfo {
            id: self.id,
            kind: self.kind,
        }
    }

    /// Whether the owning cone is a trusted syslib cone.
    pub fn is_trusted_syslib(&self) -> bool {
        self.trust.is_trusted_syslib()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConeDependencyKind {
    SysrootAuto,
    LocalSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceConeDependencyEdge {
    pub target: ConeId,
    pub kind: SourceConeDependencyKind,
}

#[derive(Debug, Clone)]
pub struct SourceConeNode {
    pub id: ConeId,
    pub role: SourceConeRole,
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: ConeManifest,
    pub kind: ConeKind,
    pub native_build: ConeNativeBuildConfig,
    pub trust: SourceConeTrust,
    pub sources: Vec<SourceFile>,
    pub entry_main: Option<PathBuf>,
    pub dependencies: Vec<SourceConeDependencyEdge>,
}

#[derive(Debug, Clone)]
pub struct SourceConeGraph {
    nodes: Vec<SourceConeNode>,
    consumer: ConeId,
}

impl SourceConeGraph {
    /// Validate graph membership and return nodes in dependency-before-consumer topo order.
    pub fn from_nodes(mut nodes: Vec<SourceConeNode>, consumer: ConeId) -> Result<Self> {
        if nodes.is_empty() {
            return Err(miette!("source cone graph 至少需要一个 node"));
        }

        for node in &mut nodes {
            if node.sources.is_empty() {
                return Err(miette!(
                    "source cone graph node `{}` 没有 sources",
                    node.manifest.cone.name
                ));
            }
            node.dependencies.sort_by_key(|edge| edge.target);
            node.dependencies.dedup_by_key(|edge| edge.target);
        }

        let mut by_id = BTreeMap::new();
        for (idx, node) in nodes.iter().enumerate() {
            if by_id.insert(node.id, idx).is_some() {
                return Err(miette!(
                    "source cone graph 出现重复 cone id：{}",
                    node.id.as_u32()
                ));
            }
        }
        let Some(&consumer_idx) = by_id.get(&consumer) else {
            return Err(miette!(
                "source cone graph 缺少 consumer cone id：{}",
                consumer.as_u32()
            ));
        };
        if nodes[consumer_idx].role != SourceConeRole::Consumer {
            return Err(miette!(
                "source cone graph consumer id {} 指向的 node 不是 consumer",
                consumer.as_u32()
            ));
        }
        let consumer_count = nodes
            .iter()
            .filter(|node| node.role == SourceConeRole::Consumer)
            .count();
        if consumer_count != 1 {
            return Err(miette!(
                "source cone graph 必须恰好包含一个 consumer node，但得到 {consumer_count} 个"
            ));
        }

        for node in &nodes {
            for edge in &node.dependencies {
                if !by_id.contains_key(&edge.target) {
                    return Err(miette!(
                        "source cone graph node `{}` 依赖未知 cone id {}",
                        node.manifest.cone.name,
                        edge.target.as_u32()
                    ));
                }
            }
        }

        let order = topo_order(&nodes, &by_id)?;
        let ordered_nodes = order.into_iter().map(|idx| nodes[idx].clone()).collect();
        Ok(Self {
            nodes: ordered_nodes,
            consumer,
        })
    }

    /// Return the graph nodes in validated topological order.
    pub fn nodes(&self) -> &[SourceConeNode] {
        &self.nodes
    }

    /// Return the validated consumer cone id.
    pub fn consumer_id(&self) -> ConeId {
        self.consumer
    }

    /// Return the validated consumer node.
    pub fn consumer(&self) -> &SourceConeNode {
        self.nodes
            .iter()
            .find(|node| node.id == self.consumer)
            .expect("validated source cone graph should contain consumer")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Done,
}

fn topo_order(nodes: &[SourceConeNode], by_id: &BTreeMap<ConeId, usize>) -> Result<Vec<usize>> {
    let mut ids = nodes.iter().map(|node| node.id).collect::<Vec<_>>();
    ids.sort_by(|lhs, rhs| {
        node_order_key(&nodes[by_id[lhs]]).cmp(&node_order_key(&nodes[by_id[rhs]]))
    });

    let mut states = BTreeMap::new();
    let mut ordered = Vec::with_capacity(nodes.len());
    for id in ids {
        visit(id, nodes, by_id, &mut states, &mut ordered)?;
    }
    Ok(ordered)
}

fn visit(
    id: ConeId,
    nodes: &[SourceConeNode],
    by_id: &BTreeMap<ConeId, usize>,
    states: &mut BTreeMap<ConeId, VisitState>,
    ordered: &mut Vec<usize>,
) -> Result<()> {
    match states.get(&id).copied() {
        Some(VisitState::Done) => return Ok(()),
        Some(VisitState::Visiting) => {
            return Err(miette!(
                "source cone graph dependency cycle reaches cone id {}",
                id.as_u32()
            ));
        }
        None => {}
    }

    states.insert(id, VisitState::Visiting);
    let idx = by_id[&id];
    let mut deps = nodes[idx]
        .dependencies
        .iter()
        .map(|edge| edge.target)
        .collect::<Vec<_>>();
    deps.sort_by(|lhs, rhs| {
        node_order_key(&nodes[by_id[lhs]]).cmp(&node_order_key(&nodes[by_id[rhs]]))
    });
    for dep in deps {
        visit(dep, nodes, by_id, states, ordered)?;
    }
    states.insert(id, VisitState::Done);
    ordered.push(idx);
    Ok(())
}

fn node_order_key(node: &SourceConeNode) -> (u8, &str, &Path, u32) {
    let role = match node.role {
        SourceConeRole::SysrootAuto => 0,
        SourceConeRole::LocalDependency => 1,
        SourceConeRole::Consumer => 2,
    };
    (
        role,
        node.manifest.cone.name.as_str(),
        node.root.as_path(),
        node.id.as_u32(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ConeDependencySpec, ConeSection};

    fn empty_manifest(name: &str, kind: ConeKind) -> ConeManifest {
        ConeManifest {
            cone: ConeSection {
                name: name.to_string(),
                version: "0.0.0".to_string(),
                kind,
            },
            dependencies: Default::default(),
            pre_specialize_functions: Vec::new(),
            pre_specialize_types: Vec::new(),
            export_entry_points: Vec::new(),
            selectors: Vec::new(),
            native_build: ConeNativeBuildConfig::default(),
        }
    }

    fn virtual_node(
        id: ConeId,
        role: SourceConeRole,
        name: &str,
        deps: Vec<SourceConeDependencyEdge>,
    ) -> SourceConeNode {
        let manifest = empty_manifest(name, ConeKind::Lib);
        SourceConeNode {
            id,
            role,
            root: PathBuf::from(format!("/tmp/{name}")),
            manifest_path: PathBuf::new(),
            kind: manifest.cone.kind,
            native_build: manifest.native_build.clone(),
            manifest,
            trust: SourceConeTrust::Untrusted,
            sources: vec![SourceFile::new_virtual(
                format!("/tmp/{name}/src/lib.scoop"),
                format!("package {name}\nfun marker() {{}}\n"),
            )],
            entry_main: None,
            dependencies: deps,
        }
    }

    #[test]
    fn source_cone_graph_toposorts_dependencies_before_consumer() {
        let dep_id = ConeId::new(2);
        let consumer_id = CONSUMER_CONE_ID;
        let consumer = virtual_node(
            consumer_id,
            SourceConeRole::Consumer,
            "fixture.app",
            vec![SourceConeDependencyEdge {
                target: dep_id,
                kind: SourceConeDependencyKind::LocalSource,
            }],
        );
        let dep = virtual_node(
            dep_id,
            SourceConeRole::LocalDependency,
            "fixture.dep",
            Vec::new(),
        );

        let graph = SourceConeGraph::from_nodes(vec![consumer, dep], consumer_id).unwrap();
        let names = graph
            .nodes()
            .iter()
            .map(|node| node.manifest.cone.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["fixture.dep", "fixture.app"]);
    }

    #[test]
    fn source_cone_graph_rejects_cycles() {
        let a_id = CONSUMER_CONE_ID;
        let b_id = ConeId::new(2);
        let a = virtual_node(
            a_id,
            SourceConeRole::Consumer,
            "fixture.a",
            vec![SourceConeDependencyEdge {
                target: b_id,
                kind: SourceConeDependencyKind::LocalSource,
            }],
        );
        let b = virtual_node(
            b_id,
            SourceConeRole::LocalDependency,
            "fixture.b",
            vec![SourceConeDependencyEdge {
                target: a_id,
                kind: SourceConeDependencyKind::LocalSource,
            }],
        );

        let err = SourceConeGraph::from_nodes(vec![a, b], a_id)
            .unwrap_err()
            .to_string();
        assert!(err.contains("dependency cycle"), "unexpected error: {err}");
    }

    #[test]
    fn source_cone_info_derives_resolver_info_from_project_model_identity() {
        let node = virtual_node(
            CONSUMER_CONE_ID,
            SourceConeRole::Consumer,
            "fixture.app",
            vec![],
        );
        let info = SourceConeInfo::from_node(&node);

        assert_eq!(info.id, CONSUMER_CONE_ID);
        assert_eq!(info.kind, ConeKind::Lib);
        assert_eq!(info.stable_key.name(), "fixture.app");
        assert_eq!(info.resolver_info().id, CONSUMER_CONE_ID);
    }

    #[test]
    fn stable_cone_key_reads_manifest_and_virtual_source_path() {
        let manifest = empty_manifest("demo-cone", ConeKind::Lib);
        let explicit = StableConeKey::from_manifest(&manifest);
        let virtual_key = StableConeKey::for_virtual_source_path(Path::new("/tmp/example.scoop"));

        assert_eq!(explicit.name(), "demo-cone");
        assert_eq!(explicit.version(), "0.0.0");
        assert_eq!(virtual_key.name(), "example");
        assert_eq!(virtual_key.version(), "0.0.0");
    }

    #[test]
    fn source_cone_graph_rejects_unknown_dependency() {
        let consumer = virtual_node(
            CONSUMER_CONE_ID,
            SourceConeRole::Consumer,
            "fixture.app",
            vec![SourceConeDependencyEdge {
                target: ConeId::new(99),
                kind: SourceConeDependencyKind::LocalSource,
            }],
        );

        let err = SourceConeGraph::from_nodes(vec![consumer], CONSUMER_CONE_ID)
            .unwrap_err()
            .to_string();
        assert!(err.contains("依赖未知 cone id"), "unexpected error: {err}");
    }

    #[test]
    fn dependency_spec_stays_manifest_owned() {
        let spec = ConeDependencySpec::LocalPath {
            path: "../dep".to_string(),
        };
        assert_eq!(spec.local_path(), Some("../dep"));
    }
}
