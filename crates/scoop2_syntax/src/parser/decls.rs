//! 声明解析（grammar §3 / §4 / §5.1）。

use scoop2_base::Span;

use crate::ast::decl::*;
use crate::ast::expr::Expr;
use crate::ast::types::TypeRef;
use crate::ast::{AnnotationArg, AnnotationUse, Ident, Modifier, ModifierKind, TypePath};
use crate::token::{Keyword, Symbol, Token, TokenKind};

use super::{PResult, Parser};

/// 函数声明的上下文（影响 body 省略规则与表达式体的边界判断）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunContext {
    TopLevel,
    Member { in_interface: bool, in_effect: bool },
}

#[derive(Debug, Clone, Copy)]
struct TypeBodyContext {
    allow_enum_variants: bool,
    allow_init_blocks: bool,
    allow_secondary_ctors: bool,
    allow_companion_object: bool,
    is_effect: bool,
    is_interface: bool,
}

impl TypeBodyContext {
    fn for_type_kind(kind: TypeKind) -> Self {
        match kind {
            TypeKind::Class => Self {
                allow_enum_variants: false,
                allow_init_blocks: true,
                allow_secondary_ctors: true,
                allow_companion_object: true,
                is_effect: false,
                is_interface: false,
            },
            TypeKind::Interface => Self {
                allow_enum_variants: false,
                allow_init_blocks: false,
                allow_secondary_ctors: false,
                allow_companion_object: false,
                is_effect: false,
                is_interface: true,
            },
            TypeKind::Struct | TypeKind::Enum | TypeKind::Effect => Self {
                allow_enum_variants: matches!(kind, TypeKind::Enum),
                allow_init_blocks: false,
                allow_secondary_ctors: false,
                allow_companion_object: false,
                is_effect: matches!(kind, TypeKind::Effect),
                is_interface: false,
            },
        }
    }

    const OBJECT: Self = Self {
        allow_enum_variants: false,
        allow_init_blocks: true,
        allow_secondary_ctors: false,
        allow_companion_object: false,
        is_effect: false,
        is_interface: false,
    };
}

impl<'a> Parser<'a> {
    // --------------------------------------------------------------
    // §4 注解与修饰符
    // --------------------------------------------------------------

    /// `declPrefix ::= (annotationUse | modifier)*`；修饰符排序去重（§3）。
    pub(crate) fn parse_decl_prefix(&mut self) -> PResult<(Vec<AnnotationUse>, Vec<Modifier>)> {
        let mut annotations = Vec::new();
        let mut modifiers: Vec<Modifier> = Vec::new();

        loop {
            if self.at_sym(Symbol::At) {
                annotations.push(self.parse_annotation_use()?);
                continue;
            }

            let kind = match self.peek().kind {
                TokenKind::Keyword(Keyword::Public) => ModifierKind::Public,
                TokenKind::Keyword(Keyword::Internal) => ModifierKind::Internal,
                TokenKind::Keyword(Keyword::Private) => ModifierKind::Private,
                TokenKind::Keyword(Keyword::Open) => ModifierKind::Open,
                TokenKind::Keyword(Keyword::Abstract) => ModifierKind::Abstract,
                TokenKind::Keyword(Keyword::Sealed) => ModifierKind::Sealed,
                TokenKind::Keyword(Keyword::Override) => ModifierKind::Override,
                TokenKind::Keyword(Keyword::Operator) => ModifierKind::Operator,
                TokenKind::Keyword(Keyword::Annotation) => ModifierKind::Annotation,
                TokenKind::Keyword(Keyword::Inline) => {
                    // `inline` 已移除（§10）：消费、记录、继续。
                    let tok = self.bump();
                    self.err_inline_removed(tok.span);
                    continue;
                }
                _ => break,
            };

            let tok = self.bump();
            modifiers.push(Modifier {
                kind,
                span: tok.span,
            });
        }

        modifiers.sort_by_key(|m| modifier_rank(m.kind));
        modifiers.dedup_by(|a, b| a.kind == b.kind);

        Ok((annotations, modifiers))
    }

    /// `annotationUse ::= '@' (IDENT ':')? identPath ('(' args ')')?`（§4）。
    pub(crate) fn parse_annotation_use(&mut self) -> PResult<AnnotationUse> {
        let at = self.expect_sym(Symbol::At)?;
        let id = self.nid();
        let start = at.span.start;

        let first = self.expect_ident("注解名")?;
        let mut target = None;
        let mut segments = vec![self.ident(first)];
        let mut end = first.span.end;

        // use-site target：`@target:Name`
        if self.at_sym(Symbol::Colon) {
            target = segments.pop();
            self.bump(); // `:`
            let name = self.expect_ident("注解名")?;
            end = name.span.end;
            segments.push(self.ident(name));
        }

        while self.at_sym(Symbol::Dot) {
            self.bump(); // `.`
            let seg = self.expect_ident("注解名")?;
            end = seg.span.end;
            segments.push(self.ident(seg));
        }

        let mut args = Vec::new();
        if self.at_sym(Symbol::LParen) {
            self.bump(); // `(`
            if !self.at_sym(Symbol::RParen) {
                loop {
                    args.push(self.parse_annotation_arg()?);
                    if self.eat_sym(Symbol::Comma) {
                        if self.at_sym(Symbol::RParen) {
                            break;
                        }
                        continue;
                    }
                    break;
                }
            }
            let close = self.expect_sym(Symbol::RParen)?;
            end = close.span.end;
        }

        Ok(AnnotationUse {
            id,
            span: Span::new(start, end),
            target,
            path: TypePath {
                segments,
                span: Span::new(start, end),
            },
            args,
        })
    }

    fn parse_annotation_arg(&mut self) -> PResult<AnnotationArg> {
        let start = self.peek().span.start;

        // 命名实参：`name = value` 总是；`name: value` 仅当下一个不是 `:`（防 `String::class`）。
        let name = if self.at_kind(TokenKind::Ident)
            && (self.at_sym_n(1, Symbol::Eq)
                || (self.at_sym_n(1, Symbol::Colon) && !self.at_sym_n(2, Symbol::Colon)))
        {
            let name_tok = self.bump();
            self.bump(); // `=` / `:`
            Some(self.ident(name_tok))
        } else {
            None
        };

        let value = self.expr()?;
        Ok(AnnotationArg {
            id: self.nid(),
            span: Span::new(start, value.span.end),
            name,
            value,
        })
    }

    // --------------------------------------------------------------
    // §5.1 类型参数 / where 子句 / generic bound
    // --------------------------------------------------------------

