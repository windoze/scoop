//! Stage-independent `Cone.toml` manifest model and parser.

use std::collections::BTreeMap;

use miette::{Context as _, Result, miette};

use crate::opt::OptLevel;

/// `Cone.toml` 的固定文件名。
pub const CONE_TOML_FILE_NAME: &str = "Cone.toml";

/// Cone 的构建/加载类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConeKind {
    Bin,
    Lib,
    Syslib,
}

impl ConeKind {
    /// 解析 manifest 中的 `[cone].kind` 字符串。
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "bin" => Ok(Self::Bin),
            "lib" => Ok(Self::Lib),
            "syslib" => Ok(Self::Syslib),
            other => Err(miette!(
                "`[cone].kind` 必须是 `bin`、`lib` 或 `syslib`，但得到 `{other}`"
            )),
        }
    }

    /// Return the canonical manifest spelling for this cone kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bin => "bin",
            Self::Lib => "lib",
            Self::Syslib => "syslib",
        }
    }
}

impl std::fmt::Display for ConeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `[cone]` 段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeSection {
    pub name: String,
    pub version: String,
    pub kind: ConeKind,
}

/// `[[select]]`：平台选择器的 `when` 条件（spec §13.9）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeSelectWhen {
    pub platform: String,
}

/// `[[select]]`：平台选择器条目（spec §13.9）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeSelectEntry {
    pub when: ConeSelectWhen,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// `[native-build]`：生成最终可执行文件时使用的工程化配置。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConeNativeBuildConfig {
    /// 入口包名（期望该包定义 `fun main`；校验由后续 native build 接入实现）。
    pub entry_package: Option<String>,
    /// 优化等级（对齐 `scoop build/run -O` 与 profile 默认策略）。
    pub opt_level: Option<OptLevel>,
    /// 额外 C 源文件（相对 cone root）。
    pub c_sources: Vec<String>,
    /// 仅作用于 `c_sources` 的编译参数。
    pub c_flags: Vec<String>,
    /// 额外 C++ 源文件（相对 cone root）。
    pub cxx_sources: Vec<String>,
    /// 仅作用于 `cxx_sources` 的编译参数。
    pub cxx_flags: Vec<String>,
    /// linker 可执行文件（语义：指定 linker 程序；默认由 toolchain 选择）。
    pub linker: Option<String>,
    /// 额外链接参数（追加到最终链接命令；不替代 `linker`）。
    pub link_flags: Vec<String>,
}

/// `[dependencies]` 中的单个依赖声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConeDependencySpec {
    /// 旧 manifest parser 保留的版本要求；source-only active path 暂不解析 registry 依赖。
    Version(String),
    /// 本地 source cone 依赖，路径相对当前 cone root。
    LocalPath { path: String },
}

impl ConeDependencySpec {
    /// Return the version requirement if this dependency uses registry syntax.
    pub fn version_req(&self) -> Option<&str> {
        match self {
            Self::Version(req) => Some(req),
            Self::LocalPath { .. } => None,
        }
    }

    /// Return the relative local path if this dependency uses source path syntax.
    pub fn local_path(&self) -> Option<&str> {
        match self {
            Self::Version(_) => None,
            Self::LocalPath { path } => Some(path),
        }
    }
}

/// `Cone.toml` 的最小可用 manifest。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeManifest {
    pub cone: ConeSection,
    /// 依赖（cone name → dependency spec）。
    pub dependencies: BTreeMap<String, ConeDependencySpec>,
    /// pre-specialize：需要预编译的“常用单态化实例”。
    pub pre_specialize_functions: Vec<String>,
    /// pre-specialize：类型实例。
    pub pre_specialize_types: Vec<String>,
    /// program boundary：库导出入口（host/embedded entry points）。
    pub export_entry_points: Vec<String>,
    /// 平台选择器（`[[select]]`，spec §13.9）。
    pub selectors: Vec<ConeSelectEntry>,
    /// native build 配置。
    pub native_build: ConeNativeBuildConfig,
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
        let kind =
            ConeKind::parse(get_required_string(cone, "kind").wrap_err("读取 `[cone].kind` 失败")?)
                .wrap_err("读取 `[cone].kind` 失败")?;

        let dependencies = parse_dependencies(root)?;
        let export_entry_points = parse_export_entry_points(root)?;
        let (pre_specialize_functions, pre_specialize_types) = parse_pre_specialize(root)?;
        let selectors = parse_selectors(root)?;
        let native_build = parse_native_build(root)?;

        Ok(Self {
            cone: ConeSection {
                name,
                version,
                kind,
            },
            dependencies,
            pre_specialize_functions,
            pre_specialize_types,
            export_entry_points,
            selectors,
            native_build,
        })
    }
}

