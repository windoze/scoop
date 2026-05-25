//! Cone 项目的 build 产物目录布局（T1121）。
//!
//! 约定来自 `CONE-IMPROVEMENTS.md`：
//! - 默认/host：`build/<profile>/...`
//! - 预留 cross compile：`build/<target>/<profile>/...`
//! - 可执行：`build/<profile>/<project-name>`（Windows 为 `.exe`）
//! - 中间产物：`build/<profile>/obj/`

use std::path::{Path, PathBuf};

pub(crate) fn cone_build_dir(cone_root: &Path, target: Option<&str>, profile: &str) -> PathBuf {
    // 目录层级约定：
    // - `build/<profile>/`：默认/host（保持简单，兼容早期习惯）
    // - `build/<target>/<profile>/`：为未来 cross compile 预留隔离层
    let mut dir = cone_root.join("build");
    if let Some(target) = target {
        dir = dir.join(target);
    }
    dir.join(profile)
}

#[cfg(feature = "llvm")]
pub(crate) fn cone_obj_dir(cone_root: &Path, target: Option<&str>, profile: &str) -> PathBuf {
    cone_build_dir(cone_root, target, profile).join("obj")
}

pub(crate) fn cone_build_json_path(
    cone_root: &Path,
    target: Option<&str>,
    profile: &str,
) -> PathBuf {
    cone_build_dir(cone_root, target, profile).join(super::incremental::BUILD_JSON_FILE_NAME)
}

#[cfg(feature = "llvm")]
pub(crate) fn cone_link_dir(
    cone_root: &Path,
    target: Option<&str>,
    profile: &str,
    cone_key: &str,
) -> PathBuf {
    cone_build_dir(cone_root, target, profile)
        .join("link")
        .join(cone_key)
}

pub(crate) fn cone_exe_file_name(project_name: &str) -> String {
    let ext = std::env::consts::EXE_EXTENSION;
    if ext.is_empty() {
        project_name.to_string()
    } else {
        format!("{project_name}.{ext}")
    }
}

pub(crate) fn cone_exe_path(
    cone_root: &Path,
    target: Option<&str>,
    profile: &str,
    project_name: &str,
) -> PathBuf {
    cone_build_dir(cone_root, target, profile).join(cone_exe_file_name(project_name))
}

#[cfg(feature = "llvm")]
pub(crate) fn obj_file_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.obj")
    } else {
        format!("{stem}.o")
    }
}