    pub(crate) fn parse_type_param_list_opt(&mut self) -> PResult<Option<TypeParamList>> {
        if !self.at_sym(Symbol::Lt) {
            return Ok(None);
        }
        let lt = self.bump();
        let id = self.nid();
        let start = lt.span.start;

        let mut params = Vec::new();
        let mut effect_row: Option<EffectRowParam> = None;

        if self.at_sym(Symbol::Gt) {
            let gt = self.bump();
            return Ok(Some(TypeParamList {
                id,
                span: Span::new(start, gt.span.end),
                params,
                effect_row,
            }));
        }

        loop {
            // `eff E (= Row)?`（上下文关键字；至多一个且必须最后）。
            if self.at_ident_text("eff") {
                let eff_kw = self.bump();
                if effect_row.is_some() {
                    let tok = self.peek();
                    return Err(self.err_expected("泛型参数名（`eff` 只能出现一次）", tok));
                }
                let name_tok = self.expect_ident("effect row 参数名")?;
                let name = self.ident(name_tok);
                let (default, end) = if self.eat_sym(Symbol::Eq) {
                    let row = self.parse_effect_row_expr()?;
                    let end = row.span.end;
                    (Some(row), end)
                } else {
                    (None, name_tok.span.end)
                };
                effect_row = Some(EffectRowParam {
                    id: self.nid(),
                    span: Span::new(eff_kw.span.start, end),
                    name,
                    default,
                });
                if self.eat_sym(Symbol::Comma) && !self.at_sym(Symbol::Gt) {
                    let tok = self.peek();
                    return Err(self.err_expected("`>`（`eff` 参数必须位于泛型列表末尾）", tok));
                }
                break;
            }

            let (variance, variance_start) = match self.peek().kind {
                TokenKind::Keyword(Keyword::In) => {
                    let kw = self.bump();
                    (Some(Variance::In), Some(kw.span.start))
                }
                TokenKind::Keyword(Keyword::Out) => {
                    let kw = self.bump();
                    (Some(Variance::Out), Some(kw.span.start))
                }
                _ => (None, None),
            };

            let name_tok = self.expect_ident("类型参数名")?;
            let name = self.ident(name_tok);
            let (bound, end) = if self.eat_sym(Symbol::Colon) {
                let bound = self.parse_generic_bound()?;
                let end = generic_bound_end(&bound);
                (Some(bound), end)
            } else {
                (None, name_tok.span.end)
            };
            params.push(TypeParam {
                id: self.nid(),
                span: Span::new(variance_start.unwrap_or(name_tok.span.start), end),
                variance,
                name,
                bound,
            });

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::Gt) {
                    break;
                }
                continue;
            }
            break;
        }

        let gt = self.expect_gt_close()?;
        Ok(Some(TypeParamList {
            id,
            span: Span::new(start, gt.span.end),
            params,
            effect_row,
        }))
    }

    pub(crate) fn parse_where_clause_opt(&mut self) -> PResult<Option<WhereClause>> {
        if !self.at_kw(Keyword::Where) {
            return Ok(None);
        }
        let where_kw = self.bump();
        let id = self.nid();
        let start = where_kw.span.start;

        let mut constraints = Vec::new();
        loop {
            let name_tok = self.expect_ident("类型参数名")?;
            let name = self.ident(name_tok);
            self.expect_sym(Symbol::Colon)?;
            let bound = self.parse_generic_bound()?;
            let end = generic_bound_end(&bound);
            constraints.push(WhereConstraint {
                id: self.nid(),
                span: Span::new(name_tok.span.start, end),
                name,
                bound,
            });
            if self.eat_sym(Symbol::Comma) {
                // 允许 `{` / `;` / `}` / EOF 前的尾逗号。
                if self.at_sym(Symbol::LBrace)
                    || self.at_sym(Symbol::Semicolon)
                    || self.at_sym(Symbol::RBrace)
                    || self.at_eof()
                {
                    break;
                }
                continue;
            }
            break;
        }

        let end = constraints
            .last()
            .map(|c| c.span.end)
            .unwrap_or(where_kw.span.end);
        Ok(Some(WhereClause {
            id,
            span: Span::new(start, end),
            constraints,
        }))
    }

    fn parse_generic_bound(&mut self) -> PResult<GenericBound> {
        match self.peek_bound_keyword() {
            Some("ref") => {
                let tok = self.bump();
                Ok(GenericBound::Ref(tok.span))
            }
            Some("value") => {
                let tok = self.bump();
                Ok(GenericBound::Value(tok.span))
            }
            _ => Ok(GenericBound::Type(self.parse_type_ref()?)),
        }
    }

    // --------------------------------------------------------------
    // §3.2 函数（顶层 / 成员共用 funDecl 产生式）
    // --------------------------------------------------------------

    pub(crate) fn parse_fun_decl(&mut self, ctx: FunContext) -> PResult<FunDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        self.expect_kw(Keyword::Fun)?;
        // 类型参数可以在 name 前（`fun <T> f`）或 name 后（`fun f<T>`）；两处至多一处（§3.2）。
        let pre_type_params = self.parse_type_param_list_opt()?;
        let (receiver, name) = self.parse_fun_receiver_and_name()?;
        let type_params = match pre_type_params {
            Some(tp) => Some(tp),
            None => self.parse_type_param_list_opt()?,
        };

        let (_params_span, params) = self.parse_param_list()?;

        let return_ty = if self.eat_sym(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        // effectAnn 与 whereClause 可以任意顺序各出现一次。
        let mut effect = None;
        let mut where_clause = None;
        loop {
            if effect.is_none() && self.eat_sym(Symbol::Slash) {
                effect = Some(self.parse_effect_row_expr()?);
                continue;
            }
            if where_clause.is_none() {
                where_clause = self.parse_where_clause_opt()?;
                if where_clause.is_some() {
                    continue;
                }
            }
            break;
        }

        let in_effect = matches!(
            ctx,
            FunContext::Member {
                in_effect: true,
                ..
            }
        );

        // effect operation：不允许函数体（§3.5）。
        if in_effect {
            if self.at_sym(Symbol::LBrace) {
                let tok = self.peek();
                self.err_expected("effect operation 的签名（不允许有函数体）", tok);
                let _span = self.consume_balanced(Symbol::LBrace, Symbol::RBrace)?;
            } else if self.at_sym(Symbol::Eq) {
                let tok = self.peek();
                self.err_expected("effect operation 的签名（不允许有函数体）", tok);
                self.bump();
                let _ = self.expr();
            }
            return Ok(FunDecl {
                annotations,
                modifiers,
                type_params,
                receiver,
                name,
                params,
                return_ty,
                effect,
                where_clause,
                body: None,
            });
        }

        let body = if self.at_sym(Symbol::LBrace) {
            Some(FunBody::Block(self.parse_block()?))
        } else if self.at_sym(Symbol::Eq) {
            // 表达式体（§3.2 / §14.1）；边界规则同 accessor body（§3.6）。
            self.bump();
            if self.body_boundary(ctx) {
                let tok = self.peek();
                return Err(self.err_expected("表达式（函数体）", tok));
            }
            let expr = self.expr()?;
            self.check_body_boundary(ctx, "函数表达式体");
            Some(FunBody::Expr(Box::new(expr)))
        } else {
            // 缺 body：parser 不再在此报错——保留 body: None 的 partial-but-valid 节点，
            // 由 typecheck 阶段判定 body 省略是否合法（grammar §3.2 注 / §14.7）。
            None
        };

        Ok(FunDecl {
            annotations,
            modifiers,
            type_params,
            receiver,
            name,
            params,
            return_ty,
            effect,
            where_clause,
            body,
        })
    }

    /// body 省略是否合法：abstract 修饰符 / 类型体成员（class/interface） /
    /// effect operation / 或带任意注解的声明（`@Intrinsic` / `@Extern` 等 sysroot
    /// 声明式 API）。
    ///
    /// 顶层非注解函数缺 body 是 parse error；是否允许的具体语义（如非 abstract
    /// class 含无体成员）由 typecheck 负责。
    #[allow(dead_code)]
    fn fun_body_may_be_omitted(
        &self,
        ctx: FunContext,
        annotations: &[AnnotationUse],
        modifiers: &[Modifier],
    ) -> bool {
        if !annotations.is_empty() {
            return true;
        }
        if modifiers.iter().any(|m| m.kind == ModifierKind::Abstract) {
            return true;
        }
        matches!(ctx, FunContext::Member { .. })
    }

    fn body_boundary(&self, ctx: FunContext) -> bool {
        match ctx {
            FunContext::TopLevel => self.at_top_level_boundary(),
            FunContext::Member { .. } => self.at_member_boundary(),
        }
    }

    /// §3.2/§3.6：表达式体之后的 token 必须落在边界上，否则硬错误 + 恢复。
    fn check_body_boundary(&mut self, ctx: FunContext, what: &str) {
        if self.body_boundary(ctx) {
            return;
        }
        let tok = self.peek();
        self.err_trailing(what, tok);
        match ctx {
            FunContext::TopLevel => self.skip_until_top_level_boundary(),
            FunContext::Member { .. } => self.skip_until_member_boundary(),
        }
    }

    /// `receiverAndName`：token 扫描检测扩展接收者，receiver 切片由子 parser 解析（§3.2）。
    fn parse_fun_receiver_and_name(&mut self) -> PResult<(Option<TypeRef>, Ident)> {
        let start_idx = self.i;

        let Some(dot_idx) = self.detect_extension_receiver_dot(start_idx) else {
            let name_tok = self.expect_ident("函数名")?;
            return Ok((None, self.ident(name_tok)));
        };

        let receiver = self.parse_receiver_slice(start_idx, dot_idx)?;

        // fast-forward：跳过 receiver tokens，消费 `. name`。
        self.i = dot_idx;
        self.expect_sym(Symbol::Dot)?;
        let name_tok = self.expect_ident("函数名")?;
        Ok((Some(receiver), self.ident(name_tok)))
    }

    /// 用子 parser 把 `[start_idx, dot_idx)` 的 token 切片解析为 `typeRef`。
    fn parse_receiver_slice(&mut self, start_idx: usize, dot_idx: usize) -> PResult<TypeRef> {
        let mut receiver_tokens: Vec<Token> = self.tokens[start_idx..dot_idx].to_vec();
        let eof_pos = receiver_tokens
            .last()
            .map(|t| t.span.end)
            .unwrap_or_else(|| self.tokens[dot_idx].span.start);
        receiver_tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(eof_pos, eof_pos),
        });

        let (merged, trailing_ok, sub_diags, sub_depth) = {
            let mut sub = self.sub_parser(receiver_tokens);
            let result = sub.parse_type_ref();
            let trailing_ok = result.is_ok() && sub.at_eof();
            if result.is_ok() && !trailing_ok {
                let tok = sub.peek();
                sub.err_expected("receiver 类型结束", tok);
            }
            (
                result.ok(),
                trailing_ok,
                std::mem::take(&mut sub.diagnostics),
                sub.depth,
            )
        };
        self.diagnostics.extend(sub_diags);
        self.depth = sub_depth;

        match merged {
            Some(ty) if trailing_ok => Ok(ty),
            _ => Err(super::Abort),
        }
    }

    /// 检测扩展函数 receiver 的 `.`（grammar §3.2 注：token 扫描，§12.5）。
    fn detect_extension_receiver_dot(&self, start_idx: usize) -> Option<usize> {
        // 1) 在 depth-0 找参数列表的 `(`（其前一个 token 必须是 ident 或 `>`/`>>`）。
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;

        let mut params_lparen_idx: Option<usize> = None;

        let mut idx = start_idx;
        while let Some(tok) = self.tokens.get(idx) {
            match tok.kind {
                TokenKind::Eof => break,
                TokenKind::Symbol(sym) => match sym {
                    Symbol::Lt => depth_angle += 1,
                    Symbol::Gt => depth_angle = depth_angle.saturating_sub(1),
                    Symbol::GtGt => depth_angle = depth_angle.saturating_sub(2),
                    Symbol::LParen => {
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0
                        {
                            let prev = self.tokens.get(idx.saturating_sub(1));
                            if prev.is_some_and(|p| {
                                matches!(p.kind, TokenKind::Ident)
                                    || matches!(
                                        p.kind,
                                        TokenKind::Symbol(Symbol::Gt | Symbol::GtGt)
                                    )
                            }) {
                                params_lparen_idx = Some(idx);
                                break;
                            }
                        }
                        depth_paren += 1;
                    }
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    _ => {}
                },
                _ => {}
            }
            idx = idx.saturating_add(1);
        }

        let lparen_idx = params_lparen_idx?;
        let before_lparen = lparen_idx.checked_sub(1)?;

        // 2) 从 `(` 向左回溯 name（可能带 `<T>`）。
        let name_idx = match self.tokens.get(before_lparen)?.kind {
            TokenKind::Ident => before_lparen,
            TokenKind::Symbol(Symbol::Gt | Symbol::GtGt) => {
                let mut depth = 0usize;
                let mut found_name: Option<usize> = None;
                let mut j = before_lparen;
                loop {
                    if j < start_idx {
                        break;
                    }
                    match self.tokens.get(j)?.kind {
                        TokenKind::Symbol(Symbol::Gt) => depth += 1,
                        TokenKind::Symbol(Symbol::GtGt) => depth += 2,
                        TokenKind::Symbol(Symbol::Lt) => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                let name = j.checked_sub(1)?;
                                if self.tokens.get(name)?.kind != TokenKind::Ident {
                                    return None;
                                }
                                found_name = Some(name);
                                break;
                            }
                        }
                        _ => {}
                    }
                    if j == 0 {
                        break;
                    }
                    j -= 1;
                }
                found_name?
            }
            _ => return None,
        };

        // 3) `ReceiverType . name` 形态。
        let dot_idx = name_idx.checked_sub(1)?;
        if dot_idx < start_idx {
            return None;
        }
        if self.tokens.get(dot_idx)?.kind != TokenKind::Symbol(Symbol::Dot) {
            return None;
        }
        Some(dot_idx)
    }

    // --------------------------------------------------------------
    // §3.1 typealias
    // --------------------------------------------------------------

    pub(crate) fn parse_typealias_decl(&mut self) -> PResult<TypeAliasDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        self.expect_kw(Keyword::Typealias)?;
        let name_tok = self.expect_ident("类型别名名")?;
        let name = self.ident(name_tok);

        let type_params = self.parse_type_param_list_opt()?;
        if let Some(tp) = &type_params
            && let Some(eff) = &tp.effect_row
        {
            // typealias 不支持 `eff` 参数（§3.1）。
            return Err(self.err_expected(
                "类型参数名（typealias 的泛型列表不支持 `eff` 参数）",
                Token {
                    kind: TokenKind::Ident,
                    span: eff.span,
                },
            ));
        }

        self.expect_sym(Symbol::Eq)?;
        let ty = self.parse_type_ref()?;

        Ok(TypeAliasDecl {
            annotations,
            modifiers,
            name,
            type_params,
            ty,
        })
    }

    // --------------------------------------------------------------
    // §3.3 顶层 val/var
    // --------------------------------------------------------------

    pub(crate) fn parse_top_level_val_decl(&mut self) -> PResult<ValDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        let kw = self.bump();
        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ValKind::Var,
            _ => {
                let tok = self.peek();
                return Err(self.err_expected_token("`val` / `var`", tok));
            }
        };

        // `var` 解构：专用错误（§3.3；消费平衡分组恢复）。
        if kind == ValKind::Var
            && (self.at_sym(Symbol::LParen)
                || self.looks_like_struct_pattern_ahead()
                || self.looks_like_variant_pattern_ahead())
        {
            let tok = self.peek();
            while !self.at_eof() && !self.at_sym(Symbol::LParen) && !self.at_sym(Symbol::LBrace) {
                self.bump();
            }
            if self.at_sym(Symbol::LParen) {
                let _ = self.consume_balanced(Symbol::LParen, Symbol::RParen)?;
            } else if self.at_sym(Symbol::LBrace) {
                let _ = self.consume_balanced(Symbol::LBrace, Symbol::RBrace)?;
            }
            return Err(self.err_expected("变量名（`var` 不支持解构绑定）", tok));
        }

        let should_parse_pattern = kind == ValKind::Val
            && (self.at_sym(Symbol::LParen)
                || self.looks_like_struct_pattern_ahead()
                || self.looks_like_variant_pattern_ahead());

        let binding = if should_parse_pattern {
            ValBinding::Pattern(self.parse_pattern()?)
        } else {
            let name_tok = self.expect_ident("变量名")?;
            ValBinding::Name(self.ident(name_tok))
        };

        // `:` 类型标注：普通名字绑定与解构绑定均允许（§3.3 + 顶层 val pattern）。
        let ty = if self.eat_sym(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        let init = if self.eat_sym(Symbol::Eq) {
            // `object` 虽属于顶层 item 起始集合，但在表达式位置会得到
            // 专用错误（§10 anonymous object），不能在这里当作下一个 item。
            if self.at_eof()
                || self.at_sym(Symbol::Semicolon)
                || (self.is_top_level_item_start() && !self.at_kw(Keyword::Object))
            {
                let tok = self.peek();
                return Err(self.err_expected("表达式（initializer）", tok));
            }
            let expr = self.expr()?;
            // §3.3 normative：initializer 必须干净结束，否则硬错误 + 恢复。
            if !self.at_top_level_boundary() {
                let tok = self.peek();
                self.err_trailing("顶层 val/var 的初始化表达式", tok);
                self.skip_until_top_level_boundary();
            }
            Some(expr)
        } else if matches!(binding, ValBinding::Pattern(_)) {
            let tok = self.peek();
            return Err(self.err_expected("`=`（解构绑定需要 initializer）", tok));
        } else {
            None
        };

        Ok(ValDecl {
            annotations,
            modifiers,
            kind,
            binding,
            ty,
            init,
        })
    }

    // --------------------------------------------------------------
    // §3.7 扩展属性（顶层）
    // --------------------------------------------------------------

    pub(crate) fn parse_extension_property_decl(&mut self) -> PResult<ExtensionPropertyDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        let kw = self.bump();
        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ValKind::Var,
            _ => {
                let tok = self.peek();
                return Err(self.err_expected_token("`val` / `var`", tok));
            }
        };

        let type_params = self.parse_type_param_list_opt()?;

        // receiver：token 扫描找 `Receiver . name :` 的 `.`，子 parser 解析切片。
        let start_idx = self.i;
        let Some(dot_idx) = self.detect_extension_property_receiver_dot(start_idx) else {
            let tok = self.peek();
            return Err(self.err_expected("扩展属性 receiver（形如 `ReceiverType.name`）", tok));
        };
        let receiver = self.parse_receiver_slice(start_idx, dot_idx)?;
        self.i = dot_idx;
        self.expect_sym(Symbol::Dot)?;
        let name_tok = self.expect_ident("属性名")?;
        let name = self.ident(name_tok);

        // 显式 `: Type` 必有（§3.7）。
        if !self.at_sym(Symbol::Colon) {
            let tok = self.peek();
            return Err(self.err_expected_token("`:`", tok));
        }
        self.bump();
        let ty = self.parse_type_ref()?;

        let init = if self.eat_sym(Symbol::Eq) {
            if self.at_eof()
                || self.at_sym(Symbol::Semicolon)
                || self.is_top_level_item_start()
                || self.is_property_accessor_start()
            {
                let tok = self.peek();
                return Err(self.err_expected("表达式（initializer）", tok));
            }
            let expr = self.expr()?;
            if !(self.at_eof()
                || self.at_sym(Symbol::Semicolon)
                || self.is_top_level_item_start()
                || self.is_property_accessor_start())
            {
                let tok = self.peek();
                self.err_trailing("扩展属性的初始化表达式", tok);
                self.skip_until_top_level_boundary();
            }
            Some(expr)
        } else {
            None
        };

        let mut accessors = Vec::new();
        while self.is_property_accessor_start() {
            accessors.push(self.parse_accessor_decl()?);
        }

        Ok(ExtensionPropertyDecl {
            annotations,
            modifiers,
            kind,
            type_params,
            receiver,
            name,
            ty,
            init,
            accessors,
        })
    }

    fn detect_extension_property_receiver_dot(&self, start_idx: usize) -> Option<usize> {
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;

        let mut colon_idx: Option<usize> = None;
        let mut idx = start_idx;
        while let Some(tok) = self.tokens.get(idx) {
            match tok.kind {
                TokenKind::Eof => break,
                TokenKind::Symbol(sym) => match sym {
                    Symbol::Lt => depth_angle += 1,
                    Symbol::Gt => depth_angle = depth_angle.saturating_sub(1),
                    Symbol::GtGt => depth_angle = depth_angle.saturating_sub(2),
                    Symbol::LParen => depth_paren += 1,
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    Symbol::Eq | Symbol::Semicolon
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0 =>
                    {
                        break;
                    }
                    Symbol::Colon
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && depth_angle == 0 =>
                    {
                        colon_idx = Some(idx);
                        break;
                    }
                    _ => {}
                },
                _ => {}
            }
            idx = idx.saturating_add(1);
        }

        let colon_idx = colon_idx?;
        let name_idx = colon_idx.checked_sub(1)?;
        if self.tokens.get(name_idx)?.kind != TokenKind::Ident {
            return None;
        }
        let dot_idx = name_idx.checked_sub(1)?;
        if self.tokens.get(dot_idx)?.kind != TokenKind::Symbol(Symbol::Dot) {
            return None;
        }
        Some(dot_idx)
    }

    // --------------------------------------------------------------
    // §3.8 参数
    // --------------------------------------------------------------

    pub(crate) fn parse_param_list(&mut self) -> PResult<(Span, Vec<Param>)> {
        let open = self.expect_sym(Symbol::LParen)?;
        let start = open.span.start;

        let mut params = Vec::new();
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok((Span::new(start, close.span.end), params));
        }

        loop {
            let param_start = self.peek().span.start;
            let mut annotations = Vec::new();
            while self.at_sym(Symbol::At) {
                annotations.push(self.parse_annotation_use()?);
            }

            let is_vararg = self.eat_kw(Keyword::Vararg);

            // `var` 允许作为参数名（sysroot intrinsics 如 `addressOf(var: T)`，§3.8）。
            let name_tok = if self.at_kind(TokenKind::Ident) || self.at_kw(Keyword::Var) {
                self.bump()
            } else {
                let tok = self.peek();
                return Err(self.err_expected_ident("参数名", tok));
            };
            let name = self.ident(name_tok);

            let ty = if self.eat_sym(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };

            let default = if self.eat_sym(Symbol::Eq) {
                Some(self.expr()?)
            } else {
                None
            };

            let end = default
                .as_ref()
                .map(|e| e.span.end)
                .or_else(|| ty.as_ref().map(|t| t.span.end))
                .unwrap_or(name_tok.span.end);
            params.push(Param {
                id: self.nid(),
                span: Span::new(param_start, end),
                annotations,
                is_vararg,
                name,
                ty,
                default,
            });

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok((Span::new(start, close.span.end), params))
    }

    /// 主构造参数（§3.4 ctorParam）：额外允许 `val`/`var` 与 `vararg` 任意顺序。
    fn parse_primary_ctor_param_list(&mut self) -> PResult<PrimaryCtorDecl> {
        let open = self.expect_sym(Symbol::LParen)?;
        let id = self.nid();
        let start = open.span.start;

        let mut params = Vec::new();
        if !self.at_sym(Symbol::RParen) {
            loop {
                let param_start = self.peek().span.start;
                let mut annotations = Vec::new();
                while self.at_sym(Symbol::At) {
                    annotations.push(self.parse_annotation_use()?);
                }

                let mut property: Option<ValKind> = None;
                let mut is_vararg = false;
                loop {
                    if self.at_kw(Keyword::Val) {
                        self.bump();
                        property = Some(ValKind::Val);
                        continue;
                    }
                    if self.at_kw(Keyword::Var) {
                        self.bump();
                        property = Some(ValKind::Var);
                        continue;
                    }
                    if self.at_kw(Keyword::Vararg) {
                        self.bump();
                        is_vararg = true;
                        continue;
                    }
                    break;
                }

                let name_tok = self.expect_ident("参数名")?;
                let name = self.ident(name_tok);
                let ty = if self.eat_sym(Symbol::Colon) {
                    Some(self.parse_type_ref()?)
                } else {
                    None
                };
                let default = if self.eat_sym(Symbol::Eq) {
                    Some(self.expr()?)
                } else {
                    None
                };

                let end = default
                    .as_ref()
                    .map(|e| e.span.end)
                    .or_else(|| ty.as_ref().map(|t| t.span.end))
                    .unwrap_or(name_tok.span.end);
                params.push(CtorParam {
                    id: self.nid(),
                    span: Span::new(param_start, end),
                    annotations,
                    property,
                    is_vararg,
                    name,
                    ty,
                    default,
                });

                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok(PrimaryCtorDecl {
            id,
            span: Span::new(start, close.span.end),
            params,
        })
    }

    // --------------------------------------------------------------
    // §3.4 类型声明
    // --------------------------------------------------------------

    pub(crate) fn parse_type_decl(&mut self) -> PResult<TypeDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        let kind = match self.peek().kind {
            TokenKind::Keyword(Keyword::Class) => {
                self.bump();
                TypeKind::Class
            }
            TokenKind::Keyword(Keyword::Interface) => {
                self.bump();
                TypeKind::Interface
            }
            TokenKind::Keyword(Keyword::Struct) => {
                self.bump();
                TypeKind::Struct
            }
            TokenKind::Keyword(Keyword::Enum) => {
                self.bump();
                TypeKind::Enum
            }
            TokenKind::Keyword(Keyword::Effect) => {
                self.bump();
                TypeKind::Effect
            }
            _ => {
                let tok = self.peek();
                return Err(
                    self.err_expected("类型声明关键字（class/interface/struct/enum/effect）", tok)
                );
            }
        };

        let name_tok = self.expect_ident("类型名")?;
        let name = self.ident(name_tok);

        let type_params = self.parse_type_param_list_opt()?;

        let primary_ctor = if self.at_sym(Symbol::LParen) {
            Some(self.parse_primary_ctor_param_list()?)
        } else {
            None
        };

        let supertypes = if self.eat_sym(Symbol::Colon) {
            self.parse_supertype_list()?
        } else if matches!(kind, TypeKind::Class | TypeKind::Interface)
            && self.at_kind(TokenKind::Ident)
        {
            // `class C Base`：缺少 `:`（§3.4 targeted error）。
            let tok = self.peek();
            return Err(self.err_expected_token("`:`", tok));
        } else {
            Vec::new()
        };

        let where_clause = self.parse_where_clause_opt()?;

        // header 终止（§3.4 normative）：下一个必须是 `{` / `}` / 边界，否则硬错误 + 恢复。
        let body = if self.at_sym(Symbol::LBrace) {
            Some(self.parse_type_body(TypeBodyContext::for_type_kind(kind))?)
        } else if self.at_header_boundary() {
            None
        } else {
            let tok = self.peek();
            self.err_trailing("类型声明头", tok);
            self.skip_type_header_tail();
            if self.at_sym(Symbol::LBrace) {
                Some(self.parse_type_body(TypeBodyContext::for_type_kind(kind))?)
            } else {
                None
            }
        };

        Ok(TypeDecl {
            annotations,
            modifiers,
            kind,
            name,
            type_params,
            primary_ctor,
            supertypes,
            where_clause,
            body,
        })
    }

    /// 声明头边界：body `{` / 外层 `}` / `;` / EOF / item / member 起始。
    fn at_header_boundary(&self) -> bool {
        self.at_eof()
            || self.at_sym(Symbol::RBrace)
            || self.at_sym(Symbol::Semicolon)
            || self.is_top_level_item_start()
            || self.is_type_member_start()
    }

    /// header 尾部恢复：跳到 `{` 或边界。
    fn skip_type_header_tail(&mut self) {
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        while !self.at_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.at_sym(Symbol::LBrace)
                    || self.at_sym(Symbol::RBrace)
                    || self.at_sym(Symbol::Semicolon)
                    || self.is_top_level_item_start()
                    || self.is_type_member_start())
            {
                break;
            }
            let tok = self.bump();
            match tok.kind {
                TokenKind::Symbol(Symbol::LParen) => depth_paren += 1,
                TokenKind::Symbol(Symbol::RParen) => depth_paren = depth_paren.saturating_sub(1),
                TokenKind::Symbol(Symbol::LBrace) => depth_brace += 1,
                TokenKind::Symbol(Symbol::RBrace) => depth_brace = depth_brace.saturating_sub(1),
                TokenKind::Symbol(Symbol::LBracket) => depth_bracket += 1,
                TokenKind::Symbol(Symbol::RBracket) => {
                    depth_bracket = depth_bracket.saturating_sub(1)
                }
                _ => {}
            }
        }
    }

    fn parse_supertype_list(&mut self) -> PResult<Vec<SuperType>> {
        let mut supertypes = Vec::new();
        loop {
            let ty = self.parse_type_ref()?;
            let start = ty.span.start;
            let mut end = ty.span.end;

            // `Base(args)`：构造实参（按调用实参列表规则）。
            let args = if self.at_sym(Symbol::LParen) {
                let (args_span, args) = self.parse_call_arg_list()?;
                end = args_span.end;
                args
            } else {
                Vec::new()
            };

            supertypes.push(SuperType {
                id: self.nid(),
                span: Span::new(start, end),
                ty,
                args,
            });

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::LBrace) || self.at_sym(Symbol::RBrace) || self.at_eof() {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(supertypes)
    }

    // --------------------------------------------------------------
    // §3.4 object / companion object
    // --------------------------------------------------------------

    pub(crate) fn parse_object_decl(&mut self, companion: bool) -> PResult<ObjectDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        if companion {
            self.expect_kw(Keyword::Companion)?;
            self.expect_kw(Keyword::Object)?;
        } else {
            self.expect_kw(Keyword::Object)?;
        }

        let name = if self.at_kind(TokenKind::Ident) {
            let tok = self.bump();
            Some(self.ident(tok))
        } else if companion {
            None
        } else {
            let tok = self.peek();
            return Err(self.err_expected_ident("object 名", tok));
        };

        let supertypes = if self.eat_sym(Symbol::Colon) {
            self.parse_supertype_list()?
        } else {
            Vec::new()
        };

        let body = if self.at_sym(Symbol::LBrace) {
            Some(self.parse_type_body(TypeBodyContext::OBJECT)?)
        } else if self.at_header_boundary() {
            None
        } else {
            let tok = self.peek();
            self.err_trailing("object 声明头", tok);
            self.skip_type_header_tail();
            if self.at_sym(Symbol::LBrace) {
                Some(self.parse_type_body(TypeBodyContext::OBJECT)?)
            } else {
                None
            }
        };

        Ok(ObjectDecl {
            annotations,
            modifiers,
            name,
            companion,
            supertypes,
            body,
        })
    }

    // --------------------------------------------------------------
    // §3.4 类型体成员
    // --------------------------------------------------------------

    fn parse_type_body(&mut self, ctx: TypeBodyContext) -> PResult<TypeBody> {
        let open = self.expect_sym(Symbol::LBrace)?;
        let id = self.nid();
        let start = open.span.start;

        let mut members = Vec::new();
        while !self.at_eof() && !self.at_sym(Symbol::RBrace) {
            // 空成员 `;`。
            if self.eat_sym(Symbol::Semicolon) {
                continue;
            }

            let head = self.peek_after_modifiers();

            // `init { ... }`（上下文关键字；class / object）。
            if head.kind == TokenKind::Ident && self.token_text(head) == "init" {
                if ctx.allow_init_blocks {
                    match self.parse_init_block_decl() {
                        Ok(decl) => members.push(self.member(TypeMemberKind::InitBlock(decl))),
                        Err(_abort) => self.skip_type_member_fallback(),
                    }
                    continue;
                }
            } else if head.kind == TokenKind::Ident && self.token_text(head) == "constructor" {
                if ctx.allow_secondary_ctors {
                    match self.parse_secondary_ctor_decl() {
                        Ok(decl) => members.push(self.member(TypeMemberKind::SecondaryCtor(decl))),
                        Err(_abort) => self.skip_type_member_fallback(),
                    }
                    continue;
                }
            } else if ctx.allow_enum_variants && head.kind == TokenKind::Ident {
                match self.parse_enum_variant_decl() {
                    Ok(decl) => members.push(self.member(TypeMemberKind::EnumVariant(decl))),
                    Err(_abort) => self.skip_type_member_fallback(),
                }
                // enum variants 用 `,` 分隔（每个 variant 后至多消费一个）。
                let _ = self.eat_sym(Symbol::Comma);
                continue;
            }

            match head.kind {
                TokenKind::Keyword(Keyword::Companion) => {
                    if !ctx.allow_companion_object {
                        self.err_expected(
                            "class 内的 `companion object`（仅 class 允许 companion）",
                            head,
                        );
                    }
                    match self.parse_object_decl(true) {
                        Ok(decl) => members.push(self.member(TypeMemberKind::Object(decl))),
                        Err(_abort) => self.skip_type_member_fallback(),
                    }
                }
                TokenKind::Keyword(Keyword::Object) => match self.parse_object_decl(false) {
                    Ok(decl) => members.push(self.member(TypeMemberKind::Object(decl))),
                    Err(_abort) => self.skip_type_member_fallback(),
                },
                TokenKind::Keyword(Keyword::Val | Keyword::Var) => {
                    match self.parse_property_decl() {
                        Ok(decl) => members.push(self.member(TypeMemberKind::Property(decl))),
                        Err(_abort) => self.skip_type_member_fallback(),
                    }
                }
                TokenKind::Keyword(Keyword::Fun) => {
                    let fun_ctx = FunContext::Member {
                        in_interface: ctx.is_interface,
                        in_effect: ctx.is_effect,
                    };
                    match self.parse_fun_decl(fun_ctx) {
                        Ok(decl) => members.push(self.member(TypeMemberKind::Fun(decl))),
                        Err(_abort) => self.skip_type_member_fallback(),
                    }
                }
                TokenKind::Keyword(
                    Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect,
                ) => match self.parse_type_decl() {
                    Ok(decl) => members.push(self.member(TypeMemberKind::Type(decl))),
                    Err(_abort) => self.skip_type_member_fallback(),
                },
                _ => {
                    // 未知 member 形态：先记录硬错误，再平衡括号恢复（§3.4）。
                    self.err_expected(
                        "类型体成员（fun/val/var/类型声明/object/init/constructor）",
                        head,
                    );
                    self.skip_type_member_fallback();
                }
            }
        }

        let close = self.expect_sym(Symbol::RBrace)?;
        Ok(TypeBody {
            id,
            span: Span::new(start, close.span.end),
            members,
        })
    }

    fn member(&mut self, kind: TypeMemberKind) -> TypeMember {
        TypeMember {
            id: self.nid(),
            span: member_kind_span(&kind),
            kind,
        }
    }

    fn parse_init_block_decl(&mut self) -> PResult<InitBlockDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;
        if !self.at_ident_text("init") {
            let tok = self.peek();
            return Err(self.err_expected("`init`", tok));
        }
        self.bump(); // `init`
        let body = self.parse_block()?;
        Ok(InitBlockDecl {
            annotations,
            modifiers,
            body,
        })
    }

    fn parse_secondary_ctor_decl(&mut self) -> PResult<SecondaryCtorDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;
        if !self.at_ident_text("constructor") {
            let tok = self.peek();
            return Err(self.err_expected("`constructor`", tok));
        }
        let kw_span = self.peek().span;
        self.bump(); // `constructor`

        let type_params = self.parse_type_param_list_opt()?;
        if let Some(tp) = &type_params
            && let Some(eff) = &tp.effect_row
        {
            // 次构造不支持 `eff` 参数（§3.4 注）。
            self.err_expected(
                "constructor type parameter（constructor 不支持 effect row parameter）",
                Token {
                    kind: TokenKind::Ident,
                    span: eff.span,
                },
            );
        }

        let (_params_span, params) = self.parse_param_list()?;
        let where_clause = self.parse_where_clause_opt()?;

        let delegation = if self.at_sym(Symbol::Colon) {
            Some(self.parse_ctor_delegation_call()?)
        } else {
            None
        };

        // body 块必有（§3.4）。
        let body = self.parse_block()?;

        Ok(SecondaryCtorDecl {
            annotations,
            span: kw_span,
            modifiers,
            type_params,
            params,
            where_clause,
            delegation,
            body,
        })
    }

    fn parse_ctor_delegation_call(&mut self) -> PResult<CtorDelegation> {
        self.expect_sym(Symbol::Colon)?;

        let tok = self.peek();
        let kind = if tok.kind == TokenKind::Ident {
            match self.token_text(tok) {
                "this" => CtorDelegationKind::This,
                "super" => CtorDelegationKind::Super,
                _ => return Err(self.err_expected("`this` / `super`", tok)),
            }
        } else {
            return Err(self.err_expected("`this` / `super`", tok));
        };
        self.bump();

        let (args_span, args) = self.parse_call_arg_list()?;
        Ok(CtorDelegation {
            span: Span::new(tok.span.start, args_span.end),
            kind,
            args,
        })
    }

    fn parse_enum_variant_decl(&mut self) -> PResult<EnumVariantDecl> {
        let (annotations, _modifiers) = self.parse_decl_prefix()?;

        let name_tok = self.expect_ident("enum variant 名")?;
        let name = self.ident(name_tok);

        let mut fields = Vec::new();
        let mut discriminant: Option<Expr> = None;

        if self.at_sym(Symbol::LParen) {
            self.bump(); // `(`
            if !self.at_sym(Symbol::RParen) {
                loop {
                    // variant 字段要求 `val name: T`（无默认值、无 `var`，§3.4）。
                    if !self.at_kw(Keyword::Val) {
                        let tok = self.peek();
                        return Err(self.err_expected_token("`val`", tok));
                    }
                    self.bump();
                    let field_tok = self.expect_ident("字段名")?;
                    let field_name = self.ident(field_tok);
                    if !self.eat_sym(Symbol::Colon) {
                        let tok = self.peek();
                        return Err(self.err_expected_token("`:`", tok));
                    }
                    let ty = self.parse_type_ref()?;
                    fields.push(EnumVariantField {
                        id: self.nid(),
                        span: Span::new(field_tok.span.start, ty.span.end),
                        name: field_name,
                        ty,
                    });
                    if self.eat_sym(Symbol::Comma) {
                        if self.at_sym(Symbol::RParen) {
                            break;
                        }
                        continue;
                    }
                    break;
                }
            }
            self.expect_sym(Symbol::RParen)?;
        }

        // `= expr` 判别值（§3.4）。
        if self.eat_sym(Symbol::Eq) {
            discriminant = Some(self.expr()?);
        }

        Ok(EnumVariantDecl {
            annotations,
            name,
            fields,
            discriminant,
        })
    }

    // --------------------------------------------------------------
    // §3.6 属性（类型体成员）
    // --------------------------------------------------------------

    fn parse_property_decl(&mut self) -> PResult<PropertyDecl> {
        let (annotations, modifiers) = self.parse_decl_prefix()?;

        let kw = self.bump();
        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ValKind::Var,
            _ => {
                let tok = self.peek();
                return Err(self.err_expected_token("`val` / `var`", tok));
            }
        };

        let name_tok = self.expect_ident("属性名")?;
        let name = self.ident(name_tok);

        let mut ty: Option<TypeRef> = None;
        let mut delegate: Option<Expr> = None;
        let mut init: Option<Expr> = None;

        if self.eat_sym(Symbol::Colon) {
            ty = Some(self.parse_type_ref()?);

            // delegated property：`by expr`（`by` 上下文关键字；与 init/accessors 互斥）。
            if self.at_ident_text("by") {
                self.bump();
                if self.at_member_boundary() {
                    let tok = self.peek();
                    return Err(self.err_expected("表达式（delegate）", tok));
                }
                let expr = self.expr()?;
                if !self.at_member_boundary() {
                    let tok = self.peek();
                    self.err_trailing("委托属性的 delegate 表达式", tok);
                    self.skip_until_member_boundary();
                }
                delegate = Some(expr);
            } else if self.eat_sym(Symbol::Eq) {
                init = Some(self.parse_property_init()?);
            }
        } else if self.eat_sym(Symbol::Eq) {
            // `: T` 可省略（仅当有 `=` initializer，§3.6 normative）。
            init = Some(self.parse_property_init()?);
        } else {
            // 既无 `: T` 也无 `= init`：targeted 错误，节点保留（partial-but-valid）。
            let tok = self.peek();
            self.err_expected(
                "`: 类型` 或 `= 初始化表达式`（属性需要类型标注或初始化）",
                tok,
            );
        }

        // accessors（delegated 后不允许，§3.6）。
        let mut accessors = Vec::new();
        if delegate.is_some() && self.is_property_accessor_start() {
            let tok = self.peek();
            return Err(self.err_expected("委托属性不支持 accessors（请移除 `get/set`）", tok));
        }
        while delegate.is_none() && self.is_property_accessor_start() {
            accessors.push(self.parse_accessor_decl()?);
        }

        Ok(PropertyDecl {
            annotations,
            modifiers,
            kind,
            name,
            ty,
            delegate,
            init,
            accessors,
        })
    }

    fn parse_property_init(&mut self) -> PResult<Expr> {
        if self.at_member_boundary() {
            let tok = self.peek();
            return Err(self.err_expected("表达式（initializer）", tok));
        }
        let expr = self.expr()?;
        if !self.at_member_boundary() {
            let tok = self.peek();
            self.err_trailing("属性的初始化表达式", tok);
            self.skip_until_member_boundary();
        }
        Ok(expr)
    }

    fn parse_accessor_decl(&mut self) -> PResult<AccessorDecl> {
        let name_tok = self.expect_ident("accessor 名称（get/set）")?;
        let id = self.nid();
        let start = name_tok.span.start;
        let kind = match self.token_text(name_tok) {
            "get" => AccessorKind::Get,
            "set" => AccessorKind::Set,
            _ => {
                return Err(self.err_expected("`get` / `set`", name_tok));
            }
        };

        self.expect_sym(Symbol::LParen)?;
        let (param, param_ty) = if kind == AccessorKind::Set {
            let tok = self.expect_ident("setter 参数名")?;
            let ident = self.ident(tok);
            let ty = if self.eat_sym(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            (Some(ident), ty)
        } else {
            if !self.at_sym(Symbol::RParen) {
                let tok = self.peek();
                return Err(self.err_expected_token("`)`", tok));
            }
            (None, None)
        };
        self.expect_sym(Symbol::RParen)?;

        let body = if self.eat_sym(Symbol::Eq) {
            if self.at_member_boundary() {
                let tok = self.peek();
                return Err(self.err_expected("表达式（accessor body）", tok));
            }
            let expr = self.expr()?;
            // §3.6 normative：accessor `= expr` body 必须干净结束。
            if !self.at_member_boundary() {
                let tok = self.peek();
                self.err_trailing("accessor 的表达式体", tok);
                self.skip_until_member_boundary();
            }
            AccessorBody::Expr(Box::new(expr))
        } else if self.at_sym(Symbol::LBrace) {
            AccessorBody::Block(self.parse_block()?)
        } else {
            let tok = self.peek();
            return Err(self.err_expected("`=` 或 `{ ... }`（accessor body）", tok));
        };

        let end = match &body {
            AccessorBody::Block(b) => b.span.end,
            AccessorBody::Expr(e) => e.span.end,
        };
        Ok(AccessorDecl {
            id,
            span: Span::new(start, end),
            kind,
            param,
            param_ty,
            body,
        })
    }

    // --------------------------------------------------------------
    // 辅助
    // --------------------------------------------------------------

    /// package 的 `identPath`。
    pub(crate) fn parse_dotted_path(&mut self) -> PResult<TypePath> {
        let first = self.expect_ident("标识符")?;
        let start = first.span.start;
        let mut segments = vec![self.ident(first)];
        while self.at_sym(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            self.bump(); // `.`
            let seg = self.bump();
            segments.push(self.ident(seg));
        }
        let end = segments
            .last()
            .map(|s| s.span.end)
            .unwrap_or(first.span.end);
        Ok(TypePath {
            segments,
            span: Span::new(start, end),
        })
    }
}

