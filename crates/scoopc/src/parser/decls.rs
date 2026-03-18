//! 顶层声明解析：package/import/fun/type decl。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
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
        let kw = self.expect_keyword(Keyword::Fun)?;
        let name_tok = self.expect_kind(TokenKind::Ident, "函数名（标识符）")?;
        let name = ast::Ident {
            span: name_tok.span,
        };

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
            span: Span::new(kw.span.start, last_end),
            name,
            type_params,
            params_span,
            params,
            return_ty,
            body,
        })
    }

    pub(super) fn parse_val_decl(&mut self) -> Result<ast::ValDecl, ParseError> {
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
            span: Span::new(kw.span.start, last_end),
            kind,
            name,
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
            params.push(ast::Param { name, ty });

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

        // modifiers（当前仅消费，不进入 AST）
        while self.peek_keyword(Keyword::Open)
            || self.peek_keyword(Keyword::Abstract)
            || self.peek_keyword(Keyword::Sealed)
        {
            self.bump();
        }

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
            // 允许多余的分号（例如 Kotlin 风格的 `;` 作为 member 分隔符）。
            if self.eat_symbol(Symbol::Semicolon) {
                continue;
            }

            if self.peek_keyword(Keyword::Val) || self.peek_keyword(Keyword::Var) {
                let decl = self.parse_type_member_val_decl()?;
                members.push(ast::TypeMember::Val(decl));
                continue;
            }

            if self.peek_keyword(Keyword::Fun) {
                let decl = self.parse_type_member_fun_decl()?;
                members.push(ast::TypeMember::Fun(decl));
                continue;
            }

            if self.is_type_decl_start() {
                let decl = self.parse_type_decl()?;
                members.push(ast::TypeMember::Type(decl));
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

    fn parse_type_member_val_decl(&mut self) -> Result<ast::ValDecl, ParseError> {
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

        // `val x Int` / `val x (Int, Int)`：在 member 位置基本只能是“漏写冒号”。
        // 提前给更贴近语法位置的错误，而不是等到下一轮循环在更远处报错。
        if !self.peek_symbol(Symbol::Colon)
            && (self.peek_kind(TokenKind::Ident) || self.peek_symbol(Symbol::LParen))
        {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`:`",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

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
                || self.peek_symbol(Symbol::RBrace)
                || self.is_type_member_start()
            {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（initializer）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }

            let init_start = self.peek().span.start;

            // 当前阶段不解析表达式：仅保证能“跳过 initializer”并继续解析后续 member。
            // 策略：在括号深度为 0 时，遇到 `;` / `}` / 下一个 member 开始即停止。
            let mut depth_paren = 0usize;
            let mut depth_brace = 0usize;
            let mut depth_bracket = 0usize;

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
                last_end = tok.span.end;
            }

            Some(ast::Expr::missing(Span::new(init_start, last_end)))
        } else {
            None
        };

        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::ValDecl {
            span: Span::new(kw.span.start, last_end),
            kind,
            name,
            ty,
            init,
        })
    }

    fn parse_type_member_fun_decl(&mut self) -> Result<ast::FunDecl, ParseError> {
        // 目标：只解析函数声明头（name/params/return type），函数体仍只保留 span。
        let kw = self.expect_keyword(Keyword::Fun)?;
        let name_tok = self.expect_kind(TokenKind::Ident, "函数名（标识符）")?;
        let name = ast::Ident {
            span: name_tok.span,
        };

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
            span: Span::new(kw.span.start, last_end),
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
