//! Token 游标与 lookahead 启发式。
//!
//! 这一层只提供“看/吃 token”的能力与纯 token 级扫描，不引入高层语法概念。
//! 所有 lookahead 启发式与 grammar.md §12 的消歧规则一一对应。

use scoop2_base::Span;

use crate::token::{Keyword, Symbol, Token, TokenKind};

use super::{PResult, Parser};

impl<'a> Parser<'a> {
    // --------------------------------------------------------------
    // 基础游标
    // --------------------------------------------------------------

    pub(crate) fn peek(&self) -> Token {
        // invariant: lexer 保证 token 流以恰好一个 Eof 结尾，last 一定存在。
        self.tokens
            .get(self.i)
            .or_else(|| self.tokens.last())
            .copied()
            .unwrap_or(Token {
                kind: TokenKind::Eof,
                span: Span::synthetic(),
            })
    }

    /// 向前看第 `n` 个 token（`n=0` 等价于 `peek()`）；越界钳制到 Eof。
    pub(crate) fn peek_n(&self, n: usize) -> Token {
        self.tokens
            .get(self.i.saturating_add(n))
            .or_else(|| self.tokens.last())
            .copied()
            .unwrap_or(Token {
                kind: TokenKind::Eof,
                span: Span::synthetic(),
            })
    }

