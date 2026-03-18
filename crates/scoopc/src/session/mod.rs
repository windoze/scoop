//! 编译会话（session）。
//!
//! 目的：
//! - 把“加载 sysroot / 读取源文件 / 驱动前端各阶段”的入口集中在一个地方
//! - 确保任何编译流程默认都包含 sysroot，从而让名字解析/类型检查在同一环境下工作
//!
//! 当前阶段：只实现 sysroot 注入 + 顶层符号索引的构建入口。

use miette::{Context as _, Result};
use thiserror::Error;

use crate::parser::{parse_file, ParseError};
use crate::resolve::{Index, ResolveError};
use crate::source::SourceFile;
use crate::sysroot::Sysroot;

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
    sysroot: Sysroot,
}

impl Session {
    /// 使用默认 sysroot 创建会话。
    pub fn new() -> Result<Self> {
        let sysroot = Sysroot::load_from(Sysroot::default_path())
            .wrap_err("加载默认 sysroot 失败")?;
        Ok(Self { sysroot })
    }

    /// 直接注入 sysroot（用于测试或未来的自定义工具链）。
    pub fn with_sysroot(sysroot: Sysroot) -> Self {
        Self { sysroot }
    }

    pub fn sysroot(&self) -> &Sysroot {
        &self.sysroot
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
            asts.push(parse_file(s)?);
        }

        let mut pairs: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
        for f in &self.sysroot.files {
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
    fn index_includes_sysroot_symbols() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");

        let index = sess.build_top_level_index(&[src]).unwrap();
        assert!(index.by_fqn.contains_key("scoop.core.Any"));
        assert!(index.by_fqn.contains_key("a.main"));
    }
}