fn get_required_string<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .ok_or_else(|| miette!("缺少字段 `{key}`"))?
        .as_str()
        .ok_or_else(|| miette!("字段 `{key}` 必须是字符串"))
}

fn parse_dependencies(root: &toml::Table) -> Result<BTreeMap<String, ConeDependencySpec>> {
    let Some(value) = root.get("dependencies") else {
        return Ok(BTreeMap::new());
    };
    let table = value
        .as_table()
        .ok_or_else(|| miette!("`[dependencies]` 必须是 table"))?;

    let mut out = BTreeMap::new();
    for (dep_name, dep_value) in table {
        let spec = parse_dependency_spec(dep_name, dep_value)?;
        out.insert(dep_name.to_owned(), spec);
    }
    Ok(out)
}

fn parse_dependency_spec(dep_name: &str, value: &toml::Value) -> Result<ConeDependencySpec> {
    if let Some(req) = value.as_str() {
        return Ok(ConeDependencySpec::Version(req.to_owned()));
    }

    let Some(table) = value.as_table() else {
        return Err(miette!(
            "`[dependencies].{dep_name}` 必须是字符串版本要求或 inline table（例如 {{ path = \"../dep\" }}）"
        ));
    };

    let path = get_required_string(table, "path")
        .wrap_err_with(|| format!("读取 `[dependencies].{dep_name}.path` 失败"))?;
    let path = normalize_local_dependency_path(path)
        .wrap_err_with(|| format!("解析 `[dependencies].{dep_name}.path` 失败"))?;

    Ok(ConeDependencySpec::LocalPath { path })
}

fn normalize_local_dependency_path(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(miette!("路径不能为空"));
    }

    let value = value.replace('\\', "/");
    if value.starts_with('/') {
        return Err(miette!("本地 dependency path 必须是相对路径：{value}"));
    }
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            return Err(miette!("本地 dependency path 不允许使用盘符路径：{value}"));
        }
    }

    let mut parts = Vec::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(miette!("路径不能为空"));
    }

    Ok(parts.join("/"))
}

