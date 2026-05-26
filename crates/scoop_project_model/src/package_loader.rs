//! Filesystem loader for source cone packages.
//!
//! 本模块负责“目录结构 → 路径列表”的确定性文件系统规则；
//! 纯 [`ConeSourcePackage`] 数据定义由 [`crate::package`] 持有。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use crate::manifest::{CONE_TOML_FILE_NAME, ConeKind, ConeManifest};
use crate::manifest_loader::load_cone_manifest_from_path;
use crate::package::{CONE_MAIN_FILE_NAME, CONE_SRC_DIR_NAME, ConeSourcePackage};
use crate::sysroot::default_sysroot_path;

/// 从一个 cone root 目录加载“源级包”的 sources 列表与可选 entry anchor。
///
/// 当前规则：
/// - `Cone.toml` 必须位于 root 目录下；
/// - `kind = "syslib"` 只允许出现在 `<sysroot>/lib/<cone.name>/Cone.toml`；
/// - sources = `root/src/**/*.scoop`（递归，且非空）；
/// - `bin` cone 必须有 `root/src/main.scoop`；
/// - `lib/syslib` cone 不要求 `root/src/main.scoop`，即使存在也只是普通 source。
///
/// 平台选择器（spec §13.9）：
/// - 初始 source set 仍为 `src/**/*.scoop`；
/// - 当 `Cone.toml` 含 `[[select]]` 且 `when.platform` 匹配当前 target platform 时：
///   - include globs：把匹配到的文件加入 source set；
///   - exclude globs：把匹配到的文件从 source set 移除；
/// - 多个匹配的 selector 按文件顺序依次应用。
pub fn load_cone_source_package(root: impl AsRef<Path>) -> Result<ConeSourcePackage> {
    let platform = host_target_platform_id();
    load_cone_source_package_for_platform(root, &platform)
}

/// 加载 cone source package，但允许显式指定“目标平台 id”（spec §13.9）。
pub fn load_cone_source_package_for_platform(
    root: impl AsRef<Path>,
    target_platform: &str,
) -> Result<ConeSourcePackage> {
    load_cone_source_package_for_platform_with_sysroot_root(
        root,
        target_platform,
        &default_sysroot_path(),
    )
}

pub fn load_cone_source_package_for_platform_with_sysroot_root(
    root: impl AsRef<Path>,
    target_platform: &str,
    sysroot_root: &Path,
) -> Result<ConeSourcePackage> {
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
    let manifest = load_cone_manifest_from_path(&manifest_path)?;
    validate_syslib_package_path(&root, &manifest, sysroot_root)?;

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

    let main = if manifest.cone.kind == ConeKind::Bin {
        let main = src_root.join(CONE_MAIN_FILE_NAME);
        if !main.is_file() {
            return Err(miette!(
                "`bin` cone package 缺少入口文件 `{CONE_MAIN_FILE_NAME}`：{}",
                main.display()
            ));
        }
        Some(
            main.canonicalize()
                .into_diagnostic()
                .wrap_err_with(|| format!("无法定位 main.scoop：{}", main.display()))?,
        )
    } else {
        None
    };

    let mut sources = Vec::new();
    collect_scoop_files(&src_root, &mut sources)?;

    if sources.is_empty() {
        return Err(miette!(
            "cone package 的 src 目录下没有任何 `.scoop` 文件：{}",
            src_root.display()
        ));
    }

    let mut canonical_sources = Vec::with_capacity(sources.len());
    for path in sources {
        let path = path
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| format!("无法定位源文件：{}", path.display()))?;
        canonical_sources.push(path);
    }
    canonical_sources.sort();

    if let Some(main) = main.as_ref()
        && !canonical_sources.iter().any(|p| p == main)
    {
        canonical_sources.push(main.clone());
        canonical_sources.sort();
    }

    let all_sources = build_source_map(&root, &canonical_sources)?;
    let selected_sources = apply_platform_selectors(
        &all_sources,
        &manifest,
        target_platform,
        &root,
        main.as_deref(),
    )?;

    Ok(ConeSourcePackage {
        root,
        manifest_path,
        manifest,
        src_root,
        sources: selected_sources,
        main,
    })
}

