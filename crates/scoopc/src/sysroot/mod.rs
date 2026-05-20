//! Sysroot 加载。
//!
//! Sysroot 是一组内置 source cones，描述编译器内建的 API 表面：
//! - 对编译器：提供内建类型/函数/效果的签名来源
//! - 对工具链：LSP/文档可从 sysroot 读取类型信息
//!
//! 当前阶段：通过 `sysroot/lib/*/Cone.toml` 发现内置 source cones，
//! 并复用普通 source cone package 规则收集 `src/**/*.scoop`。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use crate::cone::manifest::{CONE_TOML_FILE_NAME, ConeKind, ConeManifest};
use crate::cone::package::{
    CONE_SRC_DIR_NAME, host_target_platform_id,
    load_cone_source_package_for_platform_with_sysroot_root,
};
use crate::source::SourceFile;

/// 外部 driver 可通过该环境变量为单次构建注入 sysroot overlay。
pub const SYSROOT_OVERLAY_ENV: &str = "SCOOP_SYSROOT_OVERLAY";

/// 普通编译默认自动加载的 sysroot cones。
pub const DEFAULT_AUTO_DEPENDENCY_CONES: [&str; 4] = [
    "scoop.core",
    "scoop.lang.string",
    "scoop.collections",
    "scoop.delegates",
];

/// sysroot 中的所有文件都以完整 AST 参与声明索引与 support-source 编译。
#[derive(Debug)]
pub struct Sysroot {
    pub root: PathBuf,
    pub files: Vec<SysrootFile>,
}

#[derive(Debug)]
pub struct SysrootFile {
    pub path: PathBuf,
    pub source: SourceFile,
    pub ast: crate::ast::File,
}