fn modifier_rank(kind: ModifierKind) -> u8 {
    match kind {
        ModifierKind::Public => 0,
        ModifierKind::Internal => 1,
        ModifierKind::Private => 2,
        ModifierKind::Open => 3,
        ModifierKind::Abstract => 4,
        ModifierKind::Sealed => 5,
        ModifierKind::Override => 6,
        ModifierKind::Operator => 7,
        ModifierKind::Annotation => 8,
    }
}

fn generic_bound_end(bound: &GenericBound) -> usize {
    match bound {
        GenericBound::Ref(span) | GenericBound::Value(span) => span.end,
        GenericBound::Type(ty) => ty.span.end,
    }
}

fn member_kind_span(kind: &TypeMemberKind) -> Span {
    // member span 覆盖其声明头到结尾；这里没有每个 payload 的 span 字段，
    // 取关键组成的最大区间。
    match kind {
        TypeMemberKind::InitBlock(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.body.span.start);
            Span::new(start, d.body.span.end)
        }
        TypeMemberKind::SecondaryCtor(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.span.start);
            Span::new(start, d.body.span.end)
        }
        TypeMemberKind::EnumVariant(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.name.span.start);
            let end = d
                .discriminant
                .as_ref()
                .map(|e| e.span.end)
                .or_else(|| d.fields.last().map(|f| f.span.end))
                .unwrap_or(d.name.span.end);
            Span::new(start, end)
        }
        TypeMemberKind::Object(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .or_else(|| d.name.as_ref().map(|n| n.span.start))
                .unwrap_or(0);
            let end = d
                .body
                .as_ref()
                .map(|b| b.span.end)
                .or_else(|| d.supertypes.last().map(|s| s.span.end))
                .or_else(|| d.name.as_ref().map(|n| n.span.end))
                .unwrap_or(start);
            Span::new(start, end)
        }
        TypeMemberKind::Property(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.name.span.start);
            let end = d
                .accessors
                .last()
                .map(|a| a.span.end)
                .or_else(|| d.init.as_ref().map(|e| e.span.end))
                .or_else(|| d.delegate.as_ref().map(|e| e.span.end))
                .or_else(|| d.ty.as_ref().map(|t| t.span.end))
                .unwrap_or(d.name.span.end);
            Span::new(start, end)
        }
        TypeMemberKind::Fun(d) => fun_decl_span(d),
        TypeMemberKind::Type(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.name.span.start);
            let end = d
                .body
                .as_ref()
                .map(|b| b.span.end)
                .or_else(|| d.where_clause.as_ref().map(|w| w.span.end))
                .or_else(|| d.supertypes.last().map(|s| s.span.end))
                .or_else(|| d.primary_ctor.as_ref().map(|c| c.span.end))
                .or_else(|| d.type_params.as_ref().map(|t| t.span.end))
                .unwrap_or(d.name.span.end);
            Span::new(start, end)
        }
    }
}

pub(crate) fn fun_decl_span(d: &FunDecl) -> Span {
    let start = d
        .annotations
        .first()
        .map(|a| a.span.start)
        .unwrap_or(d.name.span.start);
    let end = match &d.body {
        Some(FunBody::Block(b)) => b.span.end,
        Some(FunBody::Expr(e)) => e.span.end,
        None => d
            .where_clause
            .as_ref()
            .map(|w| w.span.end)
            .or_else(|| d.effect.as_ref().map(|r| r.span.end))
            .or_else(|| d.return_ty.as_ref().map(|t| t.span.end))
            .or_else(|| d.params.last().map(|p| p.span.end))
            .unwrap_or(d.name.span.end),
    };
    Span::new(start, end)
}
