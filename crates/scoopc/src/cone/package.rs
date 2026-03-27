//! Cone 的“源级包（source package）”加载（T1102）。
//!
//! 目标：把一个 cone root 目录（含 `Cone.toml`）解析为：
//! - manifest（name/version/deps）；
//! - sources 列表（当前规则：`src/**/*.scoop`）；
//! - 可执行入口（当前规则：`src/main.scoop`）。
//!
//! 说明：
//! - 本模块只负责“目录结构 → 路径列表”的确定性规则；
//! - 依赖解析/`.cone` 归档/IR 导出等留给后续任务（T1103+）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use super::manifest::{CONE_TOML_FILE_NAME, ConeManifest};

/// cone 源码目录名（约定：`<cone-root>/src/**.scoop`）。
pub const CONE_SRC_DIR_NAME: &str = "src";

/// cone 可执行入口文件名（约定：`<cone-root>/src/main.scoop`）。
pub const CONE_MAIN_FILE_NAME: &str = "main.scoop";

/// 一个“源级 cone 包”的加载结果（目录结构 + manifest + sources）。
#[derive(Debug, Clone)]
pub struct ConeSourcePackage {
    /// cone root（包含 `Cone.toml` 的目录，已 canonicalize）。
    pub root: PathBuf,
    /// `Cone.toml` 的路径（已 canonicalize）。
    pub manifest_path: PathBuf,
    pub manifest: ConeManifest,
    /// `src/` 目录路径（已 canonicalize）。
    pub src_root: PathBuf,
    /// `src/` 下发现的全部 `.scoop` 源文件路径（已 canonicalize，稳定排序）。
    pub sources: Vec<PathBuf>,
    /// `src/main.scoop` 的路径（已 canonicalize）。
    pub main: PathBuf,
}

/// 从一个 cone root 目录加载“源级包”的 sources 列表与 main 入口。
///
/// 当前阶段规则（T1102）：
/// - `Cone.toml` 必须位于 root 目录下；
/// - sources = `root/src/**/*.scoop`（递归）；
/// - main = `root/src/main.scoop`（必须存在且是文件）。
pub fn load_cone_source_package(root: impl AsRef<Path>) -> Result<ConeSourcePackage> {
    let root = root.as_ref();
    let root = root
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法定位 cone root：{}", root.display()))?;

    if !root.is_dir() {
        return Err(miette!("cone root 不是目录：{}", root.display()));
    }

    let manifest_path = root.join(CONE_TOML_FILE_NAME);
    if !manifest_path.is_file() {
        return Err(miette!(
            "cone root 下未找到 `{CONE_TOML_FILE_NAME}`：{}",
            manifest_path.display()
        ));
    }
    let manifest_path = manifest_path
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法定位 Cone.toml：{}", manifest_path.display()))?;
    let manifest = ConeManifest::load_from_path(&manifest_path)?;

    let src_root = root.join(CONE_SRC_DIR_NAME);
    if !src_root.is_dir() {
        return Err(miette!(
            "cone package 缺少 `{CONE_SRC_DIR_NAME}` 目录：{}",
            src_root.display()
        ));
    }
    let src_root = src_root
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法定位 src 目录：{}", src_root.display()))?;

    let main = src_root.join(CONE_MAIN_FILE_NAME);
    if !main.is_file() {
        return Err(miette!(
            "cone package 缺少入口文件 `{CONE_MAIN_FILE_NAME}`：{}",
            main.display()
        ));
    }
    let main = main
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法定位 main.scoop：{}", main.display()))?;

    let mut sources = Vec::new();
    collect_scoop_files(&src_root, &mut sources)?;

    if sources.is_empty() {
        return Err(miette!(
            "cone package 的 src 目录下没有任何 `.scoop` 文件：{}",
            src_root.display()
        ));
    }

    // canonicalize + 排序，避免不同相对路径导致的重复/不稳定顺序。
    let mut canonical_sources = Vec::with_capacity(sources.len());
    for path in sources {
        let path = path
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| format!("无法定位源文件：{}", path.display()))?;
        canonical_sources.push(path);
    }
    canonical_sources.sort();

    // main 必须在 sources 列表里（正常情况下 collect 会包含它；这里做一次防御性校验）。
    if !canonical_sources.iter().any(|p| p == &main) {
        canonical_sources.push(main.clone());
        canonical_sources.sort();
    }

    Ok(ConeSourcePackage {
        root,
        manifest_path,
        manifest,
        src_root,
        sources: canonical_sources,
        main,
    })
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

