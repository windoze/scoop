//! Sysroot path-layer (无 AST 依赖)。
//!
//! 本模块负责 sysroot 目录中的“cone 发现 + 源文件路径收集”，不参与 AST 解析。
//! AST 持有 sysroot 由 stage crate（如 `scoopc_hir::sysroot`）在此基础上构建。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use crate::manifest::{CONE_TOML_FILE_NAME, ConeKind, ConeManifest};
use crate::package::CONE_SRC_DIR_NAME;
use crate::package_loader::{
    collect_scoop_files, host_target_platform_id,
    load_cone_source_package_for_platform_with_sysroot_root,
};

/// 外部 driver 可通过该环境变量为单次构建注入 sysroot overlay。
pub const SYSROOT_OVERLAY_ENV: &str = "SCOOP_SYSROOT_OVERLAY";

/// 普通编译默认自动加载的 sysroot cones。
pub const DEFAULT_AUTO_DEPENDENCY_CONES: [&str; 4] = [
    "scoop.core",
    "scoop.lang.string",
    "scoop.collections",
    "scoop.delegates",
];

/// 默认 sysroot 路径（开发期路径：相对 workspace 根的 `sysroot/`）。
pub fn default_sysroot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sysroot")
}

#[derive(Debug, Clone)]
pub struct SysrootSourceEntry {
    pub path: PathBuf,
    pub trusted_syslib: bool,
}

#[derive(Debug, Clone)]
pub struct SysrootSourceConePackage {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: ConeManifest,
    pub trusted_syslib: bool,
    pub sources: Vec<PathBuf>,
}

#[derive(Debug)]
struct SysrootConeSourceSet {
    root: PathBuf,
    root_rel: PathBuf,
    manifest_path: PathBuf,
    manifest: ConeManifest,
    trusted_syslib: bool,
    sources: Vec<PathBuf>,
}

/// 收集所有 sysroot source cone 源文件路径，供 build pipeline 加入 build-closure source view。
pub fn collect_sysroot_files(
    root: &Path,
    overlay_root: Option<&Path>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let root = canonicalize_sysroot_root(root, "sysroot")?;
    out.extend(
        collect_merged_sysroot_entries(&root, overlay_root)?
            .into_iter()
            .map(|entry| entry.path),
    );
    Ok(())
}

pub fn collect_sysroot_source_cone_packages(
    root: &Path,
    overlay_root: Option<&Path>,
) -> Result<Vec<SysrootSourceConePackage>> {
    let target_platform = host_target_platform_id();
    collect_sysroot_source_cone_packages_for_platform(root, overlay_root, &target_platform)
}

pub fn collect_sysroot_source_cone_packages_for_platform(
    root: &Path,
    overlay_root: Option<&Path>,
    target_platform: &str,
) -> Result<Vec<SysrootSourceConePackage>> {
    let root = canonicalize_sysroot_root(root, "sysroot")?;
    let source_sets = collect_sysroot_cone_source_sets(&root, target_platform)?;
    let overlay_root = overlay_root
        .map(|overlay_root| canonicalize_sysroot_root(overlay_root, "sysroot overlay"))
        .transpose()?;
    let mut out = Vec::with_capacity(source_sets.len());

    for source_set in source_sets {
        let mut merged = BTreeMap::new();
        for path in &source_set.sources {
            let rel = path
                .strip_prefix(&root)
                .expect("sysroot file should be under canonical root")
                .to_path_buf();
            merged.insert(
                rel,
                SysrootSourceEntry {
                    path: path.clone(),
                    trusted_syslib: source_set.trusted_syslib,
                },
            );
        }

        if let Some(overlay_root) = overlay_root.as_deref() {
            merge_overlay_cone_sources(overlay_root, &source_set, &mut merged)?;
        }

        out.push(SysrootSourceConePackage {
            root: source_set.root,
            manifest_path: source_set.manifest_path,
            manifest: source_set.manifest,
            trusted_syslib: source_set.trusted_syslib,
            sources: merged.into_values().map(|entry| entry.path).collect(),
        });
    }

    Ok(out)
}