pub(crate) fn validate_syslib_package_path(
    root: &Path,
    manifest: &ConeManifest,
    sysroot_root: &Path,
) -> Result<()> {
    if manifest.cone.kind != ConeKind::Syslib {
        return Ok(());
    }

    let sysroot_root = sysroot_root
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法定位 sysroot 目录：{}", sysroot_root.display()))?;
    let expected_root = sysroot_root.join("lib").join(&manifest.cone.name);
    let expected_for_compare = expected_root
        .canonicalize()
        .unwrap_or_else(|_| expected_root.clone());
    if root == expected_for_compare {
        return Ok(());
    }

    Err(miette!(
        "`syslib` cone 只能位于 `sysroot/lib/<cone.fqn>/` 下：`{}` 声明为 `syslib`，但当前 root 是 `{}`，期望 `{}`",
        manifest.cone.name,
        root.display(),
        expected_root.display()
    ))
}

pub(crate) fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
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

fn build_source_map(
    root: &Path,
    canonical_sources: &[PathBuf],
) -> Result<BTreeMap<String, PathBuf>> {
    let mut out = BTreeMap::new();
    for path in canonical_sources {
        let rel = normalize_rel_path_forward_slashes(root, path)?;
        out.insert(rel, path.clone());
    }
    Ok(out)
}

pub(crate) fn normalize_rel_path_forward_slashes(root: &Path, abs: &Path) -> Result<String> {
    let rel = abs.strip_prefix(root).map_err(|_| {
        miette!(
            "源文件不在 cone root 下（root={}，path={}）",
            root.display(),
            abs.display()
        )
    })?;

    let Some(rel_str) = rel.to_str() else {
        return Err(miette!("源文件路径不是有效 UTF-8：{}", rel.display()));
    };

    Ok(rel_str.replace('\\', "/"))
}

fn apply_platform_selectors(
    all_sources: &BTreeMap<String, PathBuf>,
    manifest: &ConeManifest,
    target_platform: &str,
    root: &Path,
    main: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let mut selected = all_sources.clone();

    for selector in &manifest.selectors {
        if selector.when.platform != target_platform {
            continue;
        }

        for pattern in &selector.include {
            let pattern = pattern.replace('\\', "/");
            for (rel, path) in all_sources.iter() {
                if glob_match_forward_slashes(&pattern, rel) {
                    selected.insert(rel.clone(), path.clone());
                }
            }
        }

        for pattern in &selector.exclude {
            let pattern = pattern.replace('\\', "/");
            let to_remove: Vec<String> = selected
                .keys()
                .filter(|rel| glob_match_forward_slashes(&pattern, rel))
                .cloned()
                .collect();
            for rel in to_remove {
                selected.remove(&rel);
            }
        }
    }

    if let Some(main) = main {
        let main_rel = normalize_rel_path_forward_slashes(root, main)?;
        if !selected.contains_key(&main_rel) {
            return Err(miette!(
                "platform selector 将入口文件从 sources 中移除了（platform={target_platform}，main={main_rel}）"
            ));
        }
    }

    if selected.is_empty() {
        return Err(miette!(
            "platform selector 处理后 sources 为空（platform={target_platform}，cone={}）",
            root.display()
        ));
    }

    Ok(selected.into_values().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlobToken {
    DoubleStar,
    Star,
    Question,
    Literal(char),
}

/// 一个“稳定且平台无关”的最小 glob matcher（spec §13.9）。
fn glob_match_forward_slashes(pattern: &str, path: &str) -> bool {
    let tokens = tokenize_glob(pattern);
    let text: Vec<char> = path.chars().collect();

    let mut memo = vec![vec![None; text.len() + 1]; tokens.len() + 1];

    fn dp(
        i: usize,
        j: usize,
        tokens: &[GlobToken],
        text: &[char],
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(v) = memo[i][j] {
            return v;
        }

        let v = if i == tokens.len() {
            j == text.len()
        } else {
            match tokens[i] {
                GlobToken::Literal(c) => {
                    j < text.len() && text[j] == c && dp(i + 1, j + 1, tokens, text, memo)
                }
                GlobToken::Question => {
                    j < text.len() && text[j] != '/' && dp(i + 1, j + 1, tokens, text, memo)
                }
                GlobToken::Star => {
                    let mut k = j;
                    loop {
                        if dp(i + 1, k, tokens, text, memo) {
                            break true;
                        }
                        if k == text.len() || text[k] == '/' {
                            break false;
                        }
                        k += 1;
                    }
                }
                GlobToken::DoubleStar => {
                    let mut k = j;
                    loop {
                        if dp(i + 1, k, tokens, text, memo) {
                            break true;
                        }
                        if k == text.len() {
                            break false;
                        }
                        k += 1;
                    }
                }
            }
        };

        memo[i][j] = Some(v);
        v
    }

    dp(0, 0, &tokens, &text, &mut memo)
}

fn tokenize_glob(pattern: &str) -> Vec<GlobToken> {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    tokens.push(GlobToken::DoubleStar);
                } else {
                    tokens.push(GlobToken::Star);
                }
            }
            '?' => tokens.push(GlobToken::Question),
            other => tokens.push(GlobToken::Literal(other)),
        }
    }
    tokens
}

