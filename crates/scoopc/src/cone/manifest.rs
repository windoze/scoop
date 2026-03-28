//! `Cone.toml` manifest 的最小解析与发现。
//!
//! 规范参考：`SCOOP_FULL_SPEC.md` §13.7。
//!
//! 设计取舍（T1101）：
//! - 只解析 `[cone].name/[cone].version` 与 `[dependencies]`；
//! - 其它字段（例如 `scoop/ir_version/targets/pre-specialize`）后续任务再补齐；
//! - T0629b：额外解析可选的 `[entry-points].exports`（库导出入口 / host entry points）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

/// `Cone.toml` 的固定文件名。
pub const CONE_TOML_FILE_NAME: &str = "Cone.toml";

/// `[cone]` 段（T1101：只保留 name/version）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeSection {
    pub name: String,
    pub version: String,
}

/// `Cone.toml` 的最小可用 manifest。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeManifest {
    pub cone: ConeSection,
    /// 依赖（cone name → 版本要求）。
    ///
    /// 注意：T1101 先把版本要求当作纯字符串保存，不做 semver/范围解析。
    pub dependencies: BTreeMap<String, String>,
    /// program boundary：库导出入口（host/embedded entry points）。
    ///
    /// v0 约定：
    /// - 来源：`[entry-points].exports`（字符串数组）
    /// - 每一项为函数的 FQN（例如 `my.pkg.init`）
    /// - 语义：被列出的函数在 typecheck 中会按“入口（entry point）”规则强制为 `Pure!`（T0629b）
    pub export_entry_points: Vec<String>,
}

impl ConeManifest {
    /// 从文本解析 `Cone.toml`。
    pub fn parse_str(text: &str) -> Result<Self> {
        let value: toml::Value = text
            .parse()
            .map_err(|err| miette!("解析 Cone.toml 失败：{err}"))?;

        let root = value
            .as_table()
            .ok_or_else(|| miette!("Cone.toml 顶层必须是 table"))?;

        let cone = root
            .get("cone")
            .and_then(|value| value.as_table())
            .ok_or_else(|| miette!("Cone.toml 缺少 `[cone]` 段"))?;

        let name = get_required_string(cone, "name")
            .wrap_err("读取 `[cone].name` 失败")?
            .to_owned();
        let version = get_required_string(cone, "version")
            .wrap_err("读取 `[cone].version` 失败")?
            .to_owned();

        let dependencies = match root.get("dependencies") {
            None => BTreeMap::new(),
            Some(value) => {
                let table = value
                    .as_table()
                    .ok_or_else(|| miette!("`[dependencies]` 必须是 table"))?;

                let mut out = BTreeMap::new();
                for (dep_name, dep_value) in table {
                    let req = dep_value.as_str().ok_or_else(|| {
                        miette!(
                            "`[dependencies].{dep_name}` 必须是字符串版本要求（例如 \"1.0.0\"）"
                        )
                    })?;
                    out.insert(dep_name.to_owned(), req.to_owned());
                }
                out
            }
        };

        let export_entry_points = parse_export_entry_points(root)?;

        Ok(Self {
            cone: ConeSection { name, version },
            dependencies,
            export_entry_points,
        })
    }

    /// 从磁盘读取并解析 `Cone.toml`。
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("读取 Cone.toml 失败：{}", path.display()))?;
        Self::parse_str(&text).wrap_err_with(|| format!("解析 Cone.toml 失败：{}", path.display()))
    }

    /// 给定一个 cone root 目录，读取该目录下的 `Cone.toml`。
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_path(cone_manifest_path_in_dir(dir.as_ref()))
    }
}

/// 返回目录下 `Cone.toml` 的路径（不检查是否存在）。
pub fn cone_manifest_path_in_dir(dir: &Path) -> PathBuf {
    dir.join(CONE_TOML_FILE_NAME)
}

/// 从任意路径（文件或目录）向上查找 `Cone.toml`。
///
/// 返回找到的 manifest 文件路径；若未找到则返回 `None`。
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

fn get_required_string<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .ok_or_else(|| miette!("缺少字段 `{key}`"))?
        .as_str()
        .ok_or_else(|| miette!("字段 `{key}` 必须是字符串"))
}

fn parse_export_entry_points(root: &toml::Table) -> Result<Vec<String>> {
    // `entry-points` 与 `entry_points` 作为同义 key（便于 TOML 书写风格兼容）。
    let table = match root
        .get("entry-points")
        .or_else(|| root.get("entry_points"))
    {
        None => return Ok(Vec::new()),
        Some(value) => value
            .as_table()
            .ok_or_else(|| miette!("`[entry-points]` 必须是 table"))?,
    };

    let exports = match table.get("exports") {
        None => Vec::new(),
        Some(value) => {
            let arr = value
                .as_array()
                .ok_or_else(|| miette!("`[entry-points].exports` 必须是字符串数组"))?;

            let mut out = Vec::with_capacity(arr.len());
            for (idx, item) in arr.iter().enumerate() {
                let Some(s) = item.as_str() else {
                    return Err(miette!(
                        "`[entry-points].exports[{idx}]` 必须是字符串（函数 FQN）"
                    ));
                };
                out.push(s.to_owned());
            }
            out
        }
    };

    Ok(exports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_cone_toml_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "scoop-http"
version = "2.1.0"

[dependencies]
scoop-core = "1.0.0"
scoop-io = "1.2.0"
"#,
        )
        .unwrap();

        assert_eq!(manifest.cone.name, "scoop-http");
        assert_eq!(manifest.cone.version, "2.1.0");
        assert_eq!(
            manifest.dependencies.get("scoop-core").map(String::as_str),
            Some("1.0.0")
        );
        assert_eq!(
            manifest.dependencies.get("scoop-io").map(String::as_str),
            Some("1.2.0")
        );
        assert!(manifest.export_entry_points.is_empty());
    }

    #[test]
    fn parse_entry_points_exports_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"

[entry-points]
exports = ["a.b.init", "a.b.entry"]
"#,
        )
        .unwrap();

        assert_eq!(manifest.export_entry_points, vec!["a.b.init", "a.b.entry"]);
    }

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
