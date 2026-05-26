//! Stage-independent source cone package data.

use std::path::PathBuf;

use crate::manifest::ConeManifest;

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
    /// `bin` cone 的 `src/main.scoop` 路径（已 canonicalize）。`lib/syslib` 不拥有入口锚点。
    pub main: Option<PathBuf>,
}