pub fn collect_auto_sysroot_source_cone_packages(
    root: &Path,
    overlay_root: Option<&Path>,
    extra_dependency_names: &[String],
) -> Result<Vec<SysrootSourceConePackage>> {
    let target_platform = host_target_platform_id();
    collect_auto_sysroot_source_cone_packages_for_platform(
        root,
        overlay_root,
        extra_dependency_names,
        &target_platform,
    )
}

pub fn collect_auto_sysroot_source_cone_packages_for_platform(
    root: &Path,
    overlay_root: Option<&Path>,
    extra_dependency_names: &[String],
    target_platform: &str,
) -> Result<Vec<SysrootSourceConePackage>> {
    let packages =
        collect_sysroot_source_cone_packages_for_platform(root, overlay_root, target_platform)?;
    select_auto_sysroot_source_cone_packages(packages, extra_dependency_names)
}

pub fn collect_auto_sysroot_source_entries(
    root: &Path,
    overlay_root: Option<&Path>,
    extra_dependency_names: &[String],
) -> Result<Vec<SysrootSourceEntry>> {
    let target_platform = host_target_platform_id();
    collect_auto_sysroot_source_entries_for_platform(
        root,
        overlay_root,
        extra_dependency_names,
        &target_platform,
    )
}

pub fn collect_auto_sysroot_source_entries_for_platform(
    root: &Path,
    overlay_root: Option<&Path>,
    extra_dependency_names: &[String],
    target_platform: &str,
) -> Result<Vec<SysrootSourceEntry>> {
    let source_sets = collect_auto_sysroot_source_cone_packages_for_platform(
        root,
        overlay_root,
        extra_dependency_names,
        target_platform,
    )?;
    let source_count = source_sets.iter().map(|set| set.sources.len()).sum();
    let mut entries = Vec::with_capacity(source_count);

    for source_set in &source_sets {
        for path in &source_set.sources {
            entries.push(SysrootSourceEntry {
                path: path.clone(),
                trusted_syslib: source_set.trusted_syslib,
            });
        }
    }

    Ok(entries)
}

pub fn select_auto_sysroot_source_cone_packages(
    packages: Vec<SysrootSourceConePackage>,
    extra_dependency_names: &[String],
) -> Result<Vec<SysrootSourceConePackage>> {
    let mut packages_by_name = BTreeMap::new();
    for package in packages {
        let name = package.manifest.cone.name.clone();
        if packages_by_name.insert(name.clone(), package).is_some() {
            return Err(miette!("sysroot source cone name 重复：{name}"));
        }
    }

    let mut states = BTreeMap::new();
    let mut selected = Vec::new();
    for name in DEFAULT_AUTO_DEPENDENCY_CONES {
        visit_sysroot_dependency(name, &packages_by_name, &mut states, &mut selected)?;
    }
    for name in extra_dependency_names {
        visit_sysroot_dependency(name, &packages_by_name, &mut states, &mut selected)?;
    }

    let mut out = Vec::with_capacity(selected.len());
    for name in selected {
        out.push(
            packages_by_name
                .get(&name)
                .expect("selected sysroot cone should exist")
                .clone(),
        );
    }
    Ok(out)
}

pub fn sysroot_source_cone_names(packages: &[SysrootSourceConePackage]) -> BTreeSet<String> {
    packages
        .iter()
        .map(|package| package.manifest.cone.name.clone())
        .collect()
}

pub fn collect_merged_sysroot_entries(
    root: &Path,
    overlay_root: Option<&Path>,
) -> Result<Vec<SysrootSourceEntry>> {
    let source_sets = collect_sysroot_source_cone_packages(root, overlay_root)?;
    let source_count = source_sets.iter().map(|set| set.sources.len()).sum();
    let mut entries = Vec::with_capacity(source_count);

    for source_set in &source_sets {
        for path in &source_set.sources {
            entries.push(SysrootSourceEntry {
                path: path.clone(),
                trusted_syslib: source_set.trusted_syslib,
            });
        }
    }

    Ok(entries)
}

