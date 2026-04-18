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
mod pattern;
mod stmt;
mod types;

#[cfg(test)]
mod tests;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::syntax::lexer::{LexError, lex};
use crate::syntax::token::{Keyword, Symbol, Token, TokenKind};

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Lex(#[from] LexError),

    #[error("语法错误：共 {count} 个错误")]
    #[diagnostic(code(scoop::parse::many_errors))]
    Many {
        count: usize,
        #[label("第一个错误发生在这里")]
        span: miette::SourceSpan,
        #[related]
        errors: Vec<ParseError>,
    },

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

    #[error("语法错误：`::class` 的左侧必须是类型名路径（例如 `String::class`）")]
    #[diagnostic(code(scoop::parse::class_literal_receiver_invalid))]
    ClassLiteralReceiverInvalid {
        #[label("这里需要类型名")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：局部 `@Unsafe` block 必须写成 `@Unsafe do {{ ... }}`")]
    #[diagnostic(
        code(scoop::parse::unsafe_block_requires_do),
        help(
            "将 `@Unsafe {{ ... }}` 改写为 `@Unsafe do {{ ... }}`；裸 `{{ ... }}` 保留给 closure"
        )
    )]
    UnsafeBlockRequiresDo {
        #[label("这里的裸 `{{ ... }}` 不再作为局部 unsafe block 解析")]
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
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(source_text: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source_text,
            tokens,
            i: 0,
            errors: Vec::new(),
        }
    }

    fn record_error(&mut self, err: ParseError) {
        self.errors.push(err);
    }

    fn finish(self, file: ast::File) -> Result<ast::File, ParseError> {
        let count = self.errors.len();
        match count {
            0 => Ok(file),
            1 => Err(self.errors.into_iter().next().expect("len==1 已保证有元素")),
            _ => {
                let span = self
                    .errors
                    .iter()
                    .find_map(ParseError::primary_span)
                    .unwrap_or_else(|| (0usize, 0usize).into());
                Err(ParseError::Many {
                    count,
                    span,
                    errors: self.errors,
                })
            }
        }
    }

    fn bump_val_or_var_keyword(&mut self) -> Result<Token, ParseError> {
        let tok = *self.peek();
        match tok.kind {
            TokenKind::Keyword(Keyword::Val) | TokenKind::Keyword(Keyword::Var) => Ok(self.bump()),
            _ => Err(ParseError::Expected {
                expected: "`val` / `var`",
                found: tok.kind,
                span: tok.span.into(),
            }),
        }
    }
}

impl ParseError {
    fn primary_span(&self) -> Option<miette::SourceSpan> {
        match self {
            ParseError::Lex(_) => None,
            ParseError::Many { span, .. } => Some(*span),
            ParseError::Expected { span, .. } => Some(*span),
            ParseError::UnterminatedGroup { span, .. } => Some(*span),
            ParseError::FStringUnescapedRBrace { span } => Some(*span),
            ParseError::ClassLiteralReceiverInvalid { span } => Some(*span),
            ParseError::UnsafeBlockRequiresDo { span } => Some(*span),
        }
    }
}
