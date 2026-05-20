//! Source cone graph construction.
//!
//! The graph is the authoritative project input for source-only builds.  The
//! frontend may still flatten graph sources into one compilation unit, but the
//! graph preserves each file's owning cone, kind, trust, native-build metadata,
//! and dependency edges.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miette::{Result, miette};

use crate::cone::{ConeKind, ConeManifest, ConeNativeBuildConfig, ConeSourcePackage};
use crate::resolve::ConeId;
use crate::source::SourceFile;

pub const CONSUMER_CONE_ID: ConeId = ConeId::new(1);
const FIRST_NON_CONSUMER_CONE_ID: u32 = 2;

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
    pub fn is_trusted_syslib(self) -> bool {
        self == Self::TrustedSyslib
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

impl SourceConeNode {
    fn from_sysroot_package(
        id: ConeId,
        package: crate::sysroot::SysrootSourceConePackage,
    ) -> Result<Self> {
        let trust = if package.trusted_syslib {
            SourceConeTrust::TrustedSyslib
        } else {
            SourceConeTrust::Untrusted
        };
        let mut sources = Vec::with_capacity(package.sources.len());
        for path in &package.sources {
            sources.push(if trust.is_trusted_syslib() {
                SourceFile::load_trusted_syslib(path)?
            } else {
                SourceFile::load_sysroot(path)?
            });
        }

        let kind = package.manifest.cone.kind;
        let native_build = package.manifest.native_build.clone();
        Ok(Self {
            id,
            role: SourceConeRole::SysrootAuto,
            root: package.root,
            manifest_path: package.manifest_path,
            manifest: package.manifest,
            kind,
            native_build,
            trust,
            sources,
            entry_main: None,
            dependencies: Vec::new(),
        })
    }

    fn from_source_package(
        id: ConeId,
        role: SourceConeRole,
        package: ConeSourcePackage,
    ) -> Result<Self> {
        let mut sources = Vec::with_capacity(package.sources.len());
        for path in &package.sources {
            sources.push(SourceFile::load(path)?);
        }

        let kind = package.manifest.cone.kind;
        let native_build = package.manifest.native_build.clone();
        Ok(Self {
            id,
            role,
            root: package.root,
            manifest_path: package.manifest_path,
            manifest: package.manifest,
            kind,
            native_build,
            trust: SourceConeTrust::Untrusted,
            sources,
            entry_main: package.main,
            dependencies: Vec::new(),
        })
    }

    fn from_virtual_consumer(source: SourceFile, root: PathBuf, manifest: ConeManifest) -> Self {
        let kind = manifest.cone.kind;
        let native_build = manifest.native_build.clone();
        let entry_main = Some(source.path().to_path_buf());
        Self {
            id: CONSUMER_CONE_ID,
            role: SourceConeRole::Consumer,
            root,
            manifest_path: PathBuf::new(),
            manifest,
            kind,
            native_build,
            trust: SourceConeTrust::Untrusted,
            sources: vec![source],
            entry_main,
            dependencies: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceConeGraph {
    nodes: Vec<SourceConeNode>,
    consumer: ConeId,
}

impl SourceConeGraph {
    pub fn load_for_consumer_package(
        consumer: ConeSourcePackage,
        sysroot_root: &Path,
        sysroot_overlay: Option<&Path>,
        local_dependency_roots: &[PathBuf],
    ) -> Result<Self> {
        let sysroot_packages =
            crate::sysroot::collect_sysroot_source_cone_packages(sysroot_root, sysroot_overlay)?;
        let mut local_dependencies = Vec::with_capacity(local_dependency_roots.len());
        for root in local_dependency_roots {
            local_dependencies.push(crate::cone::load_cone_source_package(root)?);
        }
        Self::from_packages(sysroot_packages, local_dependencies, consumer)
    }

    pub fn load_for_virtual_consumer(
        source: SourceFile,
        root: PathBuf,
        manifest: ConeManifest,
        sysroot_root: &Path,
        sysroot_overlay: Option<&Path>,
    ) -> Result<Self> {
        let sysroot_packages =
            crate::sysroot::collect_sysroot_source_cone_packages(sysroot_root, sysroot_overlay)?;
        let mut nodes = Vec::with_capacity(sysroot_packages.len() + 1);
        let mut sysroot_ids = Vec::with_capacity(sysroot_packages.len());
        for (offset, package) in sysroot_packages.into_iter().enumerate() {
            let id = ConeId::new(FIRST_NON_CONSUMER_CONE_ID + offset as u32);
            sysroot_ids.push(id);
            nodes.push(SourceConeNode::from_sysroot_package(id, package)?);
        }

        let mut consumer = SourceConeNode::from_virtual_consumer(source, root, manifest);
        consumer
            .dependencies
            .extend(sysroot_ids.into_iter().map(|id| SourceConeDependencyEdge {
                target: id,
                kind: SourceConeDependencyKind::SysrootAuto,
            }));
        nodes.push(consumer);
        Self::from_nodes(nodes, CONSUMER_CONE_ID)
    }

    pub(crate) fn from_packages(
        sysroot_packages: Vec<crate::sysroot::SysrootSourceConePackage>,
        mut local_dependencies: Vec<ConeSourcePackage>,
        consumer: ConeSourcePackage,
    ) -> Result<Self> {
        local_dependencies.sort_by(|lhs, rhs| {
            lhs.manifest
                .cone
                .name
                .cmp(&rhs.manifest.cone.name)
                .then_with(|| lhs.root.cmp(&rhs.root))
        });

        let mut nodes = Vec::with_capacity(sysroot_packages.len() + local_dependencies.len() + 1);
        let mut sysroot_ids = Vec::with_capacity(sysroot_packages.len());
        let mut local_dependency_ids = Vec::with_capacity(local_dependencies.len());
        let sysroot_count = sysroot_packages.len();

        for (offset, package) in sysroot_packages.into_iter().enumerate() {
            let id = ConeId::new(FIRST_NON_CONSUMER_CONE_ID + offset as u32);
            sysroot_ids.push(id);
            nodes.push(SourceConeNode::from_sysroot_package(id, package)?);
        }

        for (offset, package) in local_dependencies.into_iter().enumerate() {
            let id = ConeId::new(FIRST_NON_CONSUMER_CONE_ID + sysroot_count as u32 + offset as u32);
            local_dependency_ids.push(id);
            let mut node =
                SourceConeNode::from_source_package(id, SourceConeRole::LocalDependency, package)?;
            node.dependencies
                .extend(
                    sysroot_ids
                        .iter()
                        .copied()
                        .map(|id| SourceConeDependencyEdge {
                            target: id,
                            kind: SourceConeDependencyKind::SysrootAuto,
                        }),
                );
            nodes.push(node);
        }

        let mut consumer = SourceConeNode::from_source_package(
            CONSUMER_CONE_ID,
            SourceConeRole::Consumer,
            consumer,
        )?;
        consumer
            .dependencies
            .extend(sysroot_ids.into_iter().map(|id| SourceConeDependencyEdge {
                target: id,
                kind: SourceConeDependencyKind::SysrootAuto,
            }));
        consumer
            .dependencies
            .extend(
                local_dependency_ids
                    .into_iter()
                    .map(|id| SourceConeDependencyEdge {
                        target: id,
                        kind: SourceConeDependencyKind::LocalSource,
                    }),
            );
        nodes.push(consumer);

        Self::from_nodes(nodes, CONSUMER_CONE_ID)
    }

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

    pub fn nodes(&self) -> &[SourceConeNode] {
        &self.nodes
    }

    pub fn consumer_id(&self) -> ConeId {
        self.consumer
    }

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
    use crate::cone::{ConeSection, load_cone_source_package};
    use crate::opt::OptLevel;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_temp_dir(label: &str) -> TempDirGuard {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "scoopc_source_cone_graph_{label}_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    fn write_file(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, text).unwrap();
    }

    fn write_manifest(root: &Path, name: &str, kind: ConeKind, extra: &str) {
        write_file(
            &root.join(crate::cone::CONE_TOML_FILE_NAME),
            &format!(
                "[cone]\nname = \"{name}\"\nversion = \"0.0.0\"\nkind = \"{}\"\n{extra}",
                kind.as_str()
            ),
        );
    }

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
    fn source_cone_graph_loads_sysroot_local_dependency_and_consumer_in_dag_order() {
        let temp = make_temp_dir("load_graph");
        let sysroot = temp.0.join("sysroot");
        let core = sysroot.join("lib").join("scoop.core");
        let string = sysroot.join("lib").join("scoop.lang.string");
        write_manifest(&core, "scoop.core", ConeKind::Syslib, "");
        write_manifest(&string, "scoop.lang.string", ConeKind::Lib, "");
        write_file(
            &core.join("src").join("core.scoop"),
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(
            &string.join("src").join("lang_string.scoop"),
            "package scoop.lang.string\npublic class StringBuilder\n",
        );

        let dep = temp.0.join("fixture.util");
        write_manifest(
            &dep,
            "fixture.util",
            ConeKind::Lib,
            "[native-build]\nc-sources = [\"native/util.c\"]\nopt-level = \"2\"\n",
        );
        write_file(
            &dep.join("src").join("api.scoop"),
            "package fixture.util\npublic fun value(): Int = 1\n",
        );
        write_file(
            &dep.join("native").join("util.c"),
            "int util(void) { return 1; }\n",
        );

        let app = temp.0.join("fixture.app");
        write_manifest(
            &app,
            "fixture.app",
            ConeKind::Bin,
            "[dependencies]\n\"fixture.util\" = \"0.0.0\"\n",
        );
        write_file(
            &app.join("src").join("main.scoop"),
            "package fixture.app\nfun main(): Int = 0\n",
        );

        let consumer = load_cone_source_package(&app).unwrap();
        let graph = SourceConeGraph::load_for_consumer_package(
            consumer,
            &sysroot,
            None,
            std::slice::from_ref(&dep),
        )
        .unwrap();

        let names = graph
            .nodes()
            .iter()
            .map(|node| node.manifest.cone.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "scoop.core",
                "scoop.lang.string",
                "fixture.util",
                "fixture.app",
            ]
        );

        let core_node = graph
            .nodes()
            .iter()
            .find(|node| node.manifest.cone.name == "scoop.core")
            .unwrap();
        assert_eq!(core_node.kind, ConeKind::Syslib);
        assert_eq!(core_node.trust, SourceConeTrust::TrustedSyslib);
        assert!(core_node.sources[0].is_trusted_syslib());

        let string_node = graph
            .nodes()
            .iter()
            .find(|node| node.manifest.cone.name == "scoop.lang.string")
            .unwrap();
        assert_eq!(string_node.kind, ConeKind::Lib);
        assert_eq!(string_node.trust, SourceConeTrust::Untrusted);
        assert!(string_node.sources[0].is_sysroot());
        assert!(!string_node.sources[0].is_trusted_syslib());

        let dep_node = graph
            .nodes()
            .iter()
            .find(|node| node.manifest.cone.name == "fixture.util")
            .unwrap();
        assert_eq!(dep_node.role, SourceConeRole::LocalDependency);
        assert_eq!(dep_node.native_build.c_sources, vec!["native/util.c"]);
        assert_eq!(dep_node.native_build.opt_level, Some(OptLevel::O2));

        let consumer = graph.consumer();
        assert_eq!(consumer.id, CONSUMER_CONE_ID);
        assert_eq!(consumer.kind, ConeKind::Bin);
        assert_eq!(consumer.role, SourceConeRole::Consumer);
        assert_eq!(consumer.dependencies.len(), 3);
        assert!(
            consumer
                .entry_main
                .as_ref()
                .is_some_and(|path| path.ends_with("src/main.scoop"))
        );
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
}