#[derive(Debug, Clone)]
pub(crate) struct SysrootSourceEntry {
    pub path: PathBuf,
    pub trusted_syslib: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SysrootSourceConePackage {
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

impl Sysroot {
    /// 默认 sysroot 路径。
    ///
    /// 当前实现是“开发期路径”：相对于 `crates/scoopc` 的 `../../sysroot`。
    /// 当编译器支持安装/分发后，这里应改为：
    /// - 优先读取 `SCOOP_SYSROOT` 环境变量
    /// - 或使用可执行文件旁的资源目录
    pub fn default_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sysroot")
    }

    pub fn load_from(root: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_with_overlay(root, None)
    }

    pub fn load_from_with_overlay(
        root: impl AsRef<Path>,
        overlay_root: Option<&Path>,
    ) -> Result<Self> {
        let root = canonicalize_sysroot_root(root.as_ref(), "sysroot")?;
        let entries = collect_merged_sysroot_entries(&root, overlay_root)?;
        Self::load_from_entries(root, entries)
    }

    pub fn load_auto_from_with_overlay_and_dependencies(
        root: impl AsRef<Path>,
        overlay_root: Option<&Path>,
        extra_dependency_names: &[String],
    ) -> Result<Self> {
        let root = canonicalize_sysroot_root(root.as_ref(), "sysroot")?;
        let entries =
            collect_auto_sysroot_source_entries(&root, overlay_root, extra_dependency_names)?;
        Self::load_from_entries(root, entries)
    }

    fn load_from_entries(root: PathBuf, entries: Vec<SysrootSourceEntry>) -> Result<Self> {
        if entries.is_empty() {
            return Err(miette!(
                "sysroot/lib 下没有可加载的 source cone：{}",
                root.display()
            ));
        }

        let mut files = Vec::new();
        for entry in entries {
            let path = entry.path;
            let source = load_sysroot_source(&path, entry.trusted_syslib)?;
            let ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;
            files.push(SysrootFile { path, source, ast });
        }

        Ok(Self { root, files })
    }

    pub fn index_files(&self) -> impl Iterator<Item = &SysrootFile> {
        self.files.iter()
    }
}

/// 收集所有 sysroot source cone 源文件路径，供 build pipeline 作为 support sources 加入 `input.sources`。
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

pub(crate) fn collect_sysroot_source_cone_packages(
    root: &Path,
    overlay_root: Option<&Path>,
) -> Result<Vec<SysrootSourceConePackage>> {
    let root = canonicalize_sysroot_root(root, "sysroot")?;
    let source_sets = collect_sysroot_cone_source_sets(&root)?;
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

pub(crate) fn collect_auto_sysroot_source_cone_packages(
    root: &Path,
    overlay_root: Option<&Path>,
    extra_dependency_names: &[String],
) -> Result<Vec<SysrootSourceConePackage>> {
    let packages = collect_sysroot_source_cone_packages(root, overlay_root)?;
    select_auto_sysroot_source_cone_packages(packages, extra_dependency_names)
}

pub(crate) fn collect_auto_sysroot_source_entries(
    root: &Path,
    overlay_root: Option<&Path>,
    extra_dependency_names: &[String],
) -> Result<Vec<SysrootSourceEntry>> {
    let source_sets =
        collect_auto_sysroot_source_cone_packages(root, overlay_root, extra_dependency_names)?;
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

pub(crate) fn select_auto_sysroot_source_cone_packages(
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

pub(crate) fn sysroot_source_cone_names(packages: &[SysrootSourceConePackage]) -> BTreeSet<String> {
    packages
        .iter()
        .map(|package| package.manifest.cone.name.clone())
        .collect()
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

fn collect_merged_sysroot_entries(
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

fn collect_sysroot_cone_source_sets(root: &Path) -> Result<Vec<SysrootConeSourceSet>> {
    let manifest_paths = collect_sysroot_cone_manifest_paths(root)?;
    let target_platform = host_target_platform_id();
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
            &target_platform,
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

fn load_sysroot_source(path: &Path, trusted_syslib: bool) -> Result<SourceFile> {
    if trusted_syslib {
        SourceFile::load_trusted_syslib(path)
    } else {
        SourceFile::load_sysroot(path)
    }
}

fn canonicalize_sysroot_root(root: &Path, label: &str) -> Result<PathBuf> {
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

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;
        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }
        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_temp_dir(prefix: &str) -> TempDirGuard {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "scoopc_sysroot_{prefix}_{}_{}",
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

    fn write_sysroot_cone_manifest(root: &Path, name: &str, kind: ConeKind) {
        write_file(
            &root.join("lib").join(name).join(CONE_TOML_FILE_NAME),
            &format!(
                "[cone]\nname = \"{name}\"\nversion = \"0.0.0\"\nkind = \"{}\"\n",
                kind.as_str()
            ),
        );
    }

    #[test]
    fn load_default_sysroot() {
        // 单元测试运行时工作目录通常是 workspace 根目录。
        // 若未来变动，可改为通过 env/config 指定。
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        assert!(!sysroot.files.is_empty());
    }

    #[test]
    fn lang_string_cone_visible_in_sysroot() {
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        let entries = sysroot
            .index_files()
            .filter(|file| {
                has_package(&file.source, file.ast.package.as_ref(), "scoop.lang.string")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            entries.len(),
            1,
            "default sysroot must index exactly one scoop.lang.string file"
        );
        let entry = entries[0];
        assert!(
            entry.ast.items.iter().any(|item| matches!(
                item,
                ast::Item::Type(decl) if entry.source.slice(decl.name.span) == "StringBuilder"
            )),
            "scoop.lang.string must index the StringBuilder surface"
        );
        assert!(
            file_has_any_block_fun(&entry.ast),
            "scoop.lang.string must keep full method bodies in the sysroot index"
        );
    }

    #[test]
    fn sysroot_regular_funs_have_body_or_explicit_annotation() {
        let root = canonicalize_sysroot_root(&Sysroot::default_path(), "sysroot").unwrap();
        let mut parsed_files = Vec::new();
        let mut violations = Vec::new();

        for entry in collect_merged_sysroot_entries(&root, None).unwrap() {
            let path = entry.path;
            let source = load_sysroot_source(&path, entry.trusted_syslib).unwrap();
            let file = crate::parser::parse_file(&source).unwrap();
            parsed_files.push((path, source, file));
        }

        for (path, source, file) in &parsed_files {
            audit_file_for_declaration_only_regular_funs(path, source, file, &mut violations);
        }

        assert!(
            violations.is_empty(),
            "sysroot ordinary functions/methods without local bodies must be `@Intrinsic`, `@Extern`, or abstract interface methods:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn overlay_paths_replace_base_and_all_files_become_support_sources() {
        let root = make_temp_dir("overlay_all_support_sources");
        let base_root = root.0.join("base");
        let overlay_root = root.0.join("overlay");
        let base_core = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let overlay_core = overlay_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let unsafe_file = base_root
            .join("lib")
            .join("scoop.unsafe")
            .join("src")
            .join("unsafe.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_sysroot_cone_manifest(&base_root, "scoop.unsafe", ConeKind::Syslib);

        write_file(
            &base_core,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(&unsafe_file, "package scoop.unsafe\ninterface PtrMarker\n");
        write_file(
            &overlay_core,
            "package scoop.core\n@Intrinsic class Array<T> { fun overlayMarker(): Int { return 7 } }\ninterface Any\n",
        );

        let sysroot = Sysroot::load_from_with_overlay(&base_root, Some(&overlay_root)).unwrap();
        let overlay_core = overlay_core.canonicalize().unwrap();
        let unsafe_file = unsafe_file.canonicalize().unwrap();

        assert_eq!(sysroot.files.len(), 2);
        assert!(sysroot.files.iter().any(|file| file.path == unsafe_file));
        assert!(sysroot.files.iter().any(|file| file.path == overlay_core));

        let mut support_sources = Vec::new();
        collect_sysroot_files(&base_root, Some(&overlay_root), &mut support_sources).unwrap();
        assert_eq!(support_sources, vec![overlay_core, unsafe_file]);
    }

    #[test]
    fn legacy_overlay_paths_outside_lib_cones_are_ignored() {
        let root = make_temp_dir("legacy_overlay_ignored");
        let base_root = root.0.join("base");
        let overlay_root = root.0.join("overlay");
        let base_core = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let legacy_overlay = overlay_root
            .join("fixtures")
            .join("typecheck")
            .join("intrinsic_surface.scoop");
        let overlay_docs = overlay_root.join("docs").join("ignored.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_file(
            &base_core,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(
            &legacy_overlay,
            "package fixtures.typecheck\n@Intrinsic fun legacy(): Int\n",
        );
        write_file(
            &overlay_docs,
            "package docs\nfun shouldNotLoad(): Int = 1\n",
        );

        let sysroot = Sysroot::load_from_with_overlay(&base_root, Some(&overlay_root)).unwrap();
        let base_core = base_core.canonicalize().unwrap();
        let legacy_overlay = legacy_overlay.canonicalize().unwrap();
        let overlay_docs = overlay_docs.canonicalize().unwrap();

        assert_eq!(sysroot.files.len(), 1);
        assert!(sysroot.files.iter().any(|file| file.path == base_core));
        assert!(!sysroot.files.iter().any(|file| file.path == legacy_overlay));
        assert!(!sysroot.files.iter().any(|file| file.path == overlay_docs));
    }

    #[test]
    fn sysroot_docs_scoop_files_are_ignored() {
        let root = make_temp_dir("docs_ignored");
        let base_root = root.0.join("base");
        let core_file = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let docs_file = base_root.join("docs").join("foo.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_file(
            &core_file,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(&docs_file, "package docs\nfun shouldNotLoad(): Int = 1\n");

        let sysroot = Sysroot::load_from(&base_root).unwrap();
        let core_file = core_file.canonicalize().unwrap();
        let docs_file = docs_file.canonicalize().unwrap();

        assert_eq!(sysroot.files.len(), 1);
        assert!(sysroot.files.iter().any(|file| file.path == core_file));
        assert!(!sysroot.files.iter().any(|file| file.path == docs_file));

        let mut support_sources = Vec::new();
        collect_sysroot_files(&base_root, None, &mut support_sources).unwrap();
        assert_eq!(support_sources, vec![core_file]);
    }

    #[test]
    fn sysroot_manifest_kind_controls_source_trust() {
        let root = make_temp_dir("manifest_kind_trust");
        let base_root = root.0.join("base");
        let core_file = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let string_file = base_root
            .join("lib")
            .join("scoop.lang.string")
            .join("src")
            .join("lang_string.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_sysroot_cone_manifest(&base_root, "scoop.lang.string", ConeKind::Lib);
        write_file(
            &core_file,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(
            &string_file,
            "package scoop.lang.string\npublic class StringBuilder\n",
        );

        let sysroot = Sysroot::load_from(&base_root).unwrap();
        let core_file = core_file.canonicalize().unwrap();
        let string_file = string_file.canonicalize().unwrap();

        let core = sysroot
            .files
            .iter()
            .find(|file| file.path == core_file)
            .unwrap();
        let string = sysroot
            .files
            .iter()
            .find(|file| file.path == string_file)
            .unwrap();

        assert!(core.source.is_trusted_syslib());
        assert!(string.source.is_sysroot());
        assert!(!string.source.is_trusted_syslib());

        let overlay_root = root.0.join("overlay");
        let string_overlay_extra = overlay_root
            .join("lib")
            .join("scoop.lang.string")
            .join("src")
            .join("extra.scoop");
        write_file(
            &string_overlay_extra,
            "package scoop.lang.string\npublic fun overlayStringToken(): Int = 1\n",
        );

        let overlaid = Sysroot::load_from_with_overlay(&base_root, Some(&overlay_root)).unwrap();
        let string_overlay_extra = string_overlay_extra.canonicalize().unwrap();
        let extra = overlaid
            .files
            .iter()
            .find(|file| file.path == string_overlay_extra)
            .unwrap();

        assert!(extra.source.is_sysroot());
        assert!(!extra.source.is_trusted_syslib());
    }

    #[test]
    fn default_core_is_full_ast_index_file_and_support_source() {
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        let mut support_sources = Vec::new();
        collect_sysroot_files(&Sysroot::default_path(), None, &mut support_sources).unwrap();

        let core = sysroot
            .index_files()
            .find(|file| {
                file.path
                    .file_name()
                    .is_some_and(|name| name == "core.scoop")
            })
            .expect("default core.scoop must remain visible through Sysroot::index_files()");
        assert!(
            support_sources.contains(&core.path),
            "default core.scoop must be compiled as a support source"
        );
        assert!(
            file_has_any_block_fun(&core.ast),
            "default core.scoop must keep full bodies in Sysroot::index_files()"
        );
    }

    fn audit_file_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        violations: &mut Vec<String>,
    ) {
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        fun,
                        FunAuditContainer::TopLevel,
                        violations,
                    );
                }
                ast::Item::Type(decl) => {
                    audit_type_for_declaration_only_regular_funs(
                        path, source, file, decl, violations,
                    );
                }
                ast::Item::Object(obj) => {
                    audit_object_for_declaration_only_regular_funs(
                        path, source, file, obj, violations,
                    );
                }
                ast::Item::TypeAlias(_) | ast::Item::ExtensionProperty(_) | ast::Item::Val(_) => {}
            }
        }
    }

    fn audit_type_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        decl: &ast::TypeDecl,
        violations: &mut Vec<String>,
    ) {
        let Some(body) = &decl.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        fun,
                        FunAuditContainer::Type(decl.kind),
                        violations,
                    );
                }
                ast::TypeMember::Type(nested) => {
                    audit_type_for_declaration_only_regular_funs(
                        path, source, file, nested, violations,
                    );
                }
                ast::TypeMember::Object(obj) => {
                    audit_object_for_declaration_only_regular_funs(
                        path, source, file, obj, violations,
                    );
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::InitBlock(_) => {}
            }
        }
    }

    fn audit_object_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        obj: &ast::ObjectDecl,
        violations: &mut Vec<String>,
    ) {
        let Some(body) = &obj.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        fun,
                        FunAuditContainer::Object,
                        violations,
                    );
                }
                ast::TypeMember::Type(nested) => {
                    audit_type_for_declaration_only_regular_funs(
                        path, source, file, nested, violations,
                    );
                }
                ast::TypeMember::Object(nested) => {
                    audit_object_for_declaration_only_regular_funs(
                        path, source, file, nested, violations,
                    );
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::InitBlock(_) => {}
            }
        }
    }

    fn audit_fun_for_declaration_only_regular_fun(
        path: &Path,
        source: &SourceFile,
        fun: &ast::FunDecl,
        container: FunAuditContainer,
        violations: &mut Vec<String>,
    ) {
        if !matches!(fun.body, ast::FunBody::Missing) || fun.kind != ast::FunDeclKind::Regular {
            return;
        }
        if matches!(container, FunAuditContainer::Type(ast::TypeKind::Interface)) {
            return;
        }
        if has_builtin_annotation(source, &fun.annotations, "Intrinsic")
            || has_builtin_annotation(source, &fun.annotations, "Extern")
        {
            return;
        }
        violations.push(format!(
            "{}: `{}`",
            path.display(),
            source.slice(fun.name.span)
        ));
    }

    #[derive(Clone, Copy)]
    enum FunAuditContainer {
        TopLevel,
        Type(ast::TypeKind),
        Object,
    }

    fn has_builtin_annotation(
        source: &SourceFile,
        annotations: &[ast::AnnotationUse],
        name: &str,
    ) -> bool {
        annotations.iter().any(|annotation| {
            annotation
                .path
                .last()
                .is_some_and(|segment| segment.text(source) == name)
        })
    }

    fn file_has_any_block_fun(file: &ast::File) -> bool {
        file.items.iter().any(item_has_any_block_fun)
    }

    fn item_has_any_block_fun(item: &ast::Item) -> bool {
        match item {
            ast::Item::Fun(fun) => matches!(fun.body, ast::FunBody::Block(_)),
            ast::Item::Type(decl) => decl
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::Item::Object(obj) => obj
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::Item::TypeAlias(_) | ast::Item::ExtensionProperty(_) | ast::Item::Val(_) => false,
        }
    }

    fn type_member_has_any_block_fun(member: &ast::TypeMember) -> bool {
        match member {
            ast::TypeMember::Fun(fun) => matches!(fun.body, ast::FunBody::Block(_)),
            ast::TypeMember::Type(decl) => decl
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::TypeMember::Object(obj) => obj
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::InitBlock(_) => false,
        }
    }

    fn has_package(
        source: &SourceFile,
        package: Option<&ast::PackageDecl>,
        expected: &str,
    ) -> bool {
        let Some(package) = package else {
            return expected.is_empty();
        };
        let actual = package
            .path
            .iter()
            .map(|segment| segment.text(source))
            .collect::<Vec<_>>()
            .join(".");
        actual == expected
    }
}