fn parse_export_entry_points(root: &toml::Table) -> Result<Vec<String>> {
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

fn parse_pre_specialize(root: &toml::Table) -> Result<(Vec<String>, Vec<String>)> {
    let table = match root
        .get("pre-specialize")
        .or_else(|| root.get("pre_specialize"))
    {
        None => return Ok((Vec::new(), Vec::new())),
        Some(value) => value
            .as_table()
            .ok_or_else(|| miette!("`[pre-specialize]` 必须是 table"))?,
    };

    fn parse_string_array(table: &toml::Table, key: &str, section: &str) -> Result<Vec<String>> {
        let Some(value) = table.get(key) else {
            return Ok(Vec::new());
        };
        let arr = value
            .as_array()
            .ok_or_else(|| miette!("`[{section}].{key}` 必须是字符串数组"))?;

        let mut out = Vec::with_capacity(arr.len());
        for (idx, item) in arr.iter().enumerate() {
            let Some(s) = item.as_str() else {
                return Err(miette!("`[{section}].{key}[{idx}]` 必须是字符串"));
            };
            out.push(s.to_owned());
        }
        Ok(out)
    }

    let mut functions = parse_string_array(table, "functions", "pre-specialize")?;
    if functions.is_empty() {
        functions = parse_string_array(table, "funs", "pre-specialize")?;
    }

    let types = parse_string_array(table, "types", "pre-specialize")?;

    Ok((functions, types))
}

fn parse_native_build(root: &toml::Table) -> Result<ConeNativeBuildConfig> {
    let table = match root
        .get("native-build")
        .or_else(|| root.get("native_build"))
    {
        None => return Ok(ConeNativeBuildConfig::default()),
        Some(value) => value
            .as_table()
            .ok_or_else(|| miette!("`[native-build]` 必须是 table"))?,
    };

    fn get_optional_string(
        table: &toml::Table,
        key: &str,
        alt_key: &str,
        section: &str,
    ) -> Result<Option<String>> {
        let value = table.get(key).or_else(|| table.get(alt_key));
        let Some(value) = value else {
            return Ok(None);
        };

        let s = value
            .as_str()
            .ok_or_else(|| miette!("`[{section}].{key}` 必须是字符串"))?;
        Ok(Some(s.to_owned()))
    }

    fn parse_optional_opt_level(
        table: &toml::Table,
        key: &str,
        alt_key: &str,
        section: &str,
    ) -> Result<Option<OptLevel>> {
        let value = table.get(key).or_else(|| table.get(alt_key));
        let Some(value) = value else {
            return Ok(None);
        };

        if let Some(v) = value.as_integer() {
            let parsed =
                OptLevel::from_i64(v).wrap_err_with(|| format!("解析 `[{section}].{key}` 失败"))?;
            return Ok(Some(parsed));
        }

        if let Some(v) = value.as_str() {
            let parsed =
                OptLevel::parse(v).wrap_err_with(|| format!("解析 `[{section}].{key}` 失败"))?;
            return Ok(Some(parsed));
        }

        Err(miette!(
            "`[{section}].{key}` 必须是字符串或整数（例如 2 / \"2\" / \"s\"）"
        ))
    }

    fn parse_optional_string_array(
        table: &toml::Table,
        key: &str,
        alt_key: &str,
        section: &str,
    ) -> Result<Vec<String>> {
        let value = table.get(key).or_else(|| table.get(alt_key));
        let Some(value) = value else {
            return Ok(Vec::new());
        };

        let arr = value
            .as_array()
            .ok_or_else(|| miette!("`[{section}].{key}` 必须是字符串数组"))?;

        let mut out = Vec::with_capacity(arr.len());
        for (idx, item) in arr.iter().enumerate() {
            let Some(s) = item.as_str() else {
                return Err(miette!("`[{section}].{key}[{idx}]` 必须是字符串"));
            };
            out.push(s.to_owned());
        }
        Ok(out)
    }

    fn normalize_rel_path_forward_slashes(value: &str) -> Result<String> {
        let value = value.trim();
        if value.is_empty() {
            return Err(miette!("路径不能为空"));
        }

        let value = value.replace('\\', "/");
        if value.starts_with('/') {
            return Err(miette!(
                "路径必须是相对 cone root 的相对路径（不允许绝对路径）：{value}"
            ));
        }

        if value.len() >= 2 {
            let bytes = value.as_bytes();
            if bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
                return Err(miette!(
                    "路径必须是相对 cone root 的相对路径（不允许盘符路径）：{value}"
                ));
            }
        }

        let mut parts = Vec::<&str>::new();
        for part in value.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                return Err(miette!("路径不允许包含 `..`：{value}"));
            }
            parts.push(part);
        }

        if parts.is_empty() {
            return Err(miette!("路径不能为空"));
        }

        Ok(parts.join("/"))
    }

    fn parse_optional_rel_path_array(
        table: &toml::Table,
        key: &str,
        alt_key: &str,
        section: &str,
    ) -> Result<Vec<String>> {
        let value = table.get(key).or_else(|| table.get(alt_key));
        let Some(value) = value else {
            return Ok(Vec::new());
        };

        let arr = value
            .as_array()
            .ok_or_else(|| miette!("`[{section}].{key}` 必须是字符串数组"))?;

        let mut out = Vec::with_capacity(arr.len());
        for (idx, item) in arr.iter().enumerate() {
            let Some(s) = item.as_str() else {
                return Err(miette!(
                    "`[{section}].{key}[{idx}]` 必须是字符串（相对 cone root 的路径）"
                ));
            };

            let normalized = normalize_rel_path_forward_slashes(s)
                .wrap_err_with(|| format!("解析 `[{section}].{key}[{idx}]` 失败"))?;
            out.push(normalized);
        }
        Ok(out)
    }

    let entry_package =
        get_optional_string(table, "entry-package", "entry_package", "native-build")?;
    let opt_level = parse_optional_opt_level(table, "opt-level", "opt_level", "native-build")?;
    let c_sources = parse_optional_rel_path_array(table, "c-sources", "c_sources", "native-build")?;
    let c_flags = parse_optional_string_array(table, "c-flags", "c_flags", "native-build")?;
    let cxx_sources =
        parse_optional_rel_path_array(table, "cxx-sources", "cxx_sources", "native-build")?;
    let cxx_flags = parse_optional_string_array(table, "cxx-flags", "cxx_flags", "native-build")?;
    let linker = get_optional_string(table, "linker", "linker", "native-build")?;
    let link_flags =
        parse_optional_string_array(table, "link-flags", "link_flags", "native-build")?;

    Ok(ConeNativeBuildConfig {
        entry_package,
        opt_level,
        c_sources,
        c_flags,
        cxx_sources,
        cxx_flags,
        linker,
        link_flags,
    })
}

