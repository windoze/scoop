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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miette::{miette, Context as _, IntoDiagnostic as _, Result};

use super::manifest::{ConeManifest, CONE_TOML_FILE_NAME};

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
///
/// 平台选择器（T1111，spec §13.9）：
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
///
/// 说明：
/// - 用于让 selector 的 include/exclude 规则可单测；
/// - 也为未来交叉编译（显式 target 参数）预留扩展点；
/// - v0 driver 默认用 host platform（见 `load_cone_source_package`）。
pub fn load_cone_source_package_for_platform(
    root: impl AsRef<Path>,
    target_platform: &str,
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

    let all_sources = build_source_map(&root, &canonical_sources)?;
    let selected_sources =
        apply_platform_selectors(&all_sources, &manifest, target_platform, &root, &main)?;

    Ok(ConeSourcePackage {
        root,
        manifest_path,
        manifest,
        src_root,
        sources: selected_sources,
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

/// 将 canonical sources 转为 “相对路径（UTF-8 + forward slashes）→ 绝对路径（canonicalize）” 的稳定映射。
///
/// 选择 `BTreeMap` 的原因：
/// - key 的排序就是最终 sources 列表的排序；
/// - key 使用平台无关的相对路径，避免绝对路径导致排序跨机器不稳定。
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

/// 归一化为相对于 cone root 的 UTF-8 路径，并统一使用 forward slashes（spec §13.9）。
fn normalize_rel_path_forward_slashes(root: &Path, abs: &Path) -> Result<String> {
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

    // 规范要求路径是“platform-independent（forward slashes）”，这里做一次显式归一化：
    // - Unix: 原样
    // - Windows: `\\` → `/`
    Ok(rel_str.replace('\\', "/"))
}

/// 在默认 source discovery 的基础上，应用 `[[select]]` 的 include/exclude 规则（spec §13.9）。
fn apply_platform_selectors(
    all_sources: &BTreeMap<String, PathBuf>,
    manifest: &ConeManifest,
    target_platform: &str,
    root: &Path,
    main: &Path,
) -> Result<Vec<PathBuf>> {
    let mut selected = all_sources.clone();

    for selector in &manifest.selectors {
        if selector.when.platform != target_platform {
            continue;
        }

        // 规则（spec §13.9）：
        // - include：加入 source set
        // - exclude：从 source set 移除
        // - 多个 selector：按文件顺序应用
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

    let main_rel = normalize_rel_path_forward_slashes(root, main)?;
    if !selected.contains_key(&main_rel) {
        return Err(miette!(
            "platform selector 将入口文件从 sources 中移除了（platform={target_platform}，main={main_rel}）"
        ));
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

/// 一个“稳定且平台无关”的最小 glob matcher（spec §13.9：实现定义，但必须稳定）。
///
/// 支持的语法子集：
/// - `*`：匹配任意长度字符序列（不跨 `/`）
/// - `**`：匹配任意长度字符序列（可跨 `/`）
/// - `?`：匹配任意单个字符（不匹配 `/`）
/// - 其它字符：按字面量匹配
///
/// 输入要求：
/// - `pattern` 与 `path` 都应使用 forward slashes 且为 UTF-8。
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
///
/// 说明：
/// - v0 阶段只支持 host target（与 TODO T0803 的范围一致）；
/// - 该 id 使用 spec 中的 `linux-x64` / `macos-arm64` / `windows-x64` 命名风格（spec §13.7/§13.9）。
fn host_target_platform_id() -> String {
    let os = option_env!("CARGO_CFG_TARGET_OS").unwrap_or("unknown");
    let arch = option_env!("CARGO_CFG_TARGET_ARCH").unwrap_or("unknown");

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

        // 若没有 selector 匹配，则保持默认 source discovery（src/**/*.scoop）。
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