pub fn canonicalize_sysroot_root(root: &Path, label: &str) -> Result<PathBuf> {
    let root = root.to_path_buf();
    let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "无法定位 {label} 目录：{}（当前实现默认相对工作目录）",
            root.display()
        )
    })?;
    if !root.is_dir() {
        return Err(miette!("{label} 不是目录：{}", root.display()));
    }
    Ok(root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SysrootDependencyVisitState {
    Visiting,
    Done,
}

fn visit_sysroot_dependency(
    name: &str,
    packages_by_name: &BTreeMap<String, SysrootSourceConePackage>,
    states: &mut BTreeMap<String, SysrootDependencyVisitState>,
    selected: &mut Vec<String>,
) -> Result<()> {
    match states.get(name).copied() {
        Some(SysrootDependencyVisitState::Done) => return Ok(()),
        Some(SysrootDependencyVisitState::Visiting) => {
            return Err(miette!(
                "sysroot source cone dependency cycle reaches `{name}`"
            ));
        }
        None => {}
    }

    let package = packages_by_name
        .get(name)
        .ok_or_else(|| miette!("sysroot dependency `{name}` 未找到对应 source cone"))?;
    let dependency_names = package
        .manifest
        .dependencies
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    states.insert(name.to_string(), SysrootDependencyVisitState::Visiting);
    for dependency_name in dependency_names {
        visit_sysroot_dependency(&dependency_name, packages_by_name, states, selected)?;
    }
    states.insert(name.to_string(), SysrootDependencyVisitState::Done);
    selected.push(name.to_string());
    Ok(())
}

fn merge_overlay_cone_sources(
    overlay_root: &Path,
    source_set: &SysrootConeSourceSet,
    merged: &mut BTreeMap<PathBuf, SysrootSourceEntry>,
) -> Result<()> {
    let overlay_cone_root = overlay_root.join(&source_set.root_rel);
    let overlay_src_root = overlay_cone_root.join(CONE_SRC_DIR_NAME);
    if !overlay_src_root.is_dir() {
        return Ok(());
    }

    let mut overlay_paths = Vec::new();
    collect_scoop_files(&overlay_src_root, &mut overlay_paths)?;
    for path in overlay_paths {
        let path = path
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| format!("无法定位 sysroot overlay 源文件：{}", path.display()))?;
        let rel_inside_cone = path
            .strip_prefix(&overlay_cone_root)
            .expect("overlay source should be under the overlay cone root");
        let rel = source_set.root_rel.join(rel_inside_cone);
        merged.insert(
            rel,
            SysrootSourceEntry {
                path,
                trusted_syslib: source_set.trusted_syslib,
            },
        );
    }

    Ok(())
}

fn collect_sysroot_cone_source_sets(
    root: &Path,
    target_platform: &str,
) -> Result<Vec<SysrootConeSourceSet>> {
    let manifest_paths = collect_sysroot_cone_manifest_paths(root)?;
    let mut source_sets = Vec::new();

    for manifest_path in manifest_paths {
        let cone_root = manifest_path
            .parent()
            .expect("Cone.toml path should have a cone root parent");
        let root_rel = cone_root
            .strip_prefix(root)
            .expect("sysroot cone root should be under canonical root")
            .to_path_buf();
        let package = load_cone_source_package_for_platform_with_sysroot_root(
            cone_root,
            target_platform,
            root,
        )?;
        let trusted_syslib = package.manifest.cone.kind == ConeKind::Syslib;
        source_sets.push(SysrootConeSourceSet {
            root: package.root,
            root_rel,
            manifest_path: package.manifest_path,
            manifest: package.manifest,
            trusted_syslib,
            sources: package.sources,
        });
    }

    source_sets.sort_by(|lhs, rhs| lhs.root_rel.cmp(&rhs.root_rel));
    Ok(source_sets)
}

fn collect_sysroot_cone_manifest_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let lib_root = root.join("lib");
    if !lib_root.is_dir() {
        return Err(miette!(
            "sysroot 缺少 `lib` 目录，无法发现 source cones：{}",
            lib_root.display()
        ));
    }

    let mut manifests = Vec::new();
    for entry in std::fs::read_dir(&lib_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取 sysroot lib 目录：{}", lib_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if !entry.file_type().into_diagnostic()?.is_dir() {
            continue;
        }

        let manifest_path = path.join(CONE_TOML_FILE_NAME);
        if manifest_path.is_file() {
            manifests.push(
                manifest_path
                    .canonicalize()
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        format!(
                            "无法定位 sysroot cone manifest：{}",
                            manifest_path.display()
                        )
                    })?,
            );
        }
    }

    manifests.sort();
    Ok(manifests)
}
