//! Parser（语法分析）。
//!
//! 本阶段采用“保守增量”策略：
//! - 先支持文件级结构：`package` / `import` / 顶层 `fun`
//! - 块表达式/函数体：解析为 `Block { stmts }`（语句子集会逐步扩展）
//! - 表达式/语句的完整解析会在后续阶段逐步补齐

mod cursor;
mod decls;
mod expr;
mod file;
mod stmt;
mod types;

#[cfg(test)]
mod tests;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::syntax::lexer::{LexError, lex};
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

    #[error("语法错误：插值字符串中出现未转义的 `}}`（使用 `}}}}` 表示字面量 `}}`）")]
    #[diagnostic(code(scoop::parse::f_string_unescaped_rbrace))]
    FStringUnescapedRBrace {
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

pub fn parse_file(source: &SourceFile) -> Result<ast::File, ParseError> {
    let tokens = lex(source.text())?;
    Parser::new(source.text(), tokens).parse_file()
}

struct Parser<'a> {
    source_text: &'a str,
    tokens: Vec<Token>,
    i: usize,
}

impl<'a> Parser<'a> {
    fn new(source_text: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source_text,
            tokens,
            i: 0,
        }
    }
}
