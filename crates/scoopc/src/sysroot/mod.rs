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
        for path in paths {
            let source = SourceFile::load(&path)?;
            let ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;
            files.push(SysrootFile { path, source, ast });
        }

        Ok(Self { root, files })
    }
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
