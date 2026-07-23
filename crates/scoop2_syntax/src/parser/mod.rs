//! Parser（语法分析，对应 `docs/spec/grammar.md` 的全部 113 条产生式）。
//!
//! 结构（镜像 legacy `scoopc_ast::parser`）：
//!
//! - 本文件：公开 API（[`parse_file`] / [`ParseResult]）、`Parser` 状态与诊断基础设施；
//! - [`cursor`]：token 游标（peek/bump/eat/expect、`>>`/`>=` 拆分、平衡分组、
//!   各类 lookahead 启发式与类型参数扫描）；
//! - [`file`]：文件级结构（package / import / 顶层 item 分发与恢复）；
//! - [`decls`]：声明（fun / val / type / object / property / generics 声明位）；
//! - [`types`]：类型引用与 effect 行（§5.2 / §6）；
//! - [`expr`]：表达式（Pratt + postfix + 控制流 + handle/try + 插值字符串）；
//! - [`stmt`]：语句与块；
//! - [`pattern`]：`val` 解构模式与 `when` 分支模式（§9）。
//!
//! 错误模型（grammar §3，normative）：每个错误先记录诊断，再做 panic-mode
//! 恢复（顶层 sync 集 / 语句级 sync / 平衡括号跳过）；AST 中没有 `Missing`
//! 占位节点——恢复产出的是 partial-but-valid 节点 + 诊断。

mod cursor;
mod decls;
mod expr;
mod file;
mod pattern;
mod stmt;
mod types;

#[cfg(test)]
mod tests;

use scoop2_base::diag::{Diagnostic, DiagnosticSink};
use scoop2_base::{Interner, NodeId, NodeIdAllocator, SourceFile, Span};

use crate::ast;
use crate::lexer;
use crate::token::{Token, TokenKind};

/// 解析结果：AST + interner + 诊断（lexer + parser，由调用方负责排序）+ NodeId 空间大小。
#[derive(Debug)]
pub struct ParseResult {
    pub file: ast::File,
    /// 本文件所有标识符的 interner；调用方持有并传给 `dump_file` / 语义阶段。
    pub interner: Interner,
    /// lexer 与 parser 的诊断汇总（**未排序**；调用方按需 `sort_by_offset`）。
    pub diagnostics: DiagnosticSink,
    /// 本文件分配过的 [`NodeId`] 数量（语义阶段致密侧表的长度）。
    pub node_count: usize,
}

/// 解析一个源文件。任何输入都不会 panic：lexer/parser 错误一律进入诊断。
pub fn parse_file(source: &SourceFile) -> ParseResult {
    let lexed = lexer::lex(source.text());
    let mut ids = NodeIdAllocator::new();
    let mut interner = Interner::new();
    let mut parser = Parser {
        source: source.text(),
        tokens: lexed.tokens,
        i: 0,
        interner: &mut interner,
        ids: &mut ids,
        diagnostics: lexed.diagnostics,
        depth: 0,
    };
    let file = parser.parse_file_root();
    let node_count = parser.ids.len();
    let diagnostics = std::mem::take(&mut parser.diagnostics);
    // 必须先让 parser 释放对 `interner` 的可变借用，才能把 interner 移出。
    drop(parser);
    ParseResult {
        file,
        interner,
        diagnostics,
        node_count,
    }
}

/// 解析结果（不含 interner；调用方拥有共享 interner）。
///
/// 用于多文件编译：所有文件共享同一个 [`Interner`]，使跨文件 FQN 符号一致。
/// NodeId 仍是每文件独立分配（语义侧表按 `(FileId, NodeId)` 定位）。
#[derive(Debug)]
pub struct ParsedFile {
    pub file: ast::File,
    pub diagnostics: DiagnosticSink,
    pub node_count: usize,
}

/// 用**共享** interner 解析一个源文件（多文件编译用）。
///
/// 任何输入都不会 panic：lexer/parser 错误一律进入诊断。
pub fn parse_file_with(source: &SourceFile, interner: &mut Interner) -> ParsedFile {
    let lexed = lexer::lex(source.text());
    let mut ids = NodeIdAllocator::new();
    let id_base = ids.len();
    let mut parser = Parser {
        source: source.text(),
        tokens: lexed.tokens,
        i: 0,
        interner,
        ids: &mut ids,
        diagnostics: lexed.diagnostics,
        depth: 0,
    };
    let file = parser.parse_file_root();
    let node_count = parser.ids.len() - id_base;
    let diagnostics = std::mem::take(&mut parser.diagnostics);
    drop(parser);
    ParsedFile {
        file,
        diagnostics,
        node_count,
    }
}

/// 表达式/类型递归深度上限（防止病态输入撑爆栈；正常代码远低于该值）。
const MAX_NESTING_DEPTH: u32 = 300;

/// 解析中止标记：错误已记录进 sink，调用方负责恢复。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Abort;

pub(crate) type PResult<T> = Result<T, Abort>;