    pub(crate) fn bump(&mut self) -> Token {
        let tok = self.peek();
        // 不越过最后的 Eof token。
        self.i = (self.i + 1).min(self.tokens.len().saturating_sub(1));
        tok
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    pub(crate) fn at_kind(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    pub(crate) fn at_sym(&self, sym: Symbol) -> bool {
        self.peek().kind == TokenKind::Symbol(sym)
    }

    pub(crate) fn at_sym_n(&self, n: usize, sym: Symbol) -> bool {
        self.peek_n(n).kind == TokenKind::Symbol(sym)
    }

    pub(crate) fn at_kw(&self, kw: Keyword) -> bool {
        self.peek().kind == TokenKind::Keyword(kw)
    }

    /// 当前 token 是否为指定文本的标识符（上下文关键字）。
    pub(crate) fn at_ident_text(&self, text: &str) -> bool {
        self.at_kind(TokenKind::Ident) && self.token_text(self.peek()) == text
    }

    pub(crate) fn eat_sym(&mut self, sym: Symbol) -> bool {
        if self.at_sym(sym) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_kw(&mut self, kw: Keyword) -> bool {
        if self.at_kw(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_sym(&mut self, sym: Symbol) -> PResult<Token> {
        if self.at_sym(sym) {
            Ok(self.bump())
        } else {
            let tok = self.peek();
            Err(self.err_expected_token(sym_name(sym), tok))
        }
    }

    pub(crate) fn expect_kw(&mut self, kw: Keyword) -> PResult<Token> {
        if self.at_kw(kw) {
            Ok(self.bump())
        } else {
            let tok = self.peek();
            Err(self.err_expected_token(kw_name(kw), tok))
        }
    }

    pub(crate) fn expect_ident(&mut self, what: &str) -> PResult<Token> {
        if self.at_kind(TokenKind::Ident) {
            Ok(self.bump())
        } else {
            let tok = self.peek();
            Err(self.err_expected_ident(what, tok))
        }
    }

    /// 闭合泛型实参列表的 `>`：`>>` 拆成两个 `>`（§5.2）；`>=` 拆成 `>` + `>=`
    /// （§5.2 normative：`A<B<C> >= x` / `A<B<C>> >= x`）。
    pub(crate) fn expect_gt_close(&mut self) -> PResult<Token> {
        if self.at_sym(Symbol::Gt) {
            return Ok(self.bump());
        }
        let tok = self.peek();
        if tok.kind == TokenKind::Symbol(Symbol::GtGt) {
            // 原地拆分：返回左半个 `>`，当前 token 变为右半个 `>`。
            let first = Token {
                kind: TokenKind::Symbol(Symbol::Gt),
                span: Span::new(tok.span.start, tok.span.start + 1),
            };
            self.tokens[self.i] = Token {
                kind: TokenKind::Symbol(Symbol::Gt),
                span: Span::new(tok.span.start + 1, tok.span.end),
            };
            return Ok(first);
        }
        if tok.kind == TokenKind::Symbol(Symbol::GtEq) {
            // 原地拆分：返回左半个 `>`；`>=` 整体保留给后续二元运算。
            // （`>` 字符被“用两次”：既是泛型闭括号，又是 `>=` 的左半——§5.2。）
            return Ok(Token {
                kind: TokenKind::Symbol(Symbol::Gt),
                span: Span::new(tok.span.start, tok.span.start + 1),
            });
        }
        Err(self.err_expected_token("`>`", tok))
    }

    // --------------------------------------------------------------
    // 平衡分组
    // --------------------------------------------------------------

    /// 消费一个平衡分组（含开闭符号）；未闭合时报错并消费到 EOF。
    pub(crate) fn consume_balanced(&mut self, open: Symbol, close: Symbol) -> PResult<Span> {
        let open_tok = self.expect_sym(open)?;
        self.consume_balanced_after_open(open, close, open_tok.span.start)
    }

    /// 在已经消费了 `open` 的前提下，继续消费到与之匹配的 `close`（含 close）。
    pub(crate) fn consume_balanced_after_open(
        &mut self,
        open: Symbol,
        close: Symbol,
        start: usize,
    ) -> PResult<Span> {
        let mut depth = 1usize;
        while !self.at_eof() {
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
        Err(self.err_unclosed(start, sym_name(close)))
    }

    // --------------------------------------------------------------
    // 声明前缀 lookahead（modifier / annotation skipping）
    // --------------------------------------------------------------

    pub(crate) fn peek_after_modifiers(&self) -> Token {
        let idx = self.skip_decl_prefix_idx(self.i);
        self.tokens
            .get(idx)
            .or_else(|| self.tokens.last())
            .copied()
            .unwrap_or(Token {
                kind: TokenKind::Eof,
                span: Span::synthetic(),
            })
    }

    /// 跳过“声明前缀”token（modifiers + annotations），返回前缀后第一个 token 的索引。
    pub(crate) fn skip_decl_prefix_idx(&self, mut idx: usize) -> usize {
        loop {
            let Some(tok) = self.tokens.get(idx).or_else(|| self.tokens.last()) else {
                return idx;
            };
            match tok.kind {
                TokenKind::Keyword(kw) if is_modifier_keyword(kw) => {
                    idx = idx.saturating_add(1);
                }
                TokenKind::Symbol(Symbol::At) => {
                    idx = self.skip_one_annotation_idx(idx);
                }
                _ => return idx,
            }
        }
    }

    /// 跳过单个注解使用（从 `@` 开始），返回其末尾后一位索引。
    pub(crate) fn skip_one_annotation_idx(&self, mut idx: usize) -> usize {
        let kind_at = |i: usize| -> Option<TokenKind> { self.tokens.get(i).map(|t| t.kind) };
        // consume `@`
        idx = idx.saturating_add(1);
        if kind_at(idx) != Some(TokenKind::Ident) {
            return idx;
        }
        idx = idx.saturating_add(1);
        // optional use-site target：`@target:Name`
        if kind_at(idx) == Some(TokenKind::Symbol(Symbol::Colon)) {
            idx = idx.saturating_add(1);
            if kind_at(idx) != Some(TokenKind::Ident) {
                return idx;
            }
            idx = idx.saturating_add(1);
        }
        // dotted path segments
        while kind_at(idx) == Some(TokenKind::Symbol(Symbol::Dot)) {
            idx = idx.saturating_add(1);
            if kind_at(idx) != Some(TokenKind::Ident) {
                break;
            }
            idx = idx.saturating_add(1);
        }
        // optional args list: `( ... )`
        if kind_at(idx) != Some(TokenKind::Symbol(Symbol::LParen)) {
            return idx;
        }
        let mut depth_paren = 0usize;
        while let Some(tok) = self.tokens.get(idx) {
            match tok.kind {
                TokenKind::Eof => break,
                TokenKind::Symbol(Symbol::LParen) => depth_paren += 1,
                TokenKind::Symbol(Symbol::RParen) => {
                    depth_paren = depth_paren.saturating_sub(1);
                    if depth_paren == 0 {
                        idx = idx.saturating_add(1);
                        break;
                    }
                }
                _ => {}
            }
            idx = idx.saturating_add(1);
        }
        idx
    }

    // --------------------------------------------------------------
    // 同步集 / 边界判断
    // --------------------------------------------------------------

    /// 顶层 item 起始（顶层 sync 集，grammar §2）。
    pub(crate) fn is_top_level_item_start(&self) -> bool {
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

    /// 类型体 member 起始（`init` / `constructor` 为上下文关键字）。
    pub(crate) fn is_type_member_start(&self) -> bool {
        let head = self.peek_after_modifiers();
        if head.kind == TokenKind::Ident {
            match self.token_text(head) {
                "init" | "constructor" => return true,
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

    /// 当前位置是否“可能是一个语句的起始”（用于 `return` 无值启发式）。
    pub(crate) fn is_stmt_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Return
                    | Keyword::If
                    | Keyword::When
                    | Keyword::For
                    | Keyword::While
                    | Keyword::Break
                    | Keyword::Continue
                    | Keyword::Try
                    | Keyword::Handle
                    | Keyword::Do
            ) | TokenKind::Ident
                | TokenKind::IntLiteral
                | TokenKind::FloatLiteral
                | TokenKind::CharLiteral
                | TokenKind::StringLiteral(_)
                | TokenKind::Symbol(
                    Symbol::LBrace | Symbol::LParen | Symbol::LBracket | Symbol::At
                )
        )
    }

    /// 语句级恢复的硬边界（`val/var/return/for/while/break/continue`）。
    pub(crate) fn is_recovery_boundary_stmt_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Return
                    | Keyword::For
                    | Keyword::While
                    | Keyword::Break
                    | Keyword::Continue
            )
        )
    }

    /// 顶层边界：initializer / fun 表达式体之后允许出现的东西（§3.3/§3.2）。
    pub(crate) fn at_top_level_boundary(&self) -> bool {
        self.at_eof() || self.at_sym(Symbol::Semicolon) || self.is_top_level_item_start()
    }

    /// 类型体边界：property / accessor / member fun 表达式体之后允许出现的东西（§3.6）。
    pub(crate) fn at_member_boundary(&self) -> bool {
        self.at_eof()
            || self.at_sym(Symbol::Semicolon)
            || self.at_sym(Symbol::RBrace)
            || self.is_type_member_start()
            || self.is_property_accessor_start()
    }

    // --------------------------------------------------------------
    // 扩展属性 / accessor lookahead
    // --------------------------------------------------------------

    /// 轻量 lookahead：当前位置是否形如扩展属性声明 `val/var Receiver.name: T ...`
    /// （只在顶层 `val/var` 分流时使用，grammar §3.7）。
    pub(crate) fn is_extension_property_decl_start(&self) -> bool {
        let mut idx = self.skip_decl_prefix_idx(self.i);
        let kind_at = |i: usize| -> Option<TokenKind> { self.tokens.get(i).map(|t| t.kind) };

        match kind_at(idx) {
            Some(TokenKind::Keyword(Keyword::Val | Keyword::Var)) => {}
            _ => return false,
        }
        idx = idx.saturating_add(1);

        // 可选 type params：`val <T> ...`
        if kind_at(idx) == Some(TokenKind::Symbol(Symbol::Lt)) {
            let mut depth_angle = 0usize;
            loop {
                match kind_at(idx) {
                    None | Some(TokenKind::Eof) => return false,
                    Some(TokenKind::Symbol(Symbol::Lt)) => depth_angle += 1,
                    Some(TokenKind::Symbol(Symbol::Gt)) => {
                        depth_angle = depth_angle.saturating_sub(1)
                    }
                    Some(TokenKind::Symbol(Symbol::GtGt)) => {
                        depth_angle = depth_angle.saturating_sub(2)
                    }
                    Some(TokenKind::Symbol(Symbol::GtEq)) => {
                        depth_angle = depth_angle.saturating_sub(1)
                    }
                    _ => {}
                }
                idx = idx.saturating_add(1);
                if depth_angle == 0 {
                    break;
                }
            }
        }

        // 在 header 里寻找 `ReceiverType . name :`（depth-0 `:`，前两个 token 是 `.` ident）。
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
                    Symbol::Eq | Symbol::Semicolon
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0 =>
                    {
                        return false;
                    }
                    Symbol::Colon
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0 =>
                    {
                        let name_tok = kind_at(idx.saturating_sub(1));
                        let dot_tok = kind_at(idx.saturating_sub(2));
                        return name_tok == Some(TokenKind::Ident)
                            && dot_tok == Some(TokenKind::Symbol(Symbol::Dot));
                    }
                    _ => {}
                },
                _ => {}
            }
            idx = idx.saturating_add(1);
        }
        false
    }

    /// 属性 accessor 起始形态：`get ( ) (=|{)` / `set ( IDENT (...)`（§3.6）。
    pub(crate) fn is_property_accessor_start(&self) -> bool {
        if !self.at_kind(TokenKind::Ident) || !self.at_sym_n(1, Symbol::LParen) {
            return false;
        }
        match self.token_text(self.peek()) {
            "get" => {
                self.at_sym_n(2, Symbol::RParen)
                    && matches!(
                        self.peek_n(3).kind,
                        TokenKind::Symbol(Symbol::Eq | Symbol::LBrace)
                    )
            }
            "set" => {
                self.peek_n(2).kind == TokenKind::Ident
                    && self.at_sym_n(3, Symbol::RParen)
                    && matches!(
                        self.peek_n(4).kind,
                        TokenKind::Symbol(Symbol::Eq | Symbol::LBrace)
                    )
            }
            _ => false,
        }
    }

    // --------------------------------------------------------------
    // 恢复
    // --------------------------------------------------------------

    /// 顶层恢复：跳到 brace-depth 0 的下一个顶层 item 起始或 EOF（§2）。
    pub(crate) fn recover_to_top_level_sync(&mut self) {
        if self.at_eof() {
            return;
        }
        let mut depth_brace = 0usize;
        let first = self.bump();
        if first.kind == TokenKind::Symbol(Symbol::LBrace) {
            depth_brace += 1;
        } else if first.kind == TokenKind::Symbol(Symbol::RBrace) {
            depth_brace = depth_brace.saturating_sub(1);
        }
        while !self.at_eof() {
            if depth_brace == 0 && self.is_top_level_item_start() {
                break;
            }
            let tok = self.bump();
            match tok.kind {
                TokenKind::Symbol(Symbol::LBrace) => depth_brace += 1,
                TokenKind::Symbol(Symbol::RBrace) => {
                    depth_brace = depth_brace.saturating_sub(1)
                }
                _ => {}
            }
        }
    }

    /// 类型体恢复：跳到下一个 member 边界（`}` / `;` / member 起始），不吞 `}`。
    pub(crate) fn skip_type_member_fallback(&mut self) {
        if self.at_eof() || self.at_sym(Symbol::RBrace) {
            return;
        }
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        self.bump_track_depth(&mut depth_paren, &mut depth_brace, &mut depth_bracket);
        while !self.at_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.at_sym(Symbol::Semicolon)
                    || self.at_sym(Symbol::RBrace)
                    || self.is_type_member_start())
            {
                break;
            }
            self.bump_track_depth(&mut depth_paren, &mut depth_brace, &mut depth_bracket);
        }
    }

    /// 顶层 initializer 恢复：跳到 `;` / EOF / 下一个顶层 item 起始（平衡括号跟踪）。
    pub(crate) fn skip_until_top_level_boundary(&mut self) {
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        while !self.at_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.at_sym(Symbol::Semicolon) || self.is_top_level_item_start())
            {
                break;
            }
            self.bump_track_depth(&mut depth_paren, &mut depth_brace, &mut depth_bracket);
        }
    }

    /// 类型体 initializer/accessor 恢复：跳到 `;` / `}` / member / accessor 起始。
    pub(crate) fn skip_until_member_boundary(&mut self) {
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        while !self.at_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.at_sym(Symbol::Semicolon)
                    || self.at_sym(Symbol::RBrace)
                    || self.is_type_member_start()
                    || self.is_property_accessor_start())
            {
                break;
            }
            self.bump_track_depth(&mut depth_paren, &mut depth_brace, &mut depth_bracket);
        }
    }

    fn bump_track_depth(
        &mut self,
        depth_paren: &mut usize,
        depth_brace: &mut usize,
        depth_bracket: &mut usize,
    ) {
        let tok = self.bump();
        match tok.kind {
            TokenKind::Symbol(Symbol::LParen) => *depth_paren += 1,
            TokenKind::Symbol(Symbol::RParen) => *depth_paren = depth_paren.saturating_sub(1),
            TokenKind::Symbol(Symbol::LBrace) => *depth_brace += 1,
            TokenKind::Symbol(Symbol::RBrace) => *depth_brace = depth_brace.saturating_sub(1),
            TokenKind::Symbol(Symbol::LBracket) => *depth_bracket += 1,
            TokenKind::Symbol(Symbol::RBracket) => {
                *depth_bracket = depth_bracket.saturating_sub(1)
            }
            _ => {}
        }
    }

    // --------------------------------------------------------------
    // 类型参数扫描（type-apply / is-arm lookahead；grammar §8.4 / §8.5）
    // --------------------------------------------------------------

    /// `expr<T>` 判断：类型实参扫描成功，且闭合 `>` 之后是 follower 集合内的
    /// token / 有换行 / 经由 `>=` 拆分闭合（§8.4；唯一的换行敏感规则）。
    pub(crate) fn looks_like_type_apply_expr(&self) -> bool {
        if !self.at_sym(Symbol::Lt) {
            return false;
        }
        let mut cursor = ScanCursor {
            tokens: &self.tokens,
            source: self.source,
            i: self.i,
            pending_gt: 0,
        };
        if !cursor.scan_type_args_end(true) {
            return false;
        }
        self.type_apply_can_end_here(cursor.i)
    }

    fn type_apply_can_end_here(&self, next: usize) -> bool {
        match self.tokens.get(next).map(|t| t.kind).unwrap_or(TokenKind::Eof) {
            TokenKind::Symbol(
                Symbol::LParen
                | Symbol::LBrace
                | Symbol::Dot
                | Symbol::QuestionDot
                | Symbol::BangBang
                | Symbol::Comma
                | Symbol::RParen
                | Symbol::RBracket
                | Symbol::RBrace
                | Symbol::Semicolon
                | Symbol::Colon,
            )
            | TokenKind::Keyword(Keyword::As | Keyword::AsQ | Keyword::Is)
            | TokenKind::Eof => true,
            // `>=`-split（§5.2）：闭合 `>` 是 `>=` 的左半，`>=` 作为比较运算符紧随其后。
            TokenKind::Symbol(Symbol::GtEq) => true,
            _ => self.has_line_break_before_token(next),
        }
    }

    fn has_line_break_before_token(&self, next: usize) -> bool {
        let Some(prev) = next
            .checked_sub(1)
            .and_then(|idx| self.tokens.get(idx))
            .or_else(|| self.tokens.last())
        else {
            return false;
        };
        let next_start = self
            .tokens
            .get(next)
            .map(|tok| tok.span.start)
            .unwrap_or(self.source.len());
        self.source
            .get(prev.span.end..next_start)
            .is_some_and(|gap| gap.contains('\n') || gap.contains('\r'))
    }

    /// `when` 非块 arm body 内的 `is TypeRef ->` arm 起始 lookahead（§8.5）。
    pub(crate) fn looks_like_when_is_arm_start(&self) -> bool {
        if !self.at_kw(Keyword::Is) {
            return false;
        }
        let mut cursor = ScanCursor {
            tokens: &self.tokens,
            source: self.source,
            i: self.i + 1,
            pending_gt: 0,
        };
        if !cursor.scan_type_ref_end(true) {
            return false;
        }
        self.tokens
            .get(cursor.i)
            .is_some_and(|t| t.kind == TokenKind::Symbol(Symbol::Arrow))
    }

    /// handle arm 头：`Path<Args>.op` 形态扫描（§8.6）。
    pub(crate) fn type_args_followed_by_dot_ident_at(&self, idx: usize) -> bool {
        if self.tokens.get(idx).map(|t| t.kind) != Some(TokenKind::Symbol(Symbol::Lt)) {
            return false;
        }
        let mut cursor = ScanCursor {
            tokens: &self.tokens,
            source: self.source,
            i: idx,
            pending_gt: 0,
        };
        if !cursor.scan_type_args_end(true) {
            return false;
        }
        self.tokens
            .get(cursor.i)
            .is_some_and(|t| t.kind == TokenKind::Symbol(Symbol::Dot))
            && self
                .tokens
                .get(cursor.i + 1)
                .is_some_and(|t| t.kind == TokenKind::Ident)
    }

    /// handler arm 起始恢复 lookahead：depth-0 `->` 且首个 `(` 之前有 `.`（§12.9）。
    pub(crate) fn looks_like_handle_arm_start_at(&self, idx: usize) -> bool {
        if self.tokens.get(idx).map(|t| t.kind) != Some(TokenKind::Ident) {
            return false;
        }
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;
        let mut saw_lparen = false;
        let mut saw_dot_before_lparen = false;

        let mut j = idx;
        while let Some(tok) = self.tokens.get(j) {
            match tok.kind {
                TokenKind::Eof => return false,
                TokenKind::Symbol(Symbol::Arrow)
                    if depth_paren == 0
                        && depth_brace == 0
                        && depth_bracket == 0
                        && depth_angle == 0 =>
                {
                    return saw_lparen && saw_dot_before_lparen;
                }
                TokenKind::Symbol(Symbol::Semicolon)
                    if depth_paren == 0
                        && depth_brace == 0
                        && depth_bracket == 0
                        && depth_angle == 0 =>
                {
                    return false;
                }
                TokenKind::Symbol(Symbol::RBrace)
                    if depth_paren == 0
                        && depth_brace == 0
                        && depth_bracket == 0
                        && depth_angle == 0 =>
                {
                    return false;
                }
                TokenKind::Symbol(sym) => match sym {
                    Symbol::LParen if depth_angle == 0 => {
                        if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                            saw_lparen = true;
                        }
                        depth_paren += 1;
                    }
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    Symbol::Dot
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0
                            && !saw_lparen =>
                    {
                        saw_dot_before_lparen = true;
                    }
                    Symbol::Lt => depth_angle += 1,
                    Symbol::Gt => depth_angle = depth_angle.saturating_sub(1),
                    Symbol::GtGt => depth_angle = depth_angle.saturating_sub(2),
                    _ => {}
                },
                _ => {}
            }
            j = j.saturating_add(1);
        }
        false
    }
}

