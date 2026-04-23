//! Sysroot 加载。
//!
//! Sysroot 是一组 `.scoop` 源文件，描述编译器内建的 API 表面：
//! - 对编译器：提供内建类型/函数/效果的签名来源
//! - 对工具链：LSP/文档可从 sysroot 读取类型信息
//!
//! 当前阶段：只实现“定位目录 + 读取 `.scoop` + 调用 parser”。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use crate::source::SourceFile;

/// T0143：sysroot 文件分为两类：
/// - `files`：签名文件（仅声明，不编译函数体）。用于 Index 与类型检查。
/// - `compilable_source_paths`：含有函数体的 sysroot 文件（如 `string.scoop`），
///   需作为编译单元的一部分参与完整的 resolve → typecheck → HIR lowering → codegen 管线。
///   这些文件不重复加入 `files`，以避免 Index 中出现双重声明。
#[derive(Debug)]
pub struct Sysroot {
    pub root: PathBuf,
    pub files: Vec<SysrootFile>,
    /// T0143：需要被编译（而非仅作为签名索引）的 sysroot 源文件路径。
    pub compilable_source_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct SysrootFile {
    pub path: PathBuf,
    pub source: SourceFile,
    pub ast: crate::ast::File,
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
        let root = root.as_ref().to_path_buf();
        let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
            format!(
                "无法定位 sysroot 目录：{}（当前实现默认相对工作目录）",
                root.display()
            )
        })?;

        let mut paths = Vec::new();
        collect_scoop_files(&root, &mut paths)?;
        if paths.is_empty() {
            return Err(miette!(
                "sysroot 目录下没有 .scoop 文件：{}",
                root.display()
            ));
        }

        let mut files = Vec::new();
        let mut compilable_source_paths = Vec::new();
        for path in paths {
            // T0143：含有顶层函数体的 sysroot 文件（如 string.scoop）需要走完整编译管线，
            // 而非仅作为签名索引。将它们分离出来，由 build pipeline 加入 input.sources。
            if is_compilable_sysroot_file(&path) {
                compilable_source_paths.push(path);
                continue;
            }

            let source = SourceFile::load(&path)?;
            let mut ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;
            crate::comptime::trim_package_level_comptime_ifs(&source, &mut ast).wrap_err_with(
                || {
                    format!(
                        "裁剪 sysroot 文件的 package-level comptime if 失败：{}",
                        path.display()
                    )
                },
            )?;
            files.push(SysrootFile { path, source, ast });
        }

        Ok(Self {
            root,
            files,
            compilable_source_paths,
        })
    }
}

/// T0143：判断 sysroot 文件是否需要作为编译单元的一部分（而非仅签名索引）。
/// 当前规则：`string.scoop`、`print.scoop` 与 `task.scoop` 含有需要编译的函数体，
/// 需要走完整编译管线。后续可扩展为基于文件内容或 annotation 的判断。
fn is_compilable_sysroot_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == "string.scoop" || name == "print.scoop" || name == "task.scoop")
}

/// T0143：收集 sysroot 中需要走完整编译管线的源文件路径。
/// 供 build pipeline 的 `load_stdlib_sources()` 调用，将这些文件与 stdlib 一同加入 `input.sources`。
pub fn collect_compilable_sysroot_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut all = Vec::new();
    collect_scoop_files(root, &mut all)?;
    for path in all {
        if is_compilable_sysroot_file(&path) {
            out.push(path);
        }
    }
    Ok(())
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

    #[test]
    fn load_default_sysroot() {
        // 单元测试运行时工作目录通常是 workspace 根目录。
        // 若未来变动，可改为通过 env/config 指定。
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        assert!(!sysroot.files.is_empty());
    }
}
