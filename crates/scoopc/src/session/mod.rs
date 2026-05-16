//! 编译会话（session）。
//!
//! 目的：
//! - 把“加载 sysroot / 读取源文件 / 驱动前端各阶段”的入口集中在一个地方
//! - 确保任何编译流程默认都包含 sysroot，从而让名字解析/类型检查在同一环境下工作
//!
//! P8 起，effect pipeline 已收口为单一路径；session 不再承载 legacy/refactor bifurcation。

use std::path::{Path, PathBuf};

use miette::{Context as _, Result};
use thiserror::Error;

use crate::comptime::ConstEvalError;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::source::SourceFile;
use crate::sysroot::Sysroot;

/// 会话构造时一次性收口的配置项。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionOptions {
    sysroot_overlay: Option<PathBuf>,
}

impl SessionOptions {
    pub const fn new() -> Self {
        Self {
            sysroot_overlay: None,
        }
    }

    pub fn with_sysroot_overlay(mut self, overlay_root: impl Into<PathBuf>) -> Self {
        self.sysroot_overlay = Some(overlay_root.into());
        self
    }

    pub fn with_env_fallback(mut self) -> Self {
        if self.sysroot_overlay.is_none() {
            self.sysroot_overlay = std::env::var_os(crate::sysroot::SYSROOT_OVERLAY_ENV)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from);
        }
        self
    }

    pub fn sysroot_overlay(&self) -> Option<&Path> {
        self.sysroot_overlay.as_deref()
    }
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum SessionError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Comptime(#[from] ConstEvalError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),
}

/// 一次编译过程的全局上下文。
#[derive(Debug)]
pub struct Session {
    options: SessionOptions,
    sysroot: Sysroot,
}

impl Session {
    /// 使用默认 sysroot 创建会话。
    pub fn new() -> Result<Self> {
        Self::with_options(SessionOptions::new())
    }

    /// 使用显式 session options 创建会话。
    pub fn with_options(options: SessionOptions) -> Result<Self> {
        let sysroot =
            Sysroot::load_from_with_overlay(Sysroot::default_path(), options.sysroot_overlay())
                .wrap_err("加载默认 sysroot 失败")?;
        Ok(Self { options, sysroot })
    }

    /// 直接注入 sysroot（用于测试或未来的自定义工具链）。
    pub fn with_sysroot(sysroot: Sysroot) -> Self {
        Self::with_sysroot_and_options(sysroot, SessionOptions::new())
    }

    /// 直接注入 sysroot，并显式指定 session options。
    pub fn with_sysroot_and_options(sysroot: Sysroot, options: SessionOptions) -> Self {
        Self { options, sysroot }
    }

    pub fn sysroot(&self) -> &Sysroot {
        &self.sysroot
    }

    pub fn options(&self) -> &SessionOptions {
        &self.options
    }

    /// 解析一个源文件为 AST。
    pub fn parse(&self, source: &SourceFile) -> Result<crate::ast::File, ParseError> {
        parse_file(source)
    }

    /// 构建“包含 sysroot”的顶层符号索引。
    ///
    /// 说明：
    /// - 当前索引仅覆盖顶层符号（见 `crate::resolve`）
    /// - 后续阶段会在这里接入 import/作用域/类型检查等更多 pass
    pub fn build_top_level_index(&self, sources: &[SourceFile]) -> Result<Index, SessionError> {
        let mut asts = Vec::with_capacity(sources.len());
        for s in sources {
            let ast = parse_file(s)?;
            asts.push(ast);
        }
        {
            let source_refs = sources.iter().collect::<Vec<_>>();
            let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
            crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
                self.sysroot(),
                &source_refs,
                &mut ast_refs,
            )?;
        }

        let mut pairs: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
        for f in self.sysroot.index_files() {
            if sources
                .iter()
                .any(|source| source.path() == f.source.path())
            {
                continue;
            }
            pairs.push((&f.source, &f.ast));
        }
        for (s, a) in sources.iter().zip(asts.iter()) {
            pairs.push((s, a));
        }

        Ok(Index::build(&pairs)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_uses_single_pipeline_defaults() {
        let sess = Session::new().unwrap();
        assert_eq!(sess.options(), &SessionOptions::new());
    }

    #[test]
    fn explicit_session_options_use_same_single_pipeline() {
        let sess = Session::with_options(SessionOptions::new()).unwrap();
        assert_eq!(sess.options(), &SessionOptions::new());
    }

    #[test]
    fn index_includes_sysroot_symbols() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");

        let index = sess.build_top_level_index(&[src]).unwrap();
        assert!(index.by_fqn.contains_key("scoop.core.Any"));
        assert!(index.by_fqn.contains_key("a.main"));
    }

    #[test]
    fn build_top_level_index_trims_package_level_comptime_if_across_source_set() {
        let sess = Session::new().unwrap();
        let defs = SourceFile::new_virtual(
            "<defs>",
            "package a\nimport scoop.core.*\nconst fun truthy<T>(value: T): Bool { return true }\n",
        );
        let main = SourceFile::new_virtual(
            "<main>",
            "package a\nimport scoop.core.*\ncomptime if (truthy<Int>(1)) {\n    fun selected() {}\n}\n",
        );

        let index = sess.build_top_level_index(&[defs, main]).unwrap();
        assert!(index.by_fqn.contains_key("a.selected"));
    }
}