pub(crate) struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    i: usize,
    interner: &'a mut Interner,
    ids: &'a mut NodeIdAllocator,
    diagnostics: DiagnosticSink,
    /// 表达式/类型递归深度（配合 [`MAX_NESTING_DEPTH`]）。
    depth: u32,
}

impl<'a> Parser<'a> {
    // --------------------------------------------------------------
    // 基础设施：NodeId / intern / 文本
    // --------------------------------------------------------------

    fn nid(&mut self) -> NodeId {
        self.ids.alloc()
    }

    fn intern(&mut self, text: &str) -> scoop2_base::Symbol {
        self.interner.intern(text)
    }

    fn token_text(&self, tok: Token) -> &'a str {
        self.source.get(tok.span.start..tok.span.end).unwrap_or("")
    }

    fn ident(&mut self, tok: Token) -> ast::Ident {
        let symbol = self.intern(self.token_text(tok));
        ast::Ident {
            symbol,
            span: tok.span,
        }
    }

    /// 合成标识符（try/catch 脱糖用）：文本不对应源码位置。
    fn synthetic_ident(&mut self, text: &str, span: Span) -> ast::Ident {
        ast::Ident {
            symbol: self.intern(text),
            span,
        }
    }

    fn record(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    /// 子 parser（receiver 类型切片、f-string hole）：共享 interner 与
    /// NodeId 分配器，诊断独立收集后由调用方合并。
    ///
    /// 返回的 sub-parser 持有对 `self` 的可变借用；调用方必须先让其 drop
    /// （例如把结果提取到一个内层作用域），再使用 `self`。
    fn sub_parser(&mut self, tokens: Vec<Token>) -> Parser<'_> {
        Parser {
            source: self.source,
            tokens,
            i: 0,
            interner: self.interner,
            ids: self.ids,
            diagnostics: DiagnosticSink::new(),
            depth: self.depth,
        }
    }

    // --------------------------------------------------------------
    // 诊断构造
    // --------------------------------------------------------------

    /// 描述一个 token（用于错误消息）：EOF 显示为“文件结尾”，其余显示源码文本。
    fn describe(&self, tok: Token) -> String {
        if tok.kind == TokenKind::Eof {
            return "文件结尾".to_string();
        }
        let text = self.token_text(tok);
        let text = if text.chars().count() > 24 {
            let mut s: String = text.chars().take(24).collect();
            s.push('…');
            s
        } else {
            text.to_string()
        };
        format!("`{text}`")
    }

    fn err_expected(&mut self, expected: &str, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::expected",
                format!("语法错误：期望 {expected}，但遇到 {}", self.describe(found)),
            )
            .with_primary(found.span, "这里"),
        );
        Abort
    }

    fn err_expected_token(&mut self, expected: &'static str, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::expected_token",
                format!("语法错误：期望 {expected}，但遇到 {}", self.describe(found)),
            )
            .with_primary(found.span, "这里"),
        );
        Abort
    }

    fn err_expected_expr(&mut self, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::expected_expression",
                format!("语法错误：期望表达式，但遇到 {}", self.describe(found)),
            )
            .with_primary(found.span, "期望表达式"),
        );
        Abort
    }

    fn err_expected_type(&mut self, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::expected_type",
                format!("语法错误：期望类型，但遇到 {}", self.describe(found)),
            )
            .with_primary(found.span, "期望类型"),
        );
        Abort
    }

    fn err_expected_ident(&mut self, what: &str, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::expected_ident",
                format!(
                    "语法错误：期望{what}（标识符），但遇到 {}",
                    self.describe(found)
                ),
            )
            .with_primary(found.span, "期望标识符"),
        );
        Abort
    }

    fn err_expected_pattern(&mut self, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::expected_pattern",
                format!(
                    "语法错误：期望模式（`_` / 标识符 / tuple / struct / variant / 字面量），但遇到 {}",
                    self.describe(found)
                ),
            )
            .with_primary(found.span, "期望模式"),
        );
        Abort
    }

    fn err_unclosed(&mut self, open_start: usize, close: &'static str) -> Abort {
        let end = self.peek().span.end.max(open_start);
        self.record(
            Diagnostic::error(
                "scoop::parse::unterminated_group",
                format!("语法错误：未闭合的分组（期望 {close}）"),
            )
            .with_primary(Span::new(open_start, end), "从这里开始"),
        );
        Abort
    }

    /// §3.3/§3.4/§3.6/§12.6：完整 initializer / header / body 之后出现多余 token
    /// 是硬错误（legacy 的 skip-and-downgrade 已被否决）。
    fn err_trailing(&mut self, context: &str, found: Token) {
        self.record(
            Diagnostic::error(
                "scoop::parse::trailing_tokens",
                format!(
                    "语法错误：{context}之后存在意外的 token {}",
                    self.describe(found)
                ),
            )
            .with_primary(found.span, "多余的 token"),
        );
    }

    fn err_nesting_too_deep(&mut self, found: Token) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::nesting_too_deep",
                "语法错误：表达式/类型嵌套过深",
            )
            .with_primary(found.span, "这里"),
        );
        Abort
    }

    // --------------------------------------------------------------
    // 专用诊断（§10 移除语法 + 各类 targeted 错误）
    // --------------------------------------------------------------

    fn err_perform_removed(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::perform_keyword_removed",
                "语法错误：`perform` 关键字已移除；请直接调用 effect operation",
            )
            .with_primary(
                span,
                "这里的 `perform` 不再作为 effect operation 调用前缀解析",
            )
            .with_help("将 `perform Effect.op(args)` 改写为 `Effect.op(args)`"),
        );
        Abort
    }

    fn err_inline_removed(&mut self, span: Span) {
        self.record(
            Diagnostic::error(
                "scoop::parse::inline_modifier_removed",
                "语法错误：`inline` 关键字已移除",
            )
            .with_primary(span, "这里的 `inline` 不再作为声明修饰符解析")
            .with_help("删除 `inline` 修饰符；Scoop 不再提供内联提示 surface"),
        );
    }

    fn err_handler_with_removed(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::handler_with_keyword_removed",
                "语法错误：handler 关键字 `with` 已被 `on` 取代",
            )
            .with_primary(span, "这里的 handler `with` 不再作为 handler arm 列表关键字解析")
            .with_help(
                "将 `handle { body } with { ... }` 改写为 `handle { body } on { ... }`；值更新表达式 `expr with { ... }` 保持不变",
            ),
        );
        Abort
    }

    fn err_resume_removed(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::handle_immediate_resume_removed",
                "语法错误：`-> resume { ... }` 已从 handler arm 语法移除",
            )
            .with_primary(span, "这里的 `resume` 不再作为 handler arm 语法关键字")
            .with_help("改用 `Effect.op(...), k -> { k.resume(...) }`"),
        );
        Abort
    }

    fn err_bound_keyword_type_position(&mut self, keyword: &'static str, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::bound_keyword_type_position",
                format!(
                    "语法错误：bound keyword `{keyword}` 只能出现在 generic bound 位置，不能作为类型使用"
                ),
            )
            .with_primary(span, "这里不是 generic bound 右侧")
            .with_help(format!(
                "将 `{keyword}` 用作泛型约束右侧，例如 `<T: {keyword}>` 或 `where T: {keyword}`"
            )),
        );
        Abort
    }

    fn err_assignment_in_expr(&mut self, span: Span) {
        self.record(
            Diagnostic::error(
                "scoop::parse::assignment_expression_not_allowed",
                "语法错误：赋值只能作为语句使用，不能嵌入表达式",
            )
            .with_primary(span, "这里的赋值位于表达式上下文")
            .with_help(
                "将赋值单独写成语句，例如 `x = value`，不要嵌入 initializer、return、条件或参数表达式中",
            ),
        );
    }

    fn err_spread_outside_call(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::spread_arg_outside_call",
                "语法错误：spread 实参 `*arg` 只能出现在调用参数列表中",
            )
            .with_primary(span, "这里"),
        );
        Abort
    }

    fn err_named_arg_outside_call(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::named_arg_outside_call",
                "语法错误：命名实参 `name = value` 只能出现在调用参数列表中",
            )
            .with_primary(span, "这里"),
        );
        Abort
    }

    fn err_unsafe_requires_do(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::unsafe_block_requires_do",
                "语法错误：局部 `@Unsafe` block 必须写成 `@Unsafe do { ... }`",
            )
            .with_primary(span, "这里的裸 `{ ... }` 不再作为局部 unsafe block 解析")
            .with_help(
                "将 `@Unsafe { ... }` 改写为 `@Unsafe do { ... }`；裸 `{ ... }` 保留给 closure",
            ),
        );
        Abort
    }

    fn err_class_lit_receiver(&mut self, span: Span) -> Abort {
        self.record(
            Diagnostic::error(
                "scoop::parse::class_literal_receiver_invalid",
                "语法错误：`::class` 的左侧必须是类型名路径（例如 `String::class`）",
            )
            .with_primary(span, "这里需要类型名"),
        );
        Abort
    }

    fn err_anonymous_object(&mut self, span: Span) {
        self.record(
            Diagnostic::error(
                "scoop::parse::anonymous_object_unsupported",
                "语法错误：匿名 object 表达式（`object : Foo { ... }`）不在语言内",
            )
            .with_primary(span, "匿名 object 表达式")
            .with_help("请改用命名 object 声明（`object Name { ... }`）"),
        );
    }

    // --------------------------------------------------------------
    // 递归深度保护
    // --------------------------------------------------------------

    fn enter(&mut self) -> PResult<()> {
        if self.depth >= MAX_NESTING_DEPTH {
            let tok = self.peek();
            return Err(self.err_nesting_too_deep(tok));
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}