/// 生成当前“目标平台 id”（用于 `Cone.toml [[select]]` 的匹配）。
pub fn host_target_platform_id() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "i686" => "x86",
        other => other,
    };

    format!("{os}-{arch}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/testdata/cone/{name}"))
    }

    fn rel_sources(pkg: &ConeSourcePackage) -> Vec<String> {
        pkg.sources
            .iter()
            .map(|p| normalize_rel_path_forward_slashes(&pkg.root, p).unwrap())
            .collect()
    }

    struct TempCone {
        root: PathBuf,
        cleanup_root: PathBuf,
        sysroot_root: Option<PathBuf>,
    }

    impl TempCone {
        fn new(label: &str) -> Self {
            let base = temp_base(label);
            std::fs::create_dir_all(base.join(CONE_SRC_DIR_NAME)).unwrap();
            Self {
                root: base.clone(),
                cleanup_root: base,
                sysroot_root: None,
            }
        }

        fn new_sysroot_lib(label: &str, cone_name: &str) -> Self {
            let base = temp_base(label);
            let sysroot_root = base.join("sysroot");
            let root = sysroot_root.join("lib").join(cone_name);
            std::fs::create_dir_all(root.join(CONE_SRC_DIR_NAME)).unwrap();
            Self {
                root,
                cleanup_root: base,
                sysroot_root: Some(sysroot_root),
            }
        }

        fn write_manifest(&self, kind: ConeKind, extra: &str) {
            self.write_manifest_named(&format!("fixture-{}", kind.as_str()), kind, extra);
        }

        fn write_manifest_named(&self, name: &str, kind: ConeKind, extra: &str) {
            std::fs::write(
                self.root.join(CONE_TOML_FILE_NAME),
                format!(
                    "[cone]\nname = \"{}\"\nversion = \"0.0.0\"\nkind = \"{}\"\n{}",
                    name,
                    kind.as_str(),
                    extra
                ),
            )
            .unwrap();
        }

        fn write_source(&self, rel: &str, text: &str) {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, text).unwrap();
        }
    }

    fn temp_base(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "scoopc_cone_package_loader_{label}_{}_{}",
            std::process::id(),
            unique
        ))
    }

    impl Drop for TempCone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.cleanup_root);
        }
    }

    #[test]
    fn lib_cone_without_main_loads_sources_ok() {
        let cone = TempCone::new("lib_without_main");
        cone.write_manifest(ConeKind::Lib, "");
        cone.write_source(
            "src/api.scoop",
            "package fixture.lib\npublic fun value(): Int = 1\n",
        );

        let pkg = load_cone_source_package(&cone.root).unwrap();

        assert_eq!(pkg.manifest.cone.kind, ConeKind::Lib);
        assert!(pkg.main.is_none());
        assert_eq!(rel_sources(&pkg), vec!["src/api.scoop"]);
    }

    #[test]
    fn syslib_cone_without_main_loads_sources_ok() {
        let cone = TempCone::new_sysroot_lib("syslib_without_main", "fixture-syslib");
        cone.write_manifest_named("fixture-syslib", ConeKind::Syslib, "");
        cone.write_source(
            "src/sys.scoop",
            "package fixture.syslib\npublic fun token(): Int = 1\n",
        );

        let pkg = load_cone_source_package_for_platform_with_sysroot_root(
            &cone.root,
            &host_target_platform_id(),
            cone.sysroot_root.as_deref().unwrap(),
        )
        .unwrap();

        assert_eq!(pkg.manifest.cone.kind, ConeKind::Syslib);
        assert!(pkg.main.is_none());
        assert_eq!(rel_sources(&pkg), vec!["src/sys.scoop"]);
    }

    #[test]
    fn user_path_syslib_cone_is_rejected() {
        let cone = TempCone::new("user_syslib_rejected");
        cone.write_manifest_named("fixture-user-syslib", ConeKind::Syslib, "");
        cone.write_source(
            "src/sys.scoop",
            "package fixture.syslib\npublic fun token(): Int = 1\n",
        );

        let err = load_cone_source_package(&cone.root)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("`syslib` cone 只能位于 `sysroot/lib/<cone.fqn>/` 下"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn bin_cone_without_main_is_error() {
        let cone = TempCone::new("bin_without_main");
        cone.write_manifest(ConeKind::Bin, "");
        cone.write_source("src/util.scoop", "package fixture.bin\nfun helper() {}\n");

        let err = load_cone_source_package(&cone.root)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("`bin` cone package 缺少入口文件"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn lib_main_named_file_is_not_entry_anchor() {
        let cone = TempCone::new("lib_main_named_file");
        cone.write_manifest(
            ConeKind::Lib,
            r#"
[[select]]
when = { platform = "test-platform" }
exclude = ["src/main.scoop"]
"#,
        );
        cone.write_source("src/api.scoop", "package fixture.lib\nfun api() {}\n");
        cone.write_source("src/main.scoop", "package fixture.lib\nfun main() {}\n");

        let pkg = load_cone_source_package_for_platform(&cone.root, "test-platform").unwrap();

        assert!(pkg.main.is_none());
        assert_eq!(rel_sources(&pkg), vec!["src/api.scoop"]);
    }

    #[test]
    fn bin_selector_cannot_remove_main_anchor() {
        let cone = TempCone::new("bin_selector_removes_main");
        cone.write_manifest(
            ConeKind::Bin,
            r#"
[[select]]
when = { platform = "test-platform" }
exclude = ["src/main.scoop"]
"#,
        );
        cone.write_source("src/api.scoop", "package fixture.bin\nfun api() {}\n");
        cone.write_source("src/main.scoop", "package fixture.bin\nfun main() {}\n");

        let err = load_cone_source_package_for_platform(&cone.root, "test-platform")
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("platform selector 将入口文件从 sources 中移除了"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn platform_selectors_apply_in_file_order_ok() {
        let dir = fixture_dir("selectors_basic");

        let linux = load_cone_source_package_for_platform(&dir, "linux-x64").unwrap();
        assert_eq!(
            rel_sources(&linux),
            vec![
                "src/common.scoop",
                "src/main.scoop",
                "src/platform/posix/posix.scoop",
            ]
        );

        let windows = load_cone_source_package_for_platform(&dir, "windows-x64").unwrap();
        assert_eq!(
            rel_sources(&windows),
            vec![
                "src/common.scoop",
                "src/main.scoop",
                "src/platform/windows/win.scoop",
            ]
        );

        let unknown = load_cone_source_package_for_platform(&dir, "unknown-unknown").unwrap();
        assert_eq!(
            rel_sources(&unknown),
            vec![
                "src/common.scoop",
                "src/main.scoop",
                "src/platform/posix/posix.scoop",
                "src/platform/windows/win.scoop",
            ]
        );
    }

    #[test]
    fn glob_match_minimal_semantics_ok() {
        assert!(glob_match_forward_slashes("src/**.scoop", "src/main.scoop"));
        assert!(glob_match_forward_slashes(
            "src/platform/posix/**.scoop",
            "src/platform/posix/a/b/posix.scoop"
        ));
        assert!(!glob_match_forward_slashes(
            "src/platform/posix/*.scoop",
            "src/platform/posix/a/posix.scoop"
        ));
        assert!(glob_match_forward_slashes(
            "src/platform/posix/?osix.scoop",
            "src/platform/posix/posix.scoop"
        ));
        assert!(!glob_match_forward_slashes(
            "src/platform/posix/?osix.scoop",
            "src/platform/posix/xxosix.scoop"
        ));
    }
}
