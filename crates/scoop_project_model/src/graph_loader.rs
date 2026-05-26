//! Filesystem/sysroot adapter for source cone graph construction.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};
use scoopc_source::SourceFile;

use crate::graph::{
    CONSUMER_CONE_ID, ConeId, SourceConeDependencyEdge, SourceConeDependencyKind, SourceConeGraph,
    SourceConeNode, SourceConeRole, SourceConeTrust,
};
use crate::manifest::{ConeDependencySpec, ConeKind, ConeManifest};
use crate::package::ConeSourcePackage;
use crate::package_loader::{host_target_platform_id, load_cone_source_package_for_platform};
use crate::sysroot::{
    SysrootSourceConePackage, collect_auto_sysroot_source_cone_packages_for_platform,
    collect_sysroot_source_cone_packages_for_platform, select_auto_sysroot_source_cone_packages,
    sysroot_source_cone_names,
};

const FIRST_NON_CONSUMER_CONE_ID: u32 = 2;

/// Load the source cone graph for an on-disk consumer package and its dependencies.
pub fn load_source_cone_graph_for_consumer_package(
    consumer: ConeSourcePackage,
    sysroot_root: &Path,
    sysroot_overlay: Option<&Path>,
    local_dependency_roots: &[PathBuf],
    extra_sysroot_dependencies: &[String],
) -> Result<SourceConeGraph> {
    let target_platform = host_target_platform_id();
    load_source_cone_graph_for_consumer_package_for_platform(
        consumer,
        sysroot_root,
        sysroot_overlay,
        local_dependency_roots,
        extra_sysroot_dependencies,
        &target_platform,
    )
}

pub fn load_source_cone_graph_for_consumer_package_for_platform(
    consumer: ConeSourcePackage,
    sysroot_root: &Path,
    sysroot_overlay: Option<&Path>,
    local_dependency_roots: &[PathBuf],
    extra_sysroot_dependencies: &[String],
    target_platform: &str,
) -> Result<SourceConeGraph> {
    let all_sysroot_packages = collect_sysroot_source_cone_packages_for_platform(
        sysroot_root,
        sysroot_overlay,
        target_platform,
    )?;
    let sysroot_names = sysroot_source_cone_names(&all_sysroot_packages);
    let local_dependencies =
        collect_local_dependency_closure(&consumer, local_dependency_roots, target_platform)?;
    let explicit_sysroot_dependencies = collect_explicit_sysroot_dependency_names(
        std::iter::once(&consumer).chain(local_dependencies.iter()),
        &sysroot_names,
        extra_sysroot_dependencies,
    )?;
    let sysroot_packages = select_auto_sysroot_source_cone_packages(
        all_sysroot_packages,
        &explicit_sysroot_dependencies,
    )?;
    source_cone_graph_from_packages_with_extra_consumer_roots(
        sysroot_packages,
        local_dependencies,
        consumer,
        local_dependency_roots,
    )
}

/// Load the source cone graph for a synthetic single-file consumer input.
pub fn load_source_cone_graph_for_virtual_consumer(
    source: SourceFile,
    root: PathBuf,
    manifest: ConeManifest,
    sysroot_root: &Path,
    sysroot_overlay: Option<&Path>,
    extra_sysroot_dependencies: &[String],
) -> Result<SourceConeGraph> {
    let target_platform = host_target_platform_id();
    load_source_cone_graph_for_virtual_consumer_for_platform(
        source,
        root,
        manifest,
        sysroot_root,
        sysroot_overlay,
        extra_sysroot_dependencies,
        &target_platform,
    )
}