fn parse_selectors(root: &toml::Table) -> Result<Vec<ConeSelectEntry>> {
    let Some(value) = root.get("select") else {
        return Ok(Vec::new());
    };

    let arr = value
        .as_array()
        .ok_or_else(|| miette!("`[[select]]` 必须是 array（table array）"))?;

    fn parse_optional_string_array(
        table: &toml::Table,
        key: &str,
        section: &str,
        idx: usize,
    ) -> Result<Vec<String>> {
        let Some(value) = table.get(key) else {
            return Ok(Vec::new());
        };

        let arr = value
            .as_array()
            .ok_or_else(|| miette!("`{section}[{idx}].{key}` 必须是字符串数组"))?;

        let mut out = Vec::with_capacity(arr.len());
        for (item_idx, item) in arr.iter().enumerate() {
            let Some(s) = item.as_str() else {
                return Err(miette!("`{section}[{idx}].{key}[{item_idx}]` 必须是字符串"));
            };
            out.push(s.to_owned());
        }
        Ok(out)
    }

    let mut out = Vec::with_capacity(arr.len());
    for (idx, item) in arr.iter().enumerate() {
        let Some(table) = item.as_table() else {
            return Err(miette!("`select[{idx}]` 必须是 table（`[[select]]` 条目）"));
        };

        let when = table
            .get("when")
            .and_then(|value| value.as_table())
            .ok_or_else(|| miette!("`select[{idx}].when` 必须是 inline table / table"))?;

        let platform = get_required_string(when, "platform")
            .wrap_err_with(|| format!("读取 `select[{idx}].when.platform` 失败"))?
            .to_owned();

        let include = parse_optional_string_array(table, "include", "select", idx)?;
        let exclude = parse_optional_string_array(table, "exclude", "select", idx)?;

        out.push(ConeSelectEntry {
            when: ConeSelectWhen { platform },
            include,
            exclude,
        });
    }

    Ok(out)
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
kind = "bin"

[dependencies]
scoop-core = "1.0.0"
scoop-io = "1.2.0"
"#,
        )
        .unwrap();

        assert_eq!(manifest.cone.name, "scoop-http");
        assert_eq!(manifest.cone.version, "2.1.0");
        assert_eq!(manifest.cone.kind, ConeKind::Bin);
        assert_eq!(
            manifest
                .dependencies
                .get("scoop-core")
                .and_then(ConeDependencySpec::version_req),
            Some("1.0.0")
        );
        assert_eq!(
            manifest
                .dependencies
                .get("scoop-io")
                .and_then(ConeDependencySpec::version_req),
            Some("1.2.0")
        );
        assert!(manifest.pre_specialize_functions.is_empty());
        assert!(manifest.pre_specialize_types.is_empty());
        assert!(manifest.export_entry_points.is_empty());
        assert!(manifest.selectors.is_empty());
        assert_eq!(manifest.native_build, ConeNativeBuildConfig::default());
    }

    #[test]
    fn parse_local_path_dependency_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture.app"
version = "0.0.0"
kind = "bin"

[dependencies]
"fixture.lib" = { path = "../fixture.lib" }
"fixture.util" = { path = "./deps//util" }
"#,
        )
        .unwrap();

        assert_eq!(
            manifest
                .dependencies
                .get("fixture.lib")
                .and_then(ConeDependencySpec::local_path),
            Some("../fixture.lib")
        );
        assert_eq!(
            manifest
                .dependencies
                .get("fixture.util")
                .and_then(ConeDependencySpec::local_path),
            Some("deps/util")
        );
    }

    #[test]
    fn parse_local_path_dependency_rejects_absolute_path() {
        let err = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture.app"
version = "0.0.0"
kind = "bin"

[dependencies]
"fixture.lib" = { path = "/tmp/fixture.lib" }
"#,
        )
        .unwrap_err();

        let text = format!("{err:?}");
        assert!(text.contains("[dependencies].fixture.lib.path"));
        assert!(text.contains("相对路径"));
    }

    #[test]
    fn parse_cone_kind_variants_ok() {
        for (kind_text, expected) in [
            ("bin", ConeKind::Bin),
            ("lib", ConeKind::Lib),
            ("syslib", ConeKind::Syslib),
        ] {
            let manifest = ConeManifest::parse_str(&format!(
                r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "{kind_text}"
"#
            ))
            .unwrap();

            assert_eq!(manifest.cone.kind, expected);
        }
    }

    #[test]
    fn parse_cone_kind_rejects_invalid_kind() {
        let err = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "plugin"
"#,
        )
        .unwrap_err();

        let text = format!("{err:?}");
        assert!(text.contains("`[cone].kind`"));
        assert!(text.contains("plugin"));
    }

    #[test]
    fn parse_cone_kind_is_required() {
        let err = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
"#,
        )
        .unwrap_err();

        let text = format!("{err:?}");
        assert!(text.contains("读取 `[cone].kind` 失败"));
        assert!(text.contains("缺少字段 `kind`"));
    }

    #[test]
    fn parse_entry_points_exports_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "bin"

