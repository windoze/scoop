//! Filesystem adapter for `Cone.toml` discovery and loading.

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

use crate::manifest::{CONE_TOML_FILE_NAME, ConeManifest};

/// 从磁盘读取并解析 `Cone.toml`。
pub fn load_cone_manifest_from_path(path: impl AsRef<Path>) -> Result<ConeManifest> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取 Cone.toml 失败：{}", path.display()))?;
    ConeManifest::parse_str(&text)
        .wrap_err_with(|| format!("解析 Cone.toml 失败：{}", path.display()))
}

/// 给定一个 cone root 目录，读取该目录下的 `Cone.toml`。
pub fn load_cone_manifest_from_dir(dir: impl AsRef<Path>) -> Result<ConeManifest> {
    load_cone_manifest_from_path(cone_manifest_path_in_dir(dir.as_ref()))
}

/// 返回目录下 `Cone.toml` 的路径（不检查是否存在）。
pub fn cone_manifest_path_in_dir(dir: &Path) -> PathBuf {
    dir.join(CONE_TOML_FILE_NAME)
}

/// 从任意路径（文件或目录）向上查找 `Cone.toml`。
pub fn discover_cone_manifest_path(start: impl AsRef<Path>) -> Option<PathBuf> {
    let start = start.as_ref();
    let dir = if start.is_dir() {
        Some(start)
    } else {
        start.parent()
    };

    for ancestor_dir in dir.into_iter().flat_map(|dir| dir.ancestors()) {
        let manifest_candidate = cone_manifest_path_in_dir(ancestor_dir);
        if manifest_candidate.is_file() {
            return Some(manifest_candidate);
        }
    }
    None
}

/// 从任意路径（文件或目录）向上查找 cone root（包含 `Cone.toml` 的目录）。
pub fn discover_cone_root(start: impl AsRef<Path>) -> Option<PathBuf> {
    discover_cone_manifest_path(start).and_then(|manifest_path| {
        manifest_path
            .parent()
            .map(|manifest_dir| manifest_dir.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_cone_manifest_path_finds_repo_fixture() {
        let fixture_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cone/minimal");

        let found = discover_cone_manifest_path(&fixture_dir).expect("应找到 Cone.toml");
        let found = found.canonicalize().unwrap();
        let expected = fixture_dir
            .join(CONE_TOML_FILE_NAME)
            .canonicalize()
            .unwrap();

        assert_eq!(found, expected);
    }
}
