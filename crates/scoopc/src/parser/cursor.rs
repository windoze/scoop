//! Token cursor 与基础消费函数。
//!
//! 这一层只提供“看/吃 token”的能力，不引入更高层的语法概念。

use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, Token, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    pub(super) fn consume_balanced(
        &mut self,
        open: Symbol,
        close: Symbol,
    ) -> Result<Span, ParseError> {
        let open_tok = self.expect_symbol(open)?;
        let start = open_tok.span.start;

        let mut depth = 1usize;
        while !self.peek_kind(TokenKind::Eof) {
            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                if sym == open {
                    depth += 1;
                } else if sym == close {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Span::new(start, tok.span.end));
                    }
                }
            }
        }

        Err(ParseError::UnterminatedGroup {
            close,
            span: Span::new(start, self.peek().span.end).into(),
        })
    }

    /// 在已经消费了 `open` 的前提下，继续消费直到与之匹配的 `close`（含 close）。
    ///
    /// 该函数用于“当前语法形态不支持/出现错误，但仍需保持 token cursor 与括号平衡正确”的场景，
    /// 避免上层错误恢复把内部的 `)`/`}` 误当作外层 block 的结束符号而引发级联错误。
    pub(super) fn consume_balanced_after_open(
        &mut self,
        open: Symbol,
        close: Symbol,
        start: usize,
    ) -> Result<Span, ParseError> {
        let mut depth = 1usize;
        while !self.peek_kind(TokenKind::Eof) {
            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                if sym == open {
                    depth += 1;
                } else if sym == close {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Span::new(start, tok.span.end));
                    }
                }
            }
        }

        Err(ParseError::UnterminatedGroup {
            close,
            span: Span::new(start, self.peek().span.end).into(),
        })
    }

    pub(super) fn expect_keyword(&mut self, kw: Keyword) -> Result<Token, ParseError> {
        if self.peek_keyword(kw) {
            Ok(self.bump())
        } else {
            let tok = *self.peek();
            Err(ParseError::Expected {
                expected: kw_name(kw),
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    pub(super) fn expect_symbol(&mut self, sym: Symbol) -> Result<Token, ParseError> {
        if self.peek_symbol(sym) {
            Ok(self.bump())
        } else {
            let tok = *self.peek();
            Err(ParseError::Expected {
                expected: sym_name(sym),
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    pub(super) fn expect_kind(
        &mut self,
        kind: TokenKind,
        expected: &'static str,
    ) -> Result<Token, ParseError> {
        if self.peek_kind(kind) {
            Ok(self.bump())
        } else {
            let tok = *self.peek();
            Err(ParseError::Expected {
                expected,
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    pub(super) fn eat_symbol(&mut self, sym: Symbol) -> bool {
        if self.peek_symbol(sym) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(super) fn peek(&self) -> &Token {
        self.tokens.get(self.i).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer must produce at least EOF token")
        })
    }

    /// 向前看第 `n` 个 token（`n=0` 等价于 `peek()`）。
    ///
    /// 超出范围时返回最后一个 token（lexer 保证至少有 EOF）。
    pub(super) fn peek_n(&self, n: usize) -> &Token {
        self.tokens.get(self.i + n).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer must produce at least EOF token")
        })
    }

    pub(super) fn bump(&mut self) -> Token {
        let tok = *self.peek();
        self.i = (self.i + 1).min(self.tokens.len());
        tok
    }

    pub(super) fn peek_kind(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    /// 判断当前 token 是否为指定文本的标识符（用于上下文关键字等场景）。
    pub(super) fn peek_ident_text(&self, text: &str) -> bool {
        if !self.peek_kind(TokenKind::Ident) {
            return false;
        }
        let tok = self.peek();
        self.source_text.get(tok.span.start..tok.span.end) == Some(text)
    }

    pub(super) fn peek_keyword(&self, kw: Keyword) -> bool {
        self.peek().kind == TokenKind::Keyword(kw)
    }

    pub(super) fn peek_symbol(&self, sym: Symbol) -> bool {
        self.peek().kind == TokenKind::Symbol(sym)
    }

    pub(super) fn peek_after_modifiers(&self) -> &Token {
        let idx = self.skip_decl_prefix_idx(self.i);
        self.tokens.get(idx).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer must produce at least EOF token")
        })
    }

    /// 跳过“声明前缀”token（modifiers + annotations），返回前缀后第一个 token 的索引。
    ///
    /// 说明：
    /// - 该函数只用于 lookahead/分流/错误恢复；
    /// - 语法错误时会尽量保守：只保证 cursor 能前进，不尝试做严格校验。
    fn skip_decl_prefix_idx(&self, mut idx: usize) -> usize {
        loop {
            let tok = self.tokens.get(idx).unwrap_or_else(|| {
                self.tokens
                    .last()
                    .expect("lexer must produce at least EOF token")
            });

            match tok.kind {
                TokenKind::Keyword(kw) if is_modifier_keyword(kw) => {
                    idx = idx.saturating_add(1);
                    continue;
                }
                TokenKind::Symbol(Symbol::At) => {
                    idx = self.skip_one_annotation_idx(idx);
                    continue;
                }
                _ => return idx,
            }
        }
    }

    /// 跳过单个注解使用（从 `@` 开始），返回其末尾后一位索引。
    ///
    /// 支持形态：
    /// - `@Name`
    /// - `@Namespace.Name(args)`
    /// - `@property:Name(args)`（use-site target）
    fn skip_one_annotation_idx(&self, mut idx: usize) -> usize {
        // consume `@`
        idx = idx.saturating_add(1);

        // 需要一个标识符；若缺失，保守返回（只跳过 `@`）。
        if !matches!(self.tokens.get(idx).map(|t| t.kind), Some(TokenKind::Ident)) {
            return idx;
        }

        // consume first ident
        idx = idx.saturating_add(1);

        // optional use-site target：`@target:Name`
        if matches!(
            self.tokens.get(idx).map(|t| t.kind),
            Some(TokenKind::Symbol(Symbol::Colon))
        ) {
            // consume ':'
            idx = idx.saturating_add(1);
            // require a new ident for real annotation path start
            if !matches!(self.tokens.get(idx).map(|t| t.kind), Some(TokenKind::Ident)) {
                return idx;
            }
            idx = idx.saturating_add(1);
        }

        // dotted path segments
        loop {
            if !matches!(
                self.tokens.get(idx).map(|t| t.kind),
                Some(TokenKind::Symbol(Symbol::Dot))
            ) {
                break;
            }
            // consume '.'
            idx = idx.saturating_add(1);
            // consume ident if present; otherwise stop（避免越界与死循环）
            if !matches!(self.tokens.get(idx).map(|t| t.kind), Some(TokenKind::Ident)) {
                break;
            }
            idx = idx.saturating_add(1);
        }

        // optional args list: `( ... )`
        if !matches!(
            self.tokens.get(idx).map(|t| t.kind),
            Some(TokenKind::Symbol(Symbol::LParen))
        ) {
            return idx;
        }

        let mut depth_paren = 0usize;
        while let Some(tok) = self.tokens.get(idx) {
            match tok.kind {
                TokenKind::Eof => break,
                TokenKind::Symbol(Symbol::LParen) => {
                    depth_paren += 1;
                    idx = idx.saturating_add(1);
                }
                TokenKind::Symbol(Symbol::RParen) => {
                    depth_paren = depth_paren.saturating_sub(1);
                    idx = idx.saturating_add(1);
                    if depth_paren == 0 {
                        break;
                    }
                }
                _ => idx = idx.saturating_add(1),
            }
        }

        idx
    }

    pub(super) fn is_type_decl_start(&self) -> bool {
        matches!(
            self.peek_after_modifiers().kind,
            TokenKind::Keyword(
                Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
            )
        )
    }

    pub(super) fn is_top_level_item_start(&self) -> bool {
        if matches!(
            self.peek().kind,
            TokenKind::Keyword(Keyword::Package | Keyword::Import)
        ) {
            return true;
        }

        matches!(
            self.peek_after_modifiers().kind,
            TokenKind::Keyword(
                Keyword::Typealias
                    | Keyword::Fun
                    | Keyword::Val
                    | Keyword::Var
                    | Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
                    | Keyword::Object
                    | Keyword::Companion
            )
        )
    }

    /// 轻量级 lookahead：判断当前位置是否形如“扩展属性声明”（spec §10.3）：
    ///
    /// - `val ReceiverType.name: Type ...`
    /// - `var ReceiverType.name: Type ...`
    ///
    /// 说明：
    /// - 该判断只用于顶层解析分流（`val/var` → 顶层变量 or 扩展属性）；
    /// - 不做完整语法解析：仅在 header 中寻找 `ReceiverType . name :` 形态。
    pub(super) fn is_extension_property_decl_start(&self) -> bool {
        // 1) 跳过 modifiers，定位到 `val/var`
        let mut idx = self.skip_decl_prefix_idx(self.i);

        let Some(tok) = self.tokens.get(idx) else {
            return false;
        };
        match tok.kind {
            TokenKind::Keyword(Keyword::Val | Keyword::Var) => {}
            _ => return false,
        }
        idx = idx.saturating_add(1);

        // 2) 可选 type params：`val <T> ...`
        if matches!(
            self.tokens.get(idx).map(|t| t.kind),
            Some(TokenKind::Symbol(Symbol::Lt))
        ) {
            let mut depth_angle = 0usize;
            while let Some(tok) = self.tokens.get(idx) {
                match tok.kind {
                    TokenKind::Eof => return false,
                    TokenKind::Symbol(Symbol::Lt) => depth_angle += 1,
                    TokenKind::Symbol(Symbol::Gt) => depth_angle = depth_angle.saturating_sub(1),
                    TokenKind::Symbol(Symbol::GtGt) => depth_angle = depth_angle.saturating_sub(2),
                    _ => {}
                }
                idx = idx.saturating_add(1);
                if depth_angle == 0 {
                    break;
                }
            }
        }

        // 3) 在 header 里寻找 `ReceiverType . name :`：
        // - 停止点：遇到 `=`（进入 initializer）或 `;` 或 EOF
        // - 只在括号/尖括号等深度为 0 时匹配 `:`
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;

        while let Some(tok) = self.tokens.get(idx) {
            match tok.kind {
                TokenKind::Eof => return false,
                TokenKind::Symbol(sym) => match sym {
                    Symbol::LParen => depth_paren += 1,
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    Symbol::Lt => depth_angle += 1,
                    Symbol::Gt => depth_angle = depth_angle.saturating_sub(1),
                    Symbol::GtGt => depth_angle = depth_angle.saturating_sub(2),
                    Symbol::Eq | Symbol::Semicolon => {
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0
                        {
                            return false;
                        }
                    }
                    Symbol::Colon => {
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0
                        {
                            // `... . name :`
                            let Some(name_tok) = self.tokens.get(idx.saturating_sub(1)) else {
                                return false;
                            };
                            let Some(dot_tok) = self.tokens.get(idx.saturating_sub(2)) else {
                                return false;
                            };
                            return name_tok.kind == TokenKind::Ident
                                && dot_tok.kind == TokenKind::Symbol(Symbol::Dot);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }

            idx = idx.saturating_add(1);
        }

        false
    }

    pub(super) fn is_type_member_start(&self) -> bool {
        // Appendix B.2.2：`init { ... }` 初始化块（T0256）在 lexer 层仍是 Ident，
        // 但在 type body 中应被视为 member 起始（用于 initializer 边界判断与错误恢复）。
        let head = self.peek_after_modifiers();
        if head.kind == TokenKind::Ident {
            match self.source_text.get(head.span.start..head.span.end) {
                Some("init") => return true,
                // Appendix B.2.2：`constructor(...) { ... }` 次构造器（T0257）当前同样是上下文关键字。
                Some("constructor") => return true,
                _ => {}
            }
        }

        matches!(
            head.kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Fun
                    | Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
                    | Keyword::Object
                    | Keyword::Companion
            )
        )
    }

    /// 粗粒度判断：当前位置是否“可能是一个语句的起始”。
    ///
    /// 该函数主要用于错误恢复（T0220），用于在 block 内尽量恢复到下一个语句边界，
    /// 而不是因为一个语法错误吞掉后续整个 block。
    pub(super) fn is_stmt_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Return
                    | Keyword::Comptime
                    | Keyword::If
                    | Keyword::When
                    | Keyword::While
                    | Keyword::Break
                    | Keyword::Continue
                    | Keyword::Try
                    | Keyword::Handle
                    | Keyword::Perform
                    | Keyword::Async
                    | Keyword::Await
            ) | TokenKind::Ident
                | TokenKind::IntLiteral
                | TokenKind::StringLiteral(_)
                | TokenKind::Symbol(Symbol::LBrace | Symbol::LParen)
        )
    }
}

fn kw_name(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Public => "`public`",
        Keyword::Internal => "`internal`",
        Keyword::Private => "`private`",
        Keyword::Open => "`open`",
        Keyword::Abstract => "`abstract`",
        Keyword::Sealed => "`sealed`",
        Keyword::Inline => "`inline`",
        Keyword::Override => "`override`",
        Keyword::Const => "`const`",
        Keyword::Annotation => "`annotation`",
        Keyword::Package => "`package`",
        Keyword::Import => "`import`",
        Keyword::Typealias => "`typealias`",
        Keyword::Fun => "`fun`",
        Keyword::Val => "`val`",
        Keyword::Var => "`var`",
        Keyword::Class => "`class`",
        Keyword::Interface => "`interface`",
        Keyword::Struct => "`struct`",
        Keyword::Enum => "`enum`",
        Keyword::Effect => "`effect`",
        Keyword::Object => "`object`",
        Keyword::Companion => "`companion`",
        Keyword::Handle => "`handle`",
        Keyword::With => "`with`",
        Keyword::Perform => "`perform`",
        Keyword::Try => "`try`",
        Keyword::Catch => "`catch`",
        Keyword::Finally => "`finally`",
        Keyword::Async => "`async`",
        Keyword::Await => "`await`",
        Keyword::Return => "`return`",
        Keyword::Comptime => "`comptime`",
        Keyword::If => "`if`",
        Keyword::Else => "`else`",
        Keyword::When => "`when`",
        Keyword::For => "`for`",
        Keyword::In => "`in`",
        Keyword::Out => "`out`",
        Keyword::Where => "`where`",
        Keyword::While => "`while`",
        Keyword::Break => "`break`",
        Keyword::Continue => "`continue`",
        Keyword::Is => "`is`",
        Keyword::As => "`as`",
        Keyword::AsQ => "`as?`",
    }
}

fn is_modifier_keyword(kw: Keyword) -> bool {
    matches!(
        kw,
        Keyword::Public
            | Keyword::Internal
            | Keyword::Private
            | Keyword::Open
            | Keyword::Abstract
            | Keyword::Sealed
            | Keyword::Async
            | Keyword::Inline
            | Keyword::Override
            | Keyword::Const
            | Keyword::Annotation
    )
}

fn sym_name(sym: Symbol) -> &'static str {
    match sym {
        Symbol::At => "`@`",
        Symbol::LParen => "`(`",
        Symbol::RParen => "`)`",
        Symbol::LBrace => "`{`",
        Symbol::RBrace => "`}`",
        Symbol::LBracket => "`[`",
        Symbol::RBracket => "`]`",
        Symbol::Comma => "`,`",
        Symbol::Colon => "`:`",
        Symbol::Semicolon => "`;`",
        Symbol::Dot => "`.`",
        Symbol::Plus => "`+`",
        Symbol::Minus => "`-`",
        Symbol::Star => "`*`",
        Symbol::Slash => "`/`",
        Symbol::Percent => "`%`",
        Symbol::And => "`&`",
        Symbol::Or => "`|`",
        Symbol::Caret => "`^`",
        Symbol::Tilde => "`~`",
        Symbol::Eq => "`=`",
        Symbol::Lt => "`<`",
        Symbol::Gt => "`>`",
        Symbol::Bang => "`!`",
        Symbol::Question => "`?`",
        Symbol::Arrow => "`->`",
        Symbol::EqEq => "`==`",
        Symbol::BangEq => "`!=`",
        Symbol::LtEq => "`<=`",
        Symbol::GtEq => "`>=`",
        Symbol::LtLt => "`<<`",
        Symbol::GtGt => "`>>`",
        Symbol::AndAnd => "`&&`",
        Symbol::OrOr => "`||`",
        Symbol::BangBang => "`!!`",
        Symbol::QuestionDot => "`?.`",
        Symbol::Elvis => "`?:`",
    }
}
