//! Parser（语法分析）。
//!
//! 本阶段采用“保守增量”策略：
//! - 先支持文件级结构：`package` / `import` / 顶层 `fun`
//! - 函数体暂时只保证 `{ ... }` 的括号平衡，并记录 `Span`
//! - 表达式/语句的完整解析会在后续阶段逐步补齐

mod cursor;
mod decls;
mod file;
mod types;

#[cfg(test)]
mod tests;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::syntax::lexer::{lex, LexError};
use crate::syntax::token::{Symbol, Token, TokenKind};

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Lex(#[from] LexError),

    #[error("语法错误：期望 {expected}，但遇到 {found:?}")]
    #[diagnostic(code(scoop::parse::expected))]
    Expected {
        expected: &'static str,
        found: TokenKind,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：未闭合的分组（期望 {close:?}）")]
    #[diagnostic(code(scoop::parse::unterminated_group))]
    UnterminatedGroup {
        close: Symbol,
        #[label("从这里开始")]
        span: miette::SourceSpan,
    },
}

pub fn parse_file(source: &SourceFile) -> Result<ast::File, ParseError> {
    let tokens = lex(source.text())?;
    Parser::new(tokens).parse_file()
}

struct Parser {
    tokens: Vec<Token>,
    i: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, i: 0 }
    }
}
