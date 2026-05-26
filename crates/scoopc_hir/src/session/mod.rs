//! 编译会话（session）。
//!
//! 目的：
//! - 把“加载 sysroot / 读取源文件 / 驱动前端各阶段”的入口集中在一个地方
//! - 确保任何编译流程默认都包含 sysroot，从而让名字解析/类型检查在同一环境下工作
//!
//! P8 起，effect pipeline 已收口为单一路径；session 不再承载 legacy/bifurcation。

use std::path::{Path, PathBuf};

use miette::{Context as _, Result};
use thiserror::Error;

use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::source::SourceFile;
use crate::sysroot::Sysroot;

/// 外部 driver 可通过该环境变量为单次构建显式加载额外 sysroot cones。
pub const SYSROOT_DEPENDENCIES_ENV: &str = "SCOOP_SYSROOT_DEPS";

/// 会话构造时一次性收口的配置项。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionOptions {
    sysroot_overlay: Option<PathBuf>,
    extra_sysroot_dependencies: Vec<String>,
}

impl SessionOptions {
    pub const fn new() -> Self {
        Self {
            sysroot_overlay: None,
            extra_sysroot_dependencies: Vec::new(),
        }
    }

    pub fn with_sysroot_overlay(mut self, overlay_root: impl Into<PathBuf>) -> Self {
        self.sysroot_overlay = Some(overlay_root.into());
        self
    }

    pub fn with_extra_sysroot_dependencies<I, S>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extra_sysroot_dependencies
            .extend(dependencies.into_iter().map(Into::into));
        self.extra_sysroot_dependencies.sort();
        self.extra_sysroot_dependencies.dedup();
        self
    }

    pub fn with_env_fallback(mut self) -> Self {
        if self.sysroot_overlay.is_none() {
            self.sysroot_overlay = std::env::var_os(crate::sysroot::SYSROOT_OVERLAY_ENV)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from);
        }
        if self.extra_sysroot_dependencies.is_empty()
            && let Some(value) = std::env::var_os(SYSROOT_DEPENDENCIES_ENV)
            && let Some(value) = value.to_str()
        {
            self = self.with_extra_sysroot_dependencies(parse_sysroot_dependency_env(value));
        }
        self
    }

    pub fn sysroot_overlay(&self) -> Option<&Path> {
        self.sysroot_overlay.as_deref()
    }

    pub fn extra_sysroot_dependencies(&self) -> &[String] {
        &self.extra_sysroot_dependencies
    }
}

fn parse_sysroot_dependency_env(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum SessionError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

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
        let sysroot = Sysroot::load_auto_from_with_overlay_and_dependencies(
            Sysroot::default_path(),
            options.sysroot_overlay(),
            options.extra_sysroot_dependencies(),
        )
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
    fn session_auto_sysroot_excludes_opt_in_cones_by_default() {
        let sess = Session::new().unwrap();

        let packages = sess
            .sysroot()
            .index_files()
            .filter_map(|file| {
                file.ast.package.as_ref().map(|package| {
                    package
                        .path
                        .iter()
                        .map(|segment| file.source.slice(segment.span))
                        .collect::<Vec<_>>()
                        .join(".")
                })
            })
            .collect::<std::collections::BTreeSet<_>>();

        assert!(packages.contains("scoop.core"));
        assert!(packages.contains("scoop.lang.string"));
        assert!(packages.contains("scoop.collections"));
        assert!(packages.contains("scoop.delegates"));
        assert!(packages.contains("scoop.unsafe"));
        assert!(!packages.contains("scoop.thread"));
        assert!(!packages.contains("scoop.sync"));
        assert!(!packages.contains("scoop.runtime.test"));
    }

    #[test]
    fn session_extra_sysroot_dependencies_load_opt_in_cones() {
        let sess = Session::with_options(
            SessionOptions::new().with_extra_sysroot_dependencies(["scoop.thread"]),
        )
        .unwrap();

        let packages = sess
            .sysroot()
            .index_files()
            .filter_map(|file| {
                file.ast.package.as_ref().map(|package| {
                    package
                        .path
                        .iter()
                        .map(|segment| file.source.slice(segment.span))
                        .collect::<Vec<_>>()
                        .join(".")
                })
            })
            .collect::<std::collections::BTreeSet<_>>();

        assert!(packages.contains("scoop.thread"));
        assert!(!packages.contains("scoop.sync"));
    }
}
