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

    #[error("语法错误：`-> resume {{ ... }}` 已从 handler arm 语法移除")]
    #[diagnostic(
        code(scoop::parse::handle_immediate_resume_removed),
        help("改用 `Effect.op(...), k -> {{ k.resume(...) }}`")
    )]
    HandleImmediateResumeRemoved {
        #[label("这里的 `resume` 不再作为 handler arm 语法关键字")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：handler keyword `with` was replaced by `on`")]
    #[diagnostic(
        code(scoop::parse::handler_with_keyword_removed),
        help(
            "将 `handle {{ body }} with {{ ... }}` 改写为 `handle {{ body }} on {{ ... }}`；值更新表达式 `expr with {{ ... }}` 保持不变"
        )
    )]
    HandlerWithKeywordRemoved {
        #[label("这里的 handler `with` 不再作为 handler arm 列表关键字解析")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：`inline` 关键字已移除")]
    #[diagnostic(
        code(scoop::parse::inline_modifier_removed),
        help("删除 `inline` 修饰符；Scoop 不再提供内联提示 surface")
    )]
    InlineModifierRemoved {
        #[label("这里的 `inline` 不再作为声明修饰符解析")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：`perform` keyword was removed; call effect operation directly")]
    #[diagnostic(
        code(scoop::parse::perform_keyword_removed),
        help("将 `perform Effect.op(args)` 改写为 `Effect.op(args)`")
    )]
    PerformKeywordRemoved {
        #[label("这里的 `perform` 不再作为 effect operation 调用前缀解析")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：bound keyword `{keyword}` 只能出现在 generic bound 位置，不能作为类型使用")]
    #[diagnostic(
        code(scoop::parse::bound_keyword_type_position),
        help("将 `{keyword}` 用作泛型约束右侧，例如 `<T: {keyword}>` 或 `where T: {keyword}`")
    )]
    BoundKeywordTypePosition {
        keyword: &'static str,
        #[label("这里不是 generic bound 右侧")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：赋值表达式不能进入 HIR；assignment 当前只能作为语句使用")]
    #[diagnostic(
        code(scoop::parse::assignment_expression_not_allowed),
        help(
            "将赋值单独写成语句，例如 `x = value`，不要嵌入 initializer、return、条件或参数表达式中"
        )
    )]
    AssignmentExpressionNotAllowed {
        #[label("这里的赋值位于表达式上下文")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：spread 实参 `*arg` 只能出现在调用参数列表中，不能作为普通表达式进入 HIR")]
    #[diagnostic(code(scoop::parse::spread_arg_outside_call))]
    SpreadArgOutsideCall {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "语法错误：命名实参 `name = value` 只能出现在调用参数列表中，不能作为普通表达式进入 HIR"
    )]
    #[diagnostic(code(scoop::parse::named_arg_outside_call))]
    NamedArgOutsideCall {
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
            ParseError::ClassLiteralReceiverInvalid { span } => Some(*span),
            ParseError::UnsafeBlockRequiresDo { span } => Some(*span),
            ParseError::HandleImmediateResumeRemoved { span } => Some(*span),
            ParseError::HandlerWithKeywordRemoved { span } => Some(*span),
            ParseError::InlineModifierRemoved { span } => Some(*span),
            ParseError::PerformKeywordRemoved { span } => Some(*span),
            ParseError::BoundKeywordTypePosition { span, .. } => Some(*span),
            ParseError::AssignmentExpressionNotAllowed { span } => Some(*span),
            ParseError::SpreadArgOutsideCall { span } => Some(*span),
            ParseError::NamedArgOutsideCall { span } => Some(*span),
        }
    }
}