[entry-points]
exports = ["a.b.init", "a.b.entry"]
"#,
        )
        .unwrap();

        assert_eq!(manifest.export_entry_points, vec!["a.b.init", "a.b.entry"]);
    }

    #[test]
    fn parse_pre_specialize_functions_and_types_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "bin"

[pre-specialize]
functions = ["a.b.id<Int>", "a.b.id<String>"]
types = ["a.b.List<Int>"]
"#,
        )
        .unwrap();

        assert_eq!(
            manifest.pre_specialize_functions,
            vec!["a.b.id<Int>", "a.b.id<String>"]
        );
        assert_eq!(manifest.pre_specialize_types, vec!["a.b.List<Int>"]);
        assert!(manifest.selectors.is_empty());
    }

    #[test]
    fn parse_native_build_config_kebab_case_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "bin"

[native-build]
entry-package = "my.app"
opt-level = "2"
c-sources = ["native\\foo.c", "./native/bar.c"]
c-flags = ["-O2", "-DSCOOP=1"]
cxx-sources = ["native/baz.cc"]
cxx-flags = ["-std=c++20"]
linker = "clang"
link-flags = ["-Wl,-dead_strip"]
"#,
        )
        .unwrap();

        assert_eq!(
            manifest.native_build.entry_package.as_deref(),
            Some("my.app")
        );
        assert_eq!(manifest.native_build.opt_level, Some(OptLevel::O2));
        assert_eq!(
            manifest.native_build.c_sources,
            vec!["native/foo.c", "native/bar.c"]
        );
        assert_eq!(manifest.native_build.c_flags, vec!["-O2", "-DSCOOP=1"]);
        assert_eq!(manifest.native_build.cxx_sources, vec!["native/baz.cc"]);
        assert_eq!(manifest.native_build.cxx_flags, vec!["-std=c++20"]);
        assert_eq!(manifest.native_build.linker.as_deref(), Some("clang"));
        assert_eq!(manifest.native_build.link_flags, vec!["-Wl,-dead_strip"]);
    }

    #[test]
    fn parse_native_build_config_snake_case_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "bin"

[native_build]
entry_package = "my.app"
opt_level = 0
c_sources = ["native/foo.c"]
c_flags = ["-O2"]
cxx_sources = ["native/baz.cc"]
cxx_flags = ["-std=c++20"]
linker = "clang"
link_flags = ["-Wl,-dead_strip"]
"#,
        )
        .unwrap();

        assert_eq!(
            manifest.native_build.entry_package.as_deref(),
            Some("my.app")
        );
        assert_eq!(manifest.native_build.opt_level, Some(OptLevel::O0));
        assert_eq!(manifest.native_build.c_sources, vec!["native/foo.c"]);
        assert_eq!(manifest.native_build.c_flags, vec!["-O2"]);
        assert_eq!(manifest.native_build.cxx_sources, vec!["native/baz.cc"]);
        assert_eq!(manifest.native_build.cxx_flags, vec!["-std=c++20"]);
        assert_eq!(manifest.native_build.linker.as_deref(), Some("clang"));
        assert_eq!(manifest.native_build.link_flags, vec!["-Wl,-dead_strip"]);
    }

    #[test]
    fn parse_selectors_ok() {
        let manifest = ConeManifest::parse_str(
            r#"
[cone]
name = "fixture"
version = "0.0.0"
kind = "bin"

[[select]]
when = { platform = "linux-x64" }
include = ["src/platform/posix/**.scoop"]
exclude = ["src/platform/windows/**.scoop"]

[[select]]
when = { platform = "windows-x64" }
include = ["src/platform/windows/**.scoop"]
exclude = ["src/platform/posix/**.scoop"]
"#,
        )
        .unwrap();

        assert_eq!(
            manifest.selectors,
            vec![
                ConeSelectEntry {
                    when: ConeSelectWhen {
                        platform: "linux-x64".to_owned(),
                    },
                    include: vec!["src/platform/posix/**.scoop".to_owned()],
                    exclude: vec!["src/platform/windows/**.scoop".to_owned()],
                },
                ConeSelectEntry {
                    when: ConeSelectWhen {
                        platform: "windows-x64".to_owned(),
                    },
                    include: vec!["src/platform/windows/**.scoop".to_owned()],
                    exclude: vec!["src/platform/posix/**.scoop".to_owned()],
                },
            ]
        );
    }
}