// ------------------------------------------------------------------
// 纯 token 扫描（无诊断；用于 type-apply / is-arm lookahead）
// ------------------------------------------------------------------

/// 扫描游标：`pending_gt` 记录 `>>` 拆分后尚未消费的虚拟 `>` 数量。
pub(crate) struct ScanCursor<'a> {
    tokens: &'a [Token],
    source: &'a str,
    i: usize,
    pending_gt: u8,
}

impl ScanCursor<'_> {
    fn kind(&self) -> TokenKind {
        self.tokens
            .get(self.i)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn kind_at(&self, i: usize) -> TokenKind {
        self.tokens
            .get(i)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn text(&self) -> &str {
        self.tokens
            .get(self.i)
            .and_then(|t| self.source.get(t.span.start..t.span.end))
            .unwrap_or("")
    }

    fn bump(&mut self) {
        self.i = self.i.saturating_add(1);
    }

    /// 扫描一个闭合 `>`：处理 `>>` 拆分与 `>=` 拆分（§5.2）。
    ///
    /// `>=` 拆分只允许用于最外层闭合（`allow_gteq`）：消费 `>` 之后 `>=`
    /// 作为后续 token 保留（游标停在该 token 上）。
    fn scan_gt(&mut self, allow_gteq: bool) -> bool {
        if self.pending_gt > 0 {
            self.pending_gt -= 1;
            if self.pending_gt == 0 {
                self.i = self.i.saturating_add(1);
            }
            return true;
        }
        match self.kind() {
            TokenKind::Symbol(Symbol::Gt) => {
                self.bump();
                true
            }
            TokenKind::Symbol(Symbol::GtGt) => {
                // 消费第一个 `>`，第二个留在当前位置。
                self.pending_gt = 1;
                true
            }
            TokenKind::Symbol(Symbol::GtEq) if allow_gteq => {
                // `>=` 的左半 `>` 闭合泛型；游标停在 `>=` 上作为 follower。
                true
            }
            _ => false,
        }
    }

    fn at_gt_like(&self) -> bool {
        self.pending_gt > 0
            || matches!(
                self.kind(),
                TokenKind::Symbol(Symbol::Gt | Symbol::GtGt | Symbol::GtEq)
            )
    }

    /// 扫描 `typeRef`（§6）；成功时游标位于 typeRef 之后。
    ///
    /// `top` 仅传给最外层（允许 `>=` 拆分闭合）。
    pub(crate) fn scan_type_ref_end(&mut self, top: bool) -> bool {
        // base：paren（tuple/group/function type 参数列表）或 path。
        if self.kind() == TokenKind::Symbol(Symbol::LParen) {
            self.bump();
            if self.kind() != TokenKind::Symbol(Symbol::RParen) {
                if !self.scan_type_ref_end(false) {
                    return false;
                }
                while self.kind() == TokenKind::Symbol(Symbol::Comma) {
                    self.bump();
                    if self.kind() == TokenKind::Symbol(Symbol::RParen) {
                        break;
                    }
                    if !self.scan_type_ref_end(false) {
                        return false;
                    }
                }
            }
            if self.kind() != TokenKind::Symbol(Symbol::RParen) {
                return false;
            }
            self.bump();
            // functionTail：`-> typeRef effectAnn?`
            if self.kind() == TokenKind::Symbol(Symbol::Arrow) {
                self.bump();
                if !self.scan_type_ref_end(false) {
                    return false;
                }
                if self.kind() == TokenKind::Symbol(Symbol::Slash) {
                    self.bump();
                    if !self.scan_effect_row_end() {
                        return false;
                    }
                }
            }
        } else {
            if self.kind() != TokenKind::Ident {
                return false;
            }
            self.bump();
            // path segments（`.(` 停止，让位 receiver function type）。
            while self.kind() == TokenKind::Symbol(Symbol::Dot)
                && self.kind_at(self.i + 1) == TokenKind::Ident
            {
                self.bump();
                self.bump();
            }
            if self.kind() == TokenKind::Symbol(Symbol::Lt) && !self.scan_type_args_end(false) {
                return false;
            }
        }

        // 后缀 `?`（零或多）。
        while self.kind() == TokenKind::Symbol(Symbol::Question) {
            self.bump();
        }

        // receiverFnTail：`. ( ... ) -> typeRef effectAnn?`
        if self.kind() == TokenKind::Symbol(Symbol::Dot)
            && self.kind_at(self.i + 1) == TokenKind::Symbol(Symbol::LParen)
        {
            self.bump();
            self.bump();
            if self.kind() != TokenKind::Symbol(Symbol::RParen) {
                if !self.scan_type_ref_end(false) {
                    return false;
                }
                while self.kind() == TokenKind::Symbol(Symbol::Comma) {
                    self.bump();
                    if self.kind() == TokenKind::Symbol(Symbol::RParen) {
                        break;
                    }
                    if !self.scan_type_ref_end(false) {
                        return false;
                    }
                }
            }
            if self.kind() != TokenKind::Symbol(Symbol::RParen) {
                return false;
            }
            self.bump();
            if self.kind() != TokenKind::Symbol(Symbol::Arrow) {
                return false;
            }
            self.bump();
            if !self.scan_type_ref_end(false) {
                return false;
            }
            if self.kind() == TokenKind::Symbol(Symbol::Slash) {
                self.bump();
                if !self.scan_effect_row_end() {
                    return false;
                }
            }
            while self.kind() == TokenKind::Symbol(Symbol::Question) {
                self.bump();
            }
        }

        let _ = top;
        true
    }

    /// 扫描 `typeArgs`（§5.2，含嵌套 / `eff` / 尾逗号 / `>>` / `>=` 拆分）。
    pub(crate) fn scan_type_args_end(&mut self, top: bool) -> bool {
        if self.kind() != TokenKind::Symbol(Symbol::Lt) {
            return false;
        }
        self.bump();

        if self.at_gt_like() {
            return self.scan_gt(top);
        }

        loop {
            // `eff` effect-row 实参（上下文关键字；必须最后）。
            if self.kind() == TokenKind::Ident && self.text() == "eff" {
                self.bump();
                if !self.scan_effect_row_end() {
                    return false;
                }
                if self.kind() == TokenKind::Symbol(Symbol::Comma) {
                    self.bump();
                    if !self.at_gt_like() {
                        return false;
                    }
                }
                break;
            }
            // star projection `*`。
            if self.kind() == TokenKind::Symbol(Symbol::Star) {
                self.bump();
            } else if !self.scan_type_ref_end(false) {
                return false;
            }
            if self.kind() == TokenKind::Symbol(Symbol::Comma) {
                self.bump();
                if self.at_gt_like() {
                    break;
                }
                continue;
            }
            break;
        }

        self.scan_gt(top)
    }

    /// 扫描 `effectRowExpr`（§6.1）。
    fn scan_effect_row_end(&mut self) -> bool {
        if self.kind() == TokenKind::Symbol(Symbol::LParen) {
            self.bump();
            if !self.scan_effect_row_end() {
                return false;
            }
            if self.kind() != TokenKind::Symbol(Symbol::RParen) {
                return false;
            }
            self.bump();
        } else {
            if !self.scan_path_end() {
                return false;
            }
            while self.kind() == TokenKind::Symbol(Symbol::Plus) {
                self.bump();
                if !self.scan_path_end() {
                    return false;
                }
            }
        }
        // 闭合行 `!`。
        if self.kind() == TokenKind::Symbol(Symbol::Bang) {
            self.bump();
        }
        true
    }

    /// 扫描 `pathType`（effect row term 用；不含 `?` / receiver tail）。
    fn scan_path_end(&mut self) -> bool {
        if self.kind() != TokenKind::Ident {
            return false;
        }
        self.bump();
        while self.kind() == TokenKind::Symbol(Symbol::Dot)
            && self.kind_at(self.i + 1) == TokenKind::Ident
        {
            self.bump();
            self.bump();
        }
        if self.kind() == TokenKind::Symbol(Symbol::Lt) {
            return self.scan_type_args_end(false);
        }
        true
    }
}

// ------------------------------------------------------------------
// 名字表
// ------------------------------------------------------------------

pub(crate) fn kw_name(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Public => "`public`",
        Keyword::Internal => "`internal`",
        Keyword::Private => "`private`",
        Keyword::Open => "`open`",
        Keyword::Abstract => "`abstract`",
        Keyword::Sealed => "`sealed`",
        Keyword::Inline => "`inline`",
        Keyword::Override => "`override`",
        Keyword::Operator => "`operator`",
        Keyword::Vararg => "`vararg`",
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
        Keyword::On => "`on`",
        Keyword::With => "`with`",
        Keyword::Perform => "`perform`",
        Keyword::Try => "`try`",
        Keyword::Catch => "`catch`",
        Keyword::Finally => "`finally`",
        Keyword::Do => "`do`",
        Keyword::Return => "`return`",
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

pub(crate) fn is_modifier_keyword(kw: Keyword) -> bool {
    matches!(
        kw,
        Keyword::Public
            | Keyword::Internal
            | Keyword::Private
            | Keyword::Open
            | Keyword::Abstract
            | Keyword::Sealed
            | Keyword::Inline
            | Keyword::Override
            | Keyword::Operator
            | Keyword::Annotation
    )
}

pub(crate) fn sym_name(sym: Symbol) -> &'static str {
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
        Symbol::DotDot => "`..`",
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
