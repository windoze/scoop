//! 顶层声明解析：package/import/fun/type decl。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, Token, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    fn parse_modifiers(&mut self) -> Vec<ast::Modifier> {
        let mut modifiers = Vec::new();

        loop {
            let modifier = match self.peek().kind {
                TokenKind::Keyword(Keyword::Public) => ast::Modifier::Public,
                TokenKind::Keyword(Keyword::Internal) => ast::Modifier::Internal,
                TokenKind::Keyword(Keyword::Private) => ast::Modifier::Private,
                TokenKind::Keyword(Keyword::Open) => ast::Modifier::Open,
                TokenKind::Keyword(Keyword::Abstract) => ast::Modifier::Abstract,
                TokenKind::Keyword(Keyword::Sealed) => ast::Modifier::Sealed,
                TokenKind::Keyword(Keyword::Inline) => ast::Modifier::Inline,
                TokenKind::Keyword(Keyword::Override) => ast::Modifier::Override,
                TokenKind::Keyword(Keyword::Const) => ast::Modifier::Const,
                _ => break,
            };

            self.bump();
            modifiers.push(modifier);
        }

        // T0245：修饰符顺序无关，统一排序并去重，保证 AST snapshot 稳定。
        modifiers.sort_unstable();
        modifiers.dedup();
        modifiers
    }

    fn parse_type_param_list(&mut self) -> Result<(Span, Vec<ast::TypeParam>), ParseError> {
        let lt = self.expect_symbol(Symbol::Lt)?;
        let start = lt.span.start;

        let mut params = Vec::new();
        if self.peek_symbol(Symbol::Gt) {
            let gt = self.bump();
            return Ok((Span::new(start, gt.span.end), params));
        }

        loop {
            let name_tok = self.expect_kind(TokenKind::Ident, "类型参数名（标识符）")?;
            let name = ast::Ident {
                span: name_tok.span,
            };
            params.push(ast::TypeParam {
                span: name_tok.span,
                name,
            });

            if self.eat_symbol(Symbol::Comma) {
                // allow trailing comma
                if self.peek_symbol(Symbol::Gt) {
                    break;
                }
                continue;
            }
            break;
        }

        let gt = self.expect_symbol(Symbol::Gt)?;
        Ok((Span::new(start, gt.span.end), params))
    }

    fn parse_type_params_opt(&mut self) -> Result<(Option<Span>, Vec<ast::TypeParam>), ParseError> {
        if !self.peek_symbol(Symbol::Lt) {
            return Ok((None, Vec::new()));
        }
        let (span, params) = self.parse_type_param_list()?;
        Ok((Some(span), params))
    }

    /// 解析函数声明头中的“可选 receiver + name”：
    ///
    /// - 普通函数：`fun name(...)`
    /// - 扩展函数：`fun ReceiverType.name(...)`
    ///
    /// 说明：
    /// - receiver 的 TypeRef 解析复用 `parse_type_ref`，但为了避免把 `name` 误吞为路径段，
    ///   这里先用 token 扫描确定 `.` 的边界，然后用一个“子 parser”在切片上解析 receiver TypeRef。
    fn parse_fun_receiver_and_name(
        &mut self,
    ) -> Result<(Option<ast::TypeRef>, ast::Ident), ParseError> {
        let start_idx = self.i;

        let Some((dot_idx, _name_idx)) = self.detect_extension_receiver_dot(start_idx) else {
            let name_tok = self.expect_kind(TokenKind::Ident, "函数名（标识符）")?;
            return Ok((
                None,
                ast::Ident {
                    span: name_tok.span,
                },
            ));
        };

        let mut receiver_tokens: Vec<Token> = self.tokens[start_idx..dot_idx].to_vec();
        let eof_pos = receiver_tokens
            .last()
            .map(|t| t.span.end)
            .unwrap_or_else(|| self.tokens[dot_idx].span.start);
        receiver_tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(eof_pos, eof_pos),
        });

        let mut sub = Parser::new(self.source_text, receiver_tokens);
        let receiver = sub.parse_type_ref()?;
        if !sub.peek_kind(TokenKind::Eof) {
            let tok = *sub.peek();
            return Err(ParseError::Expected {
                expected: "receiver 类型结束",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        // fast-forward：跳过 receiver tokens，继续在主 parser 中消费 `. name`
        self.i = dot_idx;
        self.expect_symbol(Symbol::Dot)?;
        let name_tok = self.expect_kind(TokenKind::Ident, "函数名（标识符）")?;
        Ok((
            Some(receiver),
            ast::Ident {
                span: name_tok.span,
            },
        ))
    }

    /// 若当前位置开始是扩展函数 receiver 形式，则返回 `(dot_idx, name_idx)`：
    /// - `dot_idx`：receiver 与 name 之间的 `.` 的 token index
    /// - `name_idx`：name 的 token index
    ///
    /// 该检测仅做“语法形态”的判断，不做语义解析。
    fn detect_extension_receiver_dot(&self, start_idx: usize) -> Option<(usize, usize)> {
        // 1) 在 top-level 找到参数列表的 `(`：要求其前一个 token 是 ident 或 `>`
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;

        let mut params_lparen_idx: Option<usize> = None;

        for idx in start_idx..self.tokens.len() {
            let tok = self.tokens.get(idx)?;
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
                            let prev = self.tokens.get(idx.saturating_sub(1))?;
                            if matches!(prev.kind, TokenKind::Ident)
                                || matches!(prev.kind, TokenKind::Symbol(Symbol::Gt | Symbol::GtGt))
                            {
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
        }

        let lparen_idx = params_lparen_idx?;
        let before_lparen = lparen_idx.checked_sub(1)?;

        // 2) 从 `(` 向左回溯，找出 name 的 ident（可能带 `<T>` type params）
        let name_idx = match self.tokens.get(before_lparen)?.kind {
            TokenKind::Ident => before_lparen,
            TokenKind::Symbol(Symbol::Gt | Symbol::GtGt) => {
                let mut depth = 0usize;
                let mut found_name: Option<usize> = None;
                for j in (start_idx..=before_lparen).rev() {
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
                }
                found_name?
            }
            _ => return None,
        };

        // 3) 语法形态 `ReceiverType . name`
        let dot_idx = name_idx.checked_sub(1)?;
        if dot_idx < start_idx {
            return None;
        }
        if self.tokens.get(dot_idx)?.kind != TokenKind::Symbol(Symbol::Dot) {
            return None;
        }

        Some((dot_idx, name_idx))
    }

    pub(super) fn parse_package_decl(&mut self) -> Result<ast::PackageDecl, ParseError> {
        let kw = self.expect_keyword(Keyword::Package)?;
        let path = self.parse_dotted_path()?;
        let end = path.last().map(|i| i.span.end).unwrap_or(kw.span.end);
        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::PackageDecl {
            span: Span::new(kw.span.start, end),
            path,
        })
    }

    pub(super) fn parse_import_decl(&mut self) -> Result<ast::ImportDecl, ParseError> {
        let kw = self.expect_keyword(Keyword::Import)?;
        let first = self.expect_kind(TokenKind::Ident, "标识符")?;
        let mut path = vec![ast::Ident { span: first.span }];
        let mut has_star = false;
        let mut end = first.span.end;

        while self.peek_symbol(Symbol::Dot) {
            self.bump(); // '.'

            // `import a.b.*`：`*` 不进入 path，但 span 覆盖到 `*`
            if self.peek_symbol(Symbol::Star) {
                let star = self.bump();
                has_star = true;
                end = star.span.end;
                break;
            }

            let seg = self.expect_kind(TokenKind::Ident, "标识符")?;
            end = seg.span.end;
            path.push(ast::Ident { span: seg.span });
        }

        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::ImportDecl {
            span: Span::new(kw.span.start, end),
            path,
            has_star,
        })
    }

    pub(super) fn parse_fun_decl(&mut self) -> Result<ast::FunDecl, ParseError> {
        let start = self.peek().span.start;
        let modifiers = self.parse_modifiers();

        let _kw = self.expect_keyword(Keyword::Fun)?;
        let (receiver, name) = self.parse_fun_receiver_and_name()?;

        let (_type_params_span, type_params) = self.parse_type_params_opt()?;

        let (params_span, params) = self.parse_param_list()?;

        let return_ty = if self.eat_symbol(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        // TODO: effect rows / where clause（当前先粗暴跳过，避免阻塞后续顶层解析）
        let mut last_end = return_ty
            .as_ref()
            .map(|t| t.span().end)
            .unwrap_or(params_span.end);
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::LBrace) {
            if self.is_top_level_item_start() {
                break;
            }
            last_end = self.bump().span.end;
        }

        let body = if self.peek_symbol(Symbol::LBrace) {
            let block = self.parse_block()?;
            last_end = block.span.end;
            ast::FunBody::Block(block)
        } else {
            ast::FunBody::Missing
        };

        Ok(ast::FunDecl {
            span: Span::new(start, last_end),
            modifiers,
            receiver,
            name,
            type_params,
            params_span,
            params,
            return_ty,
            body,
        })
    }

    pub(super) fn parse_val_decl(&mut self) -> Result<ast::ValDecl, ParseError> {
        let start = self.peek().span.start;
        let modifiers = self.parse_modifiers();

        let kw = if self.peek_keyword(Keyword::Val) {
            self.bump()
        } else if self.peek_keyword(Keyword::Var) {
            self.bump()
        } else {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`val` / `var`",
                found: tok.kind,
                span: tok.span.into(),
            });
        };

        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ast::ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ast::ValKind::Var,
            _ => unreachable!("kw 已经被 peek_keyword 过滤"),
        };

        let name_tok = self.expect_kind(TokenKind::Ident, "变量名（标识符）")?;
        let name = ast::Ident { span: name_tok.span };
        let binding = ast::ValBinding::Name(name);

        let ty = if self.eat_symbol(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        let mut last_end = ty
            .as_ref()
            .map(|t| t.span().end)
            .unwrap_or(name_tok.span.end);

        let init = if self.eat_symbol(Symbol::Eq) {
            if self.peek_kind(TokenKind::Eof)
                || self.peek_symbol(Symbol::Semicolon)
                || self.is_top_level_item_start()
            {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（initializer）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }

            let init_start = self.peek().span.start;
            let expr = self.try_parse_expr()?;

            // 若 initializer 只有一个“当前已支持的表达式子集”（postfix + 常见二元优先级），则直接使用解析结果；
            // 否则（例如 `a ?: b` / `a!!` 等）保持兼容：吞掉剩余 token 并降级为 Missing，
            // 避免把未实现的表达式解析变成“顶层语法错误”。
            if let Some(expr) = expr {
                last_end = expr.span.end;

                if self.peek_kind(TokenKind::Eof)
                    || self.peek_symbol(Symbol::Semicolon)
                    || self.is_top_level_item_start()
                {
                    Some(expr)
                } else {
                    // 继续跳过 initializer 的剩余部分，直到 `;` 或下一个顶层 item。
                    // 策略：在括号深度为 0 时停止（保持与旧实现一致，尽量少引入新错误）。
                    let mut depth_paren = 0usize;
                    let mut depth_brace = 0usize;
                    let mut depth_bracket = 0usize;

                    while !self.peek_kind(TokenKind::Eof) {
                        if depth_paren == 0
                            && depth_brace == 0
                            && depth_bracket == 0
                            && (self.peek_symbol(Symbol::Semicolon)
                                || self.is_top_level_item_start())
                        {
                            break;
                        }

                        let tok = self.bump();
                        if let TokenKind::Symbol(sym) = tok.kind {
                            match sym {
                                Symbol::LParen => depth_paren += 1,
                                Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                                Symbol::LBrace => depth_brace += 1,
                                Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                                Symbol::LBracket => depth_bracket += 1,
                                Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                                _ => {}
                            }
                        }
                        last_end = tok.span.end;
                    }

                    Some(ast::Expr::missing(Span::new(init_start, last_end)))
                }
            } else {
                // initializer 不是当前表达式子集的起始 token（例如 `-1` / `when (...) { ... }`）。
                // 当前阶段不报错：直接跳过整段 initializer 并以 Missing 占位。
                let mut depth_paren = 0usize;
                let mut depth_brace = 0usize;
                let mut depth_bracket = 0usize;

                while !self.peek_kind(TokenKind::Eof) {
                    if depth_paren == 0
                        && depth_brace == 0
                        && depth_bracket == 0
                        && (self.peek_symbol(Symbol::Semicolon) || self.is_top_level_item_start())
                    {
                        break;
                    }

                    let tok = self.bump();
                    if let TokenKind::Symbol(sym) = tok.kind {
                        match sym {
                            Symbol::LParen => depth_paren += 1,
                            Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                            Symbol::LBrace => depth_brace += 1,
                            Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                            Symbol::LBracket => depth_bracket += 1,
                            Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                            _ => {}
                        }
                    }
                    last_end = tok.span.end;
                }

                Some(ast::Expr::missing(Span::new(init_start, last_end)))
            }
        } else {
            None
        };

        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::ValDecl {
            span: Span::new(start, last_end),
            modifiers,
            kind,
            binding,
            ty,
            init,
        })
    }

    pub(super) fn parse_param_list(&mut self) -> Result<(Span, Vec<ast::Param>), ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        let mut params = Vec::new();
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok((Span::new(start, close.span.end), params));
        }

        loop {
            let name_tok = self.expect_kind(TokenKind::Ident, "参数名（标识符）")?;
            let name = ast::Ident {
                span: name_tok.span,
            };

            let ty = if self.eat_symbol(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };

            // Appendix B.5.2：函数参数默认值：`param: T = expr`
            let default_value = if self.eat_symbol(Symbol::Eq) {
                let tok = *self.peek();
                Some(self.try_parse_expr()?.ok_or(ParseError::Expected {
                    expected: "表达式（参数默认值）",
                    found: tok.kind,
                    span: tok.span.into(),
                })?)
            } else {
                None
            };

            params.push(ast::Param {
                name,
                ty,
                default_value,
            });

            if self.eat_symbol(Symbol::Comma) {
                // trailing comma
                if self.peek_symbol(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_symbol(Symbol::RParen)?;
        Ok((Span::new(start, close.span.end), params))
    }

    pub(super) fn parse_type_decl(&mut self) -> Result<ast::TypeDecl, ParseError> {
        let start = self.peek().span.start;

        let modifiers = self.parse_modifiers();

        let (kind_kw, kind) = if self.peek_keyword(Keyword::Class) {
            (self.bump(), ast::TypeKind::Class)
        } else if self.peek_keyword(Keyword::Interface) {
            (self.bump(), ast::TypeKind::Interface)
        } else if self.peek_keyword(Keyword::Struct) {
            (self.bump(), ast::TypeKind::Struct)
        } else if self.peek_keyword(Keyword::Enum) {
            (self.bump(), ast::TypeKind::Enum)
        } else if self.peek_keyword(Keyword::Effect) {
            (self.bump(), ast::TypeKind::Effect)
        } else {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "类型声明关键字（class/interface/struct/enum/effect）",
                found: tok.kind,
                span: tok.span.into(),
            });
        };

        let name_tok = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let name = ast::Ident {
            span: name_tok.span,
        };

        let mut last_end = name_tok.span.end.max(kind_kw.span.end);

        let (type_params_span, type_params) = self.parse_type_params_opt()?;
        if let Some(span) = type_params_span {
            last_end = last_end.max(span.end);
        }

        // optional primary ctor params: `( ... )`
        if self.peek_symbol(Symbol::LParen) {
            let span = self.consume_balanced(Symbol::LParen, Symbol::RParen)?;
            last_end = last_end.max(span.end);
        }

        // header tail（继承/实现等）：消耗到 `{` 或下一个顶层 item 开始
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::LBrace) {
            // 注意：该函数既用于顶层，也用于 type body 内的 nested type。
            // 在 nested 场景下，`}` 可能是外层 type body 的结束符，必须在此处停止，
            // 否则会把外层 `}` 吞掉导致解析错位。
            if self.peek_symbol(Symbol::RBrace) {
                break;
            }
            if self.is_top_level_item_start() {
                break;
            }
            last_end = self.bump().span.end;
        }

        let body = if self.peek_symbol(Symbol::LBrace) {
            let body = self.parse_type_body()?;
            last_end = body.span.end;
            Some(body)
        } else {
            None
        };

        Ok(ast::TypeDecl {
            span: Span::new(start, last_end),
            modifiers,
            kind,
            name,
            type_params,
            body,
        })
    }

    fn parse_type_body(&mut self) -> Result<ast::TypeBody, ParseError> {
        let open = self.expect_symbol(Symbol::LBrace)?;
        let start = open.span.start;

        let mut members = Vec::new();
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::RBrace) {
            let head = self.peek_after_modifiers().kind;

            // 允许多余的分号（例如 Kotlin 风格的 `;` 作为 member 分隔符）。
            if self.eat_symbol(Symbol::Semicolon) {
                continue;
            }

            if matches!(
                head,
                TokenKind::Keyword(Keyword::Val | Keyword::Var)
            ) {
                match self.parse_type_member_property_decl() {
                    Ok(decl) => members.push(ast::TypeMember::Property(decl)),
                    Err(e) => {
                        // T0220：type body 内错误恢复：
                        // 记录诊断并跳过到下一个 member 起始/分隔符。
                        self.record_error(e);
                        self.skip_type_member_fallback();
                    }
                }
                continue;
            }

            if head == TokenKind::Keyword(Keyword::Fun) {
                match self.parse_type_member_fun_decl() {
                    Ok(decl) => members.push(ast::TypeMember::Fun(decl)),
                    Err(e) => {
                        self.record_error(e);
                        self.skip_type_member_fallback();
                    }
                }
                continue;
            }

            if self.is_type_decl_start() {
                match self.parse_type_decl() {
                    Ok(decl) => members.push(ast::TypeMember::Type(decl)),
                    Err(e) => {
                        self.record_error(e);
                        self.skip_type_member_fallback();
                    }
                }
                continue;
            }

            // 当前阶段：type body 里除 `val/var` / `fun` / nested type 以外的成员先粗暴跳过。
            // 目标：保持括号平衡与 span 正确，让后续任务可以增量补齐。
            self.skip_type_member_fallback();
        }

        let close = self.expect_symbol(Symbol::RBrace)?;
        Ok(ast::TypeBody {
            span: Span::new(start, close.span.end),
            members,
        })
    }

    fn parse_type_member_property_decl(&mut self) -> Result<ast::PropertyDecl, ParseError> {
        let start = self.peek().span.start;
        let modifiers = self.parse_modifiers();

        let kw = if self.peek_keyword(Keyword::Val) {
            self.bump()
        } else if self.peek_keyword(Keyword::Var) {
            self.bump()
        } else {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`val` / `var`",
                found: tok.kind,
                span: tok.span.into(),
            });
        };

        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ast::ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ast::ValKind::Var,
            _ => unreachable!("kw 已经被 peek_keyword 过滤"),
        };

        let name_tok = self.expect_kind(TokenKind::Ident, "变量名（标识符）")?;
        let name = ast::Ident {
            span: name_tok.span,
        };

        // spec §10.1：属性声明在无 initializer 时必须显式标注类型；
        // 为避免把 `val x` 解析成“无类型属性并继续吞掉 accessors”，当前阶段（T0234）
        // 直接要求 type body 内的属性具备 `: Type`（即使将来允许类型推断，也可在 parser 层放宽）。
        if !self.peek_symbol(Symbol::Colon) {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`:`",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        let ty = Some({
            self.bump(); // ':'
            self.parse_type_ref()?
        });

        let mut last_end = ty
            .as_ref()
            .map(|t| t.span().end)
            .unwrap_or(name_tok.span.end);

        let init = if self.eat_symbol(Symbol::Eq) {
            if self.peek_kind(TokenKind::Eof)
                || self.peek_symbol(Symbol::Semicolon)
                || self.peek_symbol(Symbol::RBrace)
                || self.is_type_member_start()
                || self.is_property_accessor_start()
            {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（initializer）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }

            let init_start = self.peek().span.start;
            let expr = self.try_parse_expr()?;

            if let Some(expr) = expr {
                last_end = expr.span.end;
                if self.peek_kind(TokenKind::Eof)
                    || self.peek_symbol(Symbol::Semicolon)
                    || self.peek_symbol(Symbol::RBrace)
                    || self.is_type_member_start()
                    || self.is_property_accessor_start()
                {
                    Some(expr)
                } else {
                    // 继续跳过 initializer 的剩余部分，直到 `;` / `}` / 下一个 member / accessor 起始。
                    let span =
                        self.skip_until_type_member_or_accessor_boundary(init_start, last_end);
                    last_end = span.end;
                    Some(ast::Expr::missing(span))
                }
            } else {
                // initializer 不是当前表达式子集的起始 token。
                let span = self.skip_until_type_member_or_accessor_boundary(init_start, last_end);
                last_end = span.end;
                Some(ast::Expr::missing(span))
            }
        } else {
            None
        };

        // accessors：`get()` / `set(value)`
        let mut getter = None;
        let mut setter = None;
        while self.is_property_accessor_start() {
            let acc = self.parse_property_accessor_decl()?;
            last_end = last_end.max(acc.span.end);
            match acc.kind {
                ast::AccessorKind::Get => getter = Some(acc),
                ast::AccessorKind::Set => setter = Some(acc),
            }
        }

        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::PropertyDecl {
            span: Span::new(start, last_end),
            modifiers,
            kind,
            name,
            ty,
            init,
            getter,
            setter,
        })
    }

    fn is_property_accessor_start(&self) -> bool {
        if !self.peek_kind(TokenKind::Ident) || !self.peek_symbol_n(1, Symbol::LParen) {
            return false;
        }

        let tok = self.peek();
        let Some(name) = self.source_text.get(tok.span.start..tok.span.end) else {
            return false;
        };

        match name {
            // get() (= expr | { ... })
            "get" => {
                self.peek_symbol_n(2, Symbol::RParen)
                    && matches!(
                        self.peek_n(3).kind,
                        TokenKind::Symbol(Symbol::Eq | Symbol::LBrace)
                    )
            }
            // set(value) (= expr | { ... })
            "set" => {
                self.peek_kind_n(2, TokenKind::Ident)
                    && self.peek_symbol_n(3, Symbol::RParen)
                    && matches!(
                        self.peek_n(4).kind,
                        TokenKind::Symbol(Symbol::Eq | Symbol::LBrace)
                    )
            }
            _ => false,
        }
    }

    fn peek_symbol_n(&self, n: usize, sym: Symbol) -> bool {
        self.peek_n(n).kind == TokenKind::Symbol(sym)
    }

    fn peek_kind_n(&self, n: usize, kind: TokenKind) -> bool {
        self.peek_n(n).kind == kind
    }

    fn parse_property_accessor_decl(&mut self) -> Result<ast::AccessorDecl, ParseError> {
        let name_tok = self.expect_kind(TokenKind::Ident, "accessor 名称（get/set）")?;
        let start = name_tok.span.start;
        let kind = match self.source_text.get(name_tok.span.start..name_tok.span.end) {
            Some("get") => ast::AccessorKind::Get,
            Some("set") => ast::AccessorKind::Set,
            _ => {
                return Err(ParseError::Expected {
                    expected: "`get` / `set`",
                    found: TokenKind::Ident,
                    span: name_tok.span.into(),
                });
            }
        };

        self.expect_symbol(Symbol::LParen)?;

        let param = if kind == ast::AccessorKind::Set {
            let tok = self.expect_kind(TokenKind::Ident, "setter 参数名（标识符）")?;
            let ident = ast::Ident { span: tok.span };
            // 可选 `: Type`（未来补齐；当前先消费但不进入 AST）。
            if self.eat_symbol(Symbol::Colon) {
                let _ = self.parse_type_ref()?;
            }
            Some(ident)
        } else {
            // getter: `get()`
            if !self.peek_symbol(Symbol::RParen) {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "`)`",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }
            None
        };

        let close = self.expect_symbol(Symbol::RParen)?;
        let mut last_end = close.span.end;

        let body = if self.eat_symbol(Symbol::Eq) {
            if self.peek_kind(TokenKind::Eof)
                || self.peek_symbol(Symbol::Semicolon)
                || self.peek_symbol(Symbol::RBrace)
                || self.is_type_member_start()
                || self.is_property_accessor_start()
            {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（accessor body）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }

            let expr_start = self.peek().span.start;
            let expr = self.try_parse_expr()?;

            if let Some(expr) = expr {
                last_end = expr.span.end;
                if self.peek_kind(TokenKind::Eof)
                    || self.peek_symbol(Symbol::Semicolon)
                    || self.peek_symbol(Symbol::RBrace)
                    || self.is_type_member_start()
                    || self.is_property_accessor_start()
                {
                    ast::AccessorBody::Expr(expr)
                } else {
                    let span =
                        self.skip_until_type_member_or_accessor_boundary(expr_start, last_end);
                    last_end = span.end;
                    ast::AccessorBody::Expr(ast::Expr::missing(span))
                }
            } else {
                let span = self.skip_until_type_member_or_accessor_boundary(expr_start, last_end);
                last_end = span.end;
                ast::AccessorBody::Expr(ast::Expr::missing(span))
            }
        } else if self.peek_symbol(Symbol::LBrace) {
            let block = self.parse_block()?;
            last_end = block.span.end;
            ast::AccessorBody::Block(block)
        } else {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`=` 或 `{ ... }`（accessor body）",
                found: tok.kind,
                span: tok.span.into(),
            });
        };

        Ok(ast::AccessorDecl {
            span: Span::new(start, last_end),
            kind,
            param,
            body,
        })
    }

    fn skip_until_type_member_or_accessor_boundary(
        &mut self,
        start: usize,
        mut last_end: usize,
    ) -> Span {
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        while !self.peek_kind(TokenKind::Eof) {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.peek_symbol(Symbol::Semicolon)
                    || self.peek_symbol(Symbol::RBrace)
                    || self.is_type_member_start()
                    || self.is_property_accessor_start())
            {
                break;
            }

            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                match sym {
                    Symbol::LParen => depth_paren += 1,
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    _ => {}
                }
            }
            last_end = tok.span.end;
        }

        Span::new(start, last_end)
    }

    fn parse_type_member_fun_decl(&mut self) -> Result<ast::FunDecl, ParseError> {
        // 目标：只解析函数声明头（name/params/return type），函数体仍只保留 span。
        let start = self.peek().span.start;
        let modifiers = self.parse_modifiers();

        let _kw = self.expect_keyword(Keyword::Fun)?;
        let (receiver, name) = self.parse_fun_receiver_and_name()?;

        let (_type_params_span, type_params) = self.parse_type_params_opt()?;

        let (params_span, params) = self.parse_param_list()?;

        let return_ty = if self.eat_symbol(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        // TODO: effect rows / where clause（当前先粗暴跳过，避免阻塞后续 type body 解析）
        let mut last_end = return_ty
            .as_ref()
            .map(|t| t.span().end)
            .unwrap_or(params_span.end);
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::LBrace) {
            if self.peek_symbol(Symbol::Semicolon)
                || self.peek_symbol(Symbol::RBrace)
                || self.is_type_member_start()
            {
                break;
            }
            last_end = self.bump().span.end;
        }

        let body = if self.peek_symbol(Symbol::LBrace) {
            let block = self.parse_block()?;
            last_end = block.span.end;
            ast::FunBody::Block(block)
        } else {
            ast::FunBody::Missing
        };

        Ok(ast::FunDecl {
            span: Span::new(start, last_end),
            modifiers,
            receiver,
            name,
            type_params,
            params_span,
            params,
            return_ty,
            body,
        })
    }

    fn skip_type_member_fallback(&mut self) {
        // 保证至少消耗一个 token，避免死循环。
        if self.peek_kind(TokenKind::Eof) || self.peek_symbol(Symbol::RBrace) {
            return;
        }

        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        let first = self.bump();
        if let TokenKind::Symbol(sym) = first.kind {
            match sym {
                Symbol::LParen => depth_paren += 1,
                Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                Symbol::LBrace => depth_brace += 1,
                Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                Symbol::LBracket => depth_bracket += 1,
                Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                _ => {}
            }
        }

        while !self.peek_kind(TokenKind::Eof) {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.peek_symbol(Symbol::Semicolon)
                    || self.peek_symbol(Symbol::RBrace)
                    || self.is_type_member_start())
            {
                break;
            }

            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                match sym {
                    Symbol::LParen => depth_paren += 1,
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    _ => {}
                }
            }
        }
    }

    pub(super) fn parse_dotted_path(&mut self) -> Result<Vec<ast::Ident>, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "标识符")?;
        let mut parts = vec![ast::Ident { span: first.span }];
        while self.peek_symbol(Symbol::Dot) {
            self.bump(); // '.'
            let ident = self.expect_kind(TokenKind::Ident, "标识符")?;
            parts.push(ast::Ident { span: ident.span });
        }
        Ok(parts)
    }
}