pub fn load_source_cone_graph_for_virtual_consumer_for_platform(
    source: SourceFile,
    root: PathBuf,
    manifest: ConeManifest,
    sysroot_root: &Path,
    sysroot_overlay: Option<&Path>,
    extra_sysroot_dependencies: &[String],
    target_platform: &str,
) -> Result<SourceConeGraph> {
    let sysroot_packages = collect_auto_sysroot_source_cone_packages_for_platform(
        sysroot_root,
        sysroot_overlay,
        extra_sysroot_dependencies,
        target_platform,
    )?;
    let mut nodes = Vec::with_capacity(sysroot_packages.len() + 1);
    let mut sysroot_ids = Vec::with_capacity(sysroot_packages.len());
    let sysroot_ids_by_name = sysroot_packages
        .iter()
        .enumerate()
        .map(|(offset, package)| {
            (
                package.manifest.cone.name.clone(),
                ConeId::new(FIRST_NON_CONSUMER_CONE_ID + offset as u32),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (offset, package) in sysroot_packages.into_iter().enumerate() {
        let id = ConeId::new(FIRST_NON_CONSUMER_CONE_ID + offset as u32);
        sysroot_ids.push(id);
        let mut node = source_cone_node_from_sysroot_package(id, package)?;
        node.dependencies.extend(sysroot_dependency_edges(
            &node.manifest,
            &sysroot_ids_by_name,
        )?);
        nodes.push(node);
    }

    let mut consumer = source_cone_node_from_virtual_consumer(source, root, manifest);
    consumer
        .dependencies
        .extend(sysroot_ids.into_iter().map(|id| SourceConeDependencyEdge {
            target: id,
            kind: SourceConeDependencyKind::SysrootAuto,
        }));
    nodes.push(consumer);
    SourceConeGraph::from_nodes(nodes, CONSUMER_CONE_ID)
}

fn source_cone_node_from_sysroot_package(
    id: ConeId,
    package: SysrootSourceConePackage,
) -> Result<SourceConeNode> {
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
    Ok(SourceConeNode {
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

fn source_cone_node_from_source_package(
    id: ConeId,
    role: SourceConeRole,
    package: ConeSourcePackage,
) -> Result<SourceConeNode> {
    let mut sources = Vec::with_capacity(package.sources.len());
    for path in &package.sources {
        sources.push(SourceFile::load(path)?);
    }

    let kind = package.manifest.cone.kind;
    let native_build = package.manifest.native_build.clone();
    Ok(SourceConeNode {
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

fn source_cone_node_from_virtual_consumer(
    source: SourceFile,
    root: PathBuf,
    manifest: ConeManifest,
) -> SourceConeNode {
    let kind = manifest.cone.kind;
    let native_build = manifest.native_build.clone();
    let entry_main = Some(source.path().to_path_buf());
    SourceConeNode {
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

#[cfg_attr(not(test), allow(dead_code))]
fn source_cone_graph_from_packages(
    sysroot_packages: Vec<SysrootSourceConePackage>,
    local_dependencies: Vec<ConeSourcePackage>,
    consumer: ConeSourcePackage,
) -> Result<SourceConeGraph> {
    source_cone_graph_from_packages_with_extra_consumer_roots(
        sysroot_packages,
        local_dependencies,
        consumer,
        &[],
    )
}

fn source_cone_graph_from_packages_with_extra_consumer_roots(
    sysroot_packages: Vec<SysrootSourceConePackage>,
    mut local_dependencies: Vec<ConeSourcePackage>,
    consumer: ConeSourcePackage,
    extra_consumer_dependency_roots: &[PathBuf],
) -> Result<SourceConeGraph> {
    local_dependencies.sort_by(|lhs, rhs| {
        lhs.manifest
            .cone
            .name
            .cmp(&rhs.manifest.cone.name)
            .then_with(|| lhs.root.cmp(&rhs.root))
    });

    let sysroot_names = sysroot_source_cone_names(&sysroot_packages);
    collect_explicit_sysroot_dependency_names(
        std::iter::once(&consumer).chain(local_dependencies.iter()),
        &sysroot_names,
        &[],
    )?;

    let local_dependency_ids = local_dependency_ids(
        &local_dependencies,
        FIRST_NON_CONSUMER_CONE_ID + sysroot_packages.len() as u32,
    )?;

    let mut nodes = Vec::with_capacity(sysroot_packages.len() + local_dependencies.len() + 1);
    let mut sysroot_ids = Vec::with_capacity(sysroot_packages.len());
    let sysroot_count = sysroot_packages.len();
    let sysroot_ids_by_name = sysroot_packages
        .iter()
        .enumerate()
        .map(|(offset, package)| {
            (
                package.manifest.cone.name.clone(),
                ConeId::new(FIRST_NON_CONSUMER_CONE_ID + offset as u32),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (offset, package) in sysroot_packages.into_iter().enumerate() {
        let id = ConeId::new(FIRST_NON_CONSUMER_CONE_ID + offset as u32);
        sysroot_ids.push(id);
        let mut node = source_cone_node_from_sysroot_package(id, package)?;
        node.dependencies.extend(sysroot_dependency_edges(
            &node.manifest,
            &sysroot_ids_by_name,
        )?);
        nodes.push(node);
    }

    for (offset, package) in local_dependencies.into_iter().enumerate() {
        let id = ConeId::new(FIRST_NON_CONSUMER_CONE_ID + sysroot_count as u32 + offset as u32);
        let mut node =
            source_cone_node_from_source_package(id, SourceConeRole::LocalDependency, package)?;
        node.dependencies.extend(
            sysroot_ids
                .iter()
                .copied()
                .map(|id| SourceConeDependencyEdge {
                    target: id,
                    kind: SourceConeDependencyKind::SysrootAuto,
                }),
        );
        node.dependencies.extend(local_source_dependency_edges(
            &node.manifest.cone.name,
            &node.root,
            &node.manifest.dependencies,
            &local_dependency_ids,
        )?);
        nodes.push(node);
    }

    let mut consumer =
        source_cone_node_from_source_package(CONSUMER_CONE_ID, SourceConeRole::Consumer, consumer)?;
    consumer
        .dependencies
        .extend(sysroot_ids.into_iter().map(|id| SourceConeDependencyEdge {
            target: id,
            kind: SourceConeDependencyKind::SysrootAuto,
        }));
    consumer.dependencies.extend(local_source_dependency_edges(
        &consumer.manifest.cone.name,
        &consumer.root,
        &consumer.manifest.dependencies,
        &local_dependency_ids,
    )?);
    consumer
        .dependencies
        .extend(extra_local_source_dependency_edges(
            extra_consumer_dependency_roots,
            &local_dependency_ids,
        )?);
    nodes.push(consumer);

    SourceConeGraph::from_nodes(nodes, CONSUMER_CONE_ID)
}

fn collect_local_dependency_closure(
    consumer: &ConeSourcePackage,
    extra_roots: &[PathBuf],
    target_platform: &str,
) -> Result<Vec<ConeSourcePackage>> {
    let mut packages_by_root = BTreeMap::<PathBuf, ConeSourcePackage>::new();
    for (dep_name, root) in local_path_dependency_roots(consumer)? {
        collect_local_dependency_package(
            root,
            Some((consumer.manifest.cone.name.as_str(), dep_name.as_str())),
            &mut packages_by_root,
            target_platform,
        )?;
    }
    for root in extra_roots {
        let root = canonicalize_dependency_root(
            &consumer.manifest.cone.name,
            root,
            "显式 local dependency root",
        )?;
        collect_local_dependency_package(root, None, &mut packages_by_root, target_platform)?;
    }

    Ok(packages_by_root.into_values().collect())
}

fn collect_explicit_sysroot_dependency_names<'a>(
    packages: impl Iterator<Item = &'a ConeSourcePackage>,
    sysroot_names: &BTreeSet<String>,
    extra_sysroot_dependencies: &[String],
) -> Result<Vec<String>> {
    let mut out = BTreeSet::new();
    for name in extra_sysroot_dependencies {
        out.insert(name.clone());
    }

    for package in packages {
        for (dep_name, spec) in &package.manifest.dependencies {
            match spec {
                ConeDependencySpec::Version(req) => {
                    if sysroot_names.contains(dep_name) {
                        out.insert(dep_name.clone());
                    } else {
                        return Err(unsupported_version_dependency_error(
                            &package.manifest.cone.name,
                            dep_name,
                            req,
                        ));
                    }
                }
                ConeDependencySpec::LocalPath { .. } => {}
            }
        }
    }

    Ok(out.into_iter().collect())
}

fn unsupported_version_dependency_error(
    owner_name: &str,
    dep_name: &str,
    req: &str,
) -> miette::Report {
    miette!(
        "source cone graph 暂只支持本地 path dependency 或已安装 sysroot source dependency；`{}` 的 `[dependencies].{}` 使用了版本要求 `{}`",
        owner_name,
        dep_name,
        req
    )
}

fn collect_local_dependency_package(
    root: PathBuf,
    expected: Option<(&str, &str)>,
    packages_by_root: &mut BTreeMap<PathBuf, ConeSourcePackage>,
    target_platform: &str,
) -> Result<()> {
    if let Some(existing) = packages_by_root.get(&root) {
        validate_local_dependency_package(expected, existing)?;
        return Ok(());
    }

    let package = load_cone_source_package_for_platform(&root, target_platform)?;
    validate_local_dependency_package(expected, &package)?;
    packages_by_root.insert(package.root.clone(), package.clone());

    for (dep_name, dep_root) in local_path_dependency_roots(&package)? {
        collect_local_dependency_package(
            dep_root,
            Some((package.manifest.cone.name.as_str(), dep_name.as_str())),
            packages_by_root,
            target_platform,
        )?;
    }

    Ok(())
}

fn validate_local_dependency_package(
    expected: Option<(&str, &str)>,
    package: &ConeSourcePackage,
) -> Result<()> {
    if package.manifest.cone.kind != ConeKind::Lib {
        return Err(miette!(
            "本地 source path dependency `{}` 必须声明为 `lib` cone，但当前为 `{}`",
            package.manifest.cone.name,
            package.manifest.cone.kind
        ));
    }

    if let Some((owner_name, expected_name)) = expected
        && package.manifest.cone.name != expected_name
    {
        return Err(miette!(
            "`{owner_name}` 的本地 dependency `{expected_name}` 指向 cone `{}`，dependency key 必须匹配被加载 cone 的 name",
            package.manifest.cone.name
        ));
    }

    Ok(())
}

fn local_dependency_ids(
    local_dependencies: &[ConeSourcePackage],
    first_id: u32,
) -> Result<BTreeMap<PathBuf, ConeId>> {
    let mut ids = BTreeMap::new();
    for (offset, package) in local_dependencies.iter().enumerate() {
        let id = ConeId::new(first_id + offset as u32);
        if ids.insert(package.root.clone(), id).is_some() {
            return Err(miette!(
                "source cone graph 出现重复本地 dependency root：{}",
                package.root.display()
            ));
        }
    }
    Ok(ids)
}

fn local_source_dependency_edges(
    owner_name: &str,
    owner_root: &Path,
    dependencies: &BTreeMap<String, ConeDependencySpec>,
    local_dependency_ids: &BTreeMap<PathBuf, ConeId>,
) -> Result<Vec<SourceConeDependencyEdge>> {
    let mut out = Vec::new();
    for (_dep_name, root) in local_path_dependency_roots_for(owner_name, owner_root, dependencies)?
    {
        let Some(target) = local_dependency_ids.get(&root).copied() else {
            return Err(miette!(
                "source cone graph 缺少本地 dependency node：{} 依赖 {}",
                owner_name,
                root.display()
            ));
        };
        out.push(SourceConeDependencyEdge {
            target,
            kind: SourceConeDependencyKind::LocalSource,
        });
    }
    Ok(out)
}

fn sysroot_dependency_edges(
    manifest: &ConeManifest,
    sysroot_ids_by_name: &BTreeMap<String, ConeId>,
) -> Result<Vec<SourceConeDependencyEdge>> {
    let mut out = Vec::new();
    for dep_name in manifest.dependencies.keys() {
        let Some(target) = sysroot_ids_by_name.get(dep_name).copied() else {
            return Err(miette!(
                "sysroot source cone graph 缺少 `{}` 的 dependency `{}`",
                manifest.cone.name,
                dep_name
            ));
        };
        out.push(SourceConeDependencyEdge {
            target,
            kind: SourceConeDependencyKind::SysrootAuto,
        });
    }
    Ok(out)
}

fn extra_local_source_dependency_edges(
    extra_roots: &[PathBuf],
    local_dependency_ids: &BTreeMap<PathBuf, ConeId>,
) -> Result<Vec<SourceConeDependencyEdge>> {
    let mut out = Vec::new();
    for root in extra_roots {
        let root = root
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| format!("无法定位显式 local dependency root：{}", root.display()))?;
        let Some(target) = local_dependency_ids.get(&root).copied() else {
            return Err(miette!(
                "source cone graph 缺少显式本地 dependency node：{}",
                root.display()
            ));
        };
        out.push(SourceConeDependencyEdge {
            target,
            kind: SourceConeDependencyKind::LocalSource,
        });
    }
    Ok(out)
}

fn local_path_dependency_roots(package: &ConeSourcePackage) -> Result<Vec<(String, PathBuf)>> {
    local_path_dependency_roots_for(
        &package.manifest.cone.name,
        &package.root,
        &package.manifest.dependencies,
    )
}

fn local_path_dependency_roots_for(
    owner_name: &str,
    owner_root: &Path,
    dependencies: &BTreeMap<String, ConeDependencySpec>,
) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    for (dep_name, spec) in dependencies {
        match spec {
            ConeDependencySpec::LocalPath { path } => {
                let root = owner_root.join(path);
                let root = canonicalize_dependency_root(owner_name, &root, dep_name)?;
                out.push((dep_name.clone(), root));
            }
            ConeDependencySpec::Version(_) => {}
        }
    }
    Ok(out)
}

fn canonicalize_dependency_root(owner_name: &str, root: &Path, dep_name: &str) -> Result<PathBuf> {
    let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "无法定位 `{owner_name}` 的本地 dependency `{dep_name}`：{}",
            root.display()
        )
    })?;
    if !root.is_dir() {
        return Err(miette!(
            "`{owner_name}` 的本地 dependency `{dep_name}` 不是目录：{}",
            root.display()
        ));
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SourceConeInfo;
    use crate::manifest::ConeSection;
    use crate::opt::OptLevel;
    use crate::package_loader::load_cone_source_package;
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
            "scoopc_source_cone_graph_loader_{label}_{}_{}",
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
            &root.join(crate::manifest::CONE_TOML_FILE_NAME),
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
            native_build: Default::default(),
        }
    }

    #[test]
    fn source_cone_graph_loads_sysroot_local_dependency_and_consumer_in_dag_order() {
        let temp = make_temp_dir("load_graph");
        let sysroot = temp.0.join("sysroot");
        let core = sysroot.join("lib").join("scoop.core");
        let unsafe_cone = sysroot.join("lib").join("scoop.unsafe");
        let collections = sysroot.join("lib").join("scoop.collections");
        let delegates = sysroot.join("lib").join("scoop.delegates");
        let string = sysroot.join("lib").join("scoop.lang.string");
        let thread = sysroot.join("lib").join("scoop.thread");
        let sync = sysroot.join("lib").join("scoop.sync");
        let runtime_test = sysroot.join("lib").join("scoop.runtime.test");

        write_manifest(
            &core,
            "scoop.core",
            ConeKind::Syslib,
            "[dependencies]\n\"scoop.unsafe\" = \"0.0.0\"\n",
        );
        write_manifest(&unsafe_cone, "scoop.unsafe", ConeKind::Syslib, "");
        write_manifest(&collections, "scoop.collections", ConeKind::Lib, "");
        write_manifest(&delegates, "scoop.delegates", ConeKind::Syslib, "");
        write_manifest(&string, "scoop.lang.string", ConeKind::Lib, "");
        write_manifest(&thread, "scoop.thread", ConeKind::Syslib, "");
        write_manifest(&sync, "scoop.sync", ConeKind::Syslib, "");
        write_manifest(&runtime_test, "scoop.runtime.test", ConeKind::Syslib, "");
        write_file(
            &core.join("src").join("core.scoop"),
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(
            &unsafe_cone.join("src").join("unsafe.scoop"),
            "package scoop.unsafe\n@Intrinsic class Ptr<T>\n",
        );
        write_file(
            &collections.join("src").join("collections.scoop"),
            "package scoop.collections\npublic interface Iterable<T>\n",
        );
        write_file(
            &delegates.join("src").join("delegates.scoop"),
            "package scoop.delegates\npublic interface ReadOnlyProperty<T, V>\n",
        );
        write_file(
            &string.join("src").join("lang_string.scoop"),
            "package scoop.lang.string\npublic class StringBuilder\n",
        );
        write_file(
            &thread.join("src").join("thread.scoop"),
            "package scoop.thread\npublic fun currentId(): Int = 0\n",
        );
        write_file(
            &sync.join("src").join("sync.scoop"),
            "package scoop.sync\npublic class Mutex\n",
        );
        write_file(
            &runtime_test.join("src").join("runtime_test.scoop"),
            "package scoop.runtime.test\npublic fun collect(): Unit {}\n",
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
            "[dependencies]\n\"fixture.util\" = { path = \"../fixture.util\" }\n",
        );
        write_file(
            &app.join("src").join("main.scoop"),
            "package fixture.app\nfun main(): Int = 0\n",
        );

        let consumer = load_cone_source_package(&app).unwrap();
        let graph = load_source_cone_graph_for_consumer_package(consumer, &sysroot, None, &[], &[])
            .unwrap();

        let names = graph
            .nodes()
            .iter()
            .map(|node| node.manifest.cone.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "scoop.collections",
                "scoop.unsafe",
                "scoop.core",
                "scoop.delegates",
                "scoop.lang.string",
                "fixture.util",
                "fixture.app",
            ]
        );
        assert!(!names.contains(&"scoop.thread"));
        assert!(!names.contains(&"scoop.sync"));
        assert!(!names.contains(&"scoop.runtime.test"));

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
        assert_eq!(consumer.dependencies.len(), 6);
        assert!(
            consumer
                .entry_main
                .as_ref()
                .is_some_and(|path| path.ends_with("src/main.scoop"))
        );
    }

    #[test]
    fn source_cone_graph_rejects_version_dependency_in_active_source_path() {
        let temp = make_temp_dir("version_dep_rejected");
        let app = temp.0.join("fixture.app");
        write_manifest(
            &app,
            "fixture.app",
            ConeKind::Bin,
            "[dependencies]\n\"fixture.lib\" = \"0.0.0\"\n",
        );
        write_file(
            &app.join("src").join("main.scoop"),
            "package fixture.app\nfun main(): Int = 0\n",
        );

        let consumer = load_cone_source_package(&app).unwrap();
        let err = source_cone_graph_from_packages(Vec::new(), Vec::new(), consumer)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("暂只支持本地 path dependency"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn virtual_consumer_node_uses_project_model_identity() {
        let source = SourceFile::new_virtual("/tmp/main.scoop", "package fixture\nfun main() {}\n");
        let manifest = empty_manifest("fixture.virtual", ConeKind::Bin);
        let node = source_cone_node_from_virtual_consumer(source, PathBuf::from("/tmp"), manifest);

        assert_eq!(node.id, CONSUMER_CONE_ID);
        assert_eq!(node.role, SourceConeRole::Consumer);
        assert_eq!(SourceConeInfo::from_node(&node).id, CONSUMER_CONE_ID);
    }
}
