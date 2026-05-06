//! 语句与块解析（早期最小子集）。
//!
//! 当前阶段（T0207）目标：
//! - 把块表达式 `{ ... }` 解析为 `ast::Block { stmts }`
//! - 语句仅支持：
//!   - 空语句：`;`
//!   - 表达式语句：基于现有表达式子集解析（`try_parse_expr`，当前支持 postfix + 二元优先级）
//! - 其它尚未实现的语句形态：报告 parse error；`StmtKind::Missing` 仅用于错误恢复，
//!   不能出现在成功 parse 的 AST 中。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, Token, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    /// 解析块表达式（含函数体 block）：`{ stmt* }`。
    pub(super) fn parse_block(&mut self) -> Result<ast::Block, ParseError> {
        let open = self.expect_symbol(Symbol::LBrace)?;
        self.parse_block_with_open(open)
    }

    /// 在已经消费了 `{` 后解析块内容，直到匹配的 `}`。
    ///
    /// 该入口被 `parse_block()` 与 lambda 解析共享，以避免重复实现 block 内语句解析与错误恢复逻辑。
    pub(super) fn parse_block_with_open(&mut self, open: Token) -> Result<ast::Block, ParseError> {
        debug_assert_eq!(open.kind, TokenKind::Symbol(Symbol::LBrace));
        let start = open.span.start;

        let mut stmts = Vec::new();
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::RBrace) {
            // 允许多余的分号：把它们视为”空语句”。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                stmts.push(ast::Stmt {
                    span: semi.span,
                    kind: ast::StmtKind::Empty,
                    has_trailing_semi: true,
                });
                continue;
            }

            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(e) => {
                    // T0220：block 内错误恢复：
                    // - 记录诊断
                    // - 跳过到下一个“看起来像语句起始”的 token 或 `}`/`;`
                    self.record_error(e);
                    stmts.push(self.recover_stmt_after_error());
                }
            }
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBrace,
                span: Span::new(start, self.peek().span.end).into(),
            });
        }

        let close = self.expect_symbol(Symbol::RBrace)?;
        Ok(ast::Block {
            span: Span::new(start, close.span.end),
            stmts,
        })
    }

    fn parse_stmt(&mut self) -> Result<ast::Stmt, ParseError> {
        // `comptime` 语句（T0246）：
        // - `comptime { ... }`
        // - `comptime if (...) { ... } else ...`
        // - `comptime for (x in xs) { ... }`
        if self.peek_keyword(Keyword::Comptime) {
            return self.parse_comptime_stmt();
        }

        // 局部 `val/var` 绑定语句（T0208）。
        //
        // 注意：此处不能直接复用顶层的 `parse_val_decl`，因为顶层的 initializer 边界规则会假设
        // “下一个 token 必须是顶层 item start/`;`/EOF”，在 block 语境下会误把后续语句吞进 initializer。
        if self.peek_keyword(Keyword::Val)
            || self.peek_keyword(Keyword::Var)
            || self.looks_like_annotated_local_val_decl()
        {
            let decl = self.parse_local_val_decl()?;
            let mut span = decl.span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Val(decl),
                has_trailing_semi,
            });
        }

        // `return` 语句（T0226）。
        if self.peek_keyword(Keyword::Return) {
            let kw = self.bump();
            let return_span = kw.span;

            let value = if self.peek_kind(TokenKind::Eof)
                || self.peek_symbol(Symbol::Semicolon)
                || self.peek_symbol(Symbol::RBrace)
            {
                None
            } else {
                match self.try_parse_expr()? {
                    Some(expr) => Some(expr),
                    None => {
                        // `return` 后既不是语句边界，也不是当前表达式子集的起始：
                        // - 若下一个 token 看起来像“下一条语句的起始”，则视为 `return` 无返回值；
                        // - 否则报错，避免静默吞掉明显的语法问题（例如 `return + 1`）。
                        if self.is_stmt_start() {
                            None
                        } else {
                            let tok = *self.peek();
                            return Err(ParseError::Expected {
                                expected: "表达式（return 的返回值）",
                                found: tok.kind,
                                span: tok.span.into(),
                            });
                        }
                    }
                }
            };

            let mut span = Span::new(
                return_span.start,
                value
                    .as_ref()
                    .map(|e| e.span.end)
                    .unwrap_or(return_span.end),
            );
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Return { return_span, value },
                has_trailing_semi,
            });
        }

        // `while` 循环语句（T0228）：`while (cond) { ... }`
        if self.peek_keyword(Keyword::While) {
            let kw = self.bump();
            let while_span = kw.span;

            self.expect_symbol(Symbol::LParen)?;
            let cond = match self.try_parse_expr()? {
                Some(expr) => expr,
                None => {
                    let tok = *self.peek();
                    return Err(ParseError::Expected {
                        expected: "表达式（while 条件）",
                        found: tok.kind,
                        span: tok.span.into(),
                    });
                }
            };
            self.expect_symbol(Symbol::RParen)?;

            // 当前阶段仅支持 block body：`{ ... }`。
            let body = self.parse_block()?;

            let mut span = Span::new(while_span.start, body.span.end);
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::While {
                    while_span,
                    cond,
                    body,
                },
                has_trailing_semi,
            });
        }

        // `break` 语句（T0228）。
        if self.peek_keyword(Keyword::Break) {
            let kw = self.bump();
            let break_span = kw.span;
            let mut span = break_span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }
            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Break { break_span },
                has_trailing_semi,
            });
        }

        // `continue` 语句（T0228）。
        if self.peek_keyword(Keyword::Continue) {
            let kw = self.bump();
            let continue_span = kw.span;
            let mut span = continue_span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }
            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Continue { continue_span },
                has_trailing_semi,
            });
        }

        // `for (x in xs) { ... }` 循环语句（Appendix B.12 / T1304）。
        if self.peek_keyword(Keyword::For) {
            let for_tok = self.bump();
            let for_span = for_tok.span;

            self.expect_symbol(Symbol::LParen)?;
            let binder_tok = self.expect_kind(TokenKind::Ident, "循环变量名（标识符）")?;
            let binder = ast::Ident::new(binder_tok.span);

            let in_tok = self.expect_keyword(Keyword::In)?;
            let in_span = in_tok.span;

            let tok = *self.peek();
            let iter = self.try_parse_expr()?.ok_or(ParseError::Expected {
                expected: "表达式（for 迭代对象）",
                found: tok.kind,
                span: tok.span.into(),
            })?;
            self.expect_symbol(Symbol::RParen)?;

            // 当前阶段仅支持 block body：`{ ... }`。
            let body = self.parse_block()?;

            let for_stmt = ast::ForStmt {
                span: Span::new(for_span.start, body.span.end),
                for_span,
                binder,
                in_span,
                iter,
                body,
                resolved_for_info: std::cell::OnceCell::new(),
            };

            let mut span = for_stmt.span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::For(for_stmt),
                has_trailing_semi,
            });
        }

        // 先尝试“表达式语句”：当前阶段的表达式仍是受限子集（postfix + 常见二元优先级），
        // 因此语句边界也就天然落在该表达式结束处。
        if let Some(expr) = self.try_parse_stmt_expr()? {
            let mut span = expr.span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Expr(expr),
                has_trailing_semi,
            });
        }

        let tok = *self.peek();
        Err(ParseError::Expected {
            expected: "语句（当前语法不能进入 HIR）",
            found: tok.kind,
            span: tok.span.into(),
        })
    }

    fn looks_like_annotated_local_val_decl(&self) -> bool {
        if !self.peek_symbol(Symbol::At) {
            return false;
        }

        let mut idx = self.i;
        while matches!(
            self.tokens.get(idx).map(|tok| tok.kind),
            Some(TokenKind::Symbol(Symbol::At))
        ) {
            idx = self.skip_one_annotation_idx(idx);
        }

        matches!(
            self.tokens.get(idx).map(|tok| tok.kind),
            Some(TokenKind::Keyword(Keyword::Val | Keyword::Var))
        )
    }

    fn parse_comptime_stmt(&mut self) -> Result<ast::Stmt, ParseError> {
        let comptime_tok = self.expect_keyword(Keyword::Comptime)?;
        let comptime_span = comptime_tok.span;

        // `comptime { ... }`
        if self.peek_symbol(Symbol::LBrace) {
            let body = self.parse_block()?;
            let mut span = Span::new(comptime_span.start, body.span.end);
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::ComptimeBlock {
                    comptime_span,
                    body,
                },
                has_trailing_semi,
            });
        }

        // `comptime if (...) { ... } else ...`
        if self.peek_keyword(Keyword::If) {
            let if_stmt = self.parse_comptime_if_after_comptime(comptime_span)?;
            let mut span = if_stmt.span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }
            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::ComptimeIf(if_stmt),
                has_trailing_semi,
            });
        }

        // `comptime for (x in xs) { ... }`
        if self.peek_keyword(Keyword::For) {
            let for_stmt = self.parse_comptime_for_after_comptime(comptime_span)?;
            let mut span = for_stmt.span;
            let mut has_trailing_semi = false;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
                has_trailing_semi = true;
            }
            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::ComptimeFor(for_stmt),
                has_trailing_semi,
            });
        }

        let tok = *self.peek();
        Err(ParseError::Expected {
            expected: "`{` / `if` / `for`（comptime 语句）",
            found: tok.kind,
            span: tok.span.into(),
        })
    }

    fn parse_comptime_if_after_comptime(
        &mut self,
        comptime_span: Span,
    ) -> Result<ast::ComptimeIf, ParseError> {
        let if_tok = self.expect_keyword(Keyword::If)?;
        let if_span = if_tok.span;

        self.expect_symbol(Symbol::LParen)?;
        let tok = *self.peek();
        let cond = self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（comptime if 条件）",
            found: tok.kind,
            span: tok.span.into(),
        })?;
        self.expect_symbol(Symbol::RParen)?;

        let then_branch = self.parse_block()?;

        let else_branch = if self.peek_keyword(Keyword::Else) {
            let else_tok = self.bump(); // `else`
            let else_span = else_tok.span;

            // 规范示例使用 `else comptime if (...) { ... }` 作为 else-if 链。
            if self.peek_keyword(Keyword::Comptime) {
                let else_comptime = self.bump();
                let else_comptime_span = else_comptime.span;

                if !self.peek_keyword(Keyword::If) {
                    let tok = *self.peek();
                    return Err(ParseError::Expected {
                        expected: "`if`（`else comptime if`）",
                        found: tok.kind,
                        span: tok.span.into(),
                    });
                }

                let nested = self.parse_comptime_if_after_comptime(else_comptime_span)?;
                Some(Box::new(ast::ComptimeIfElse::If(Box::new(nested))))
            } else if self.peek_keyword(Keyword::If) {
                // 语法糖：允许写 `else if (...) { ... }`，等价于 `else comptime if (...) { ... }`。
                let implicit_comptime = Span::new(else_span.end, else_span.end);
                let nested = self.parse_comptime_if_after_comptime(implicit_comptime)?;
                Some(Box::new(ast::ComptimeIfElse::If(Box::new(nested))))
            } else if self.peek_symbol(Symbol::LBrace) {
                let block = self.parse_block()?;
                Some(Box::new(ast::ComptimeIfElse::Block(block)))
            } else {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "`{` 或 `comptime if`（comptime if 的 else 分支）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }
        } else {
            None
        };

        let end = else_branch
            .as_deref()
            .map(|e| match e {
                ast::ComptimeIfElse::Block(b) => b.span.end,
                ast::ComptimeIfElse::If(i) => i.span.end,
            })
            .unwrap_or(then_branch.span.end);

        Ok(ast::ComptimeIf {
            span: Span::new(comptime_span.start, end),
            comptime_span,
            if_span,
            cond,
            then_branch,
            else_branch,
        })
    }

    fn parse_comptime_for_after_comptime(
        &mut self,
        comptime_span: Span,
    ) -> Result<ast::ComptimeFor, ParseError> {
        let for_tok = self.expect_keyword(Keyword::For)?;
        let for_span = for_tok.span;

        self.expect_symbol(Symbol::LParen)?;
        let binder_tok = self.expect_kind(TokenKind::Ident, "循环变量名（标识符）")?;
        let binder = ast::Ident::new(binder_tok.span);

        let in_tok = self.expect_keyword(Keyword::In)?;
        let in_span = in_tok.span;

        let tok = *self.peek();
        let iter = self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（comptime for 迭代对象）",
            found: tok.kind,
            span: tok.span.into(),
        })?;
        self.expect_symbol(Symbol::RParen)?;

        let body = self.parse_block()?;

        Ok(ast::ComptimeFor {
            span: Span::new(comptime_span.start, body.span.end),
            comptime_span,
            for_span,
            binder,
            in_span,
            iter,
            body,
        })
    }

    fn parse_local_val_decl(&mut self) -> Result<ast::ValDecl, ParseError> {
        let mut annotations = Vec::new();
        while self.peek_symbol(Symbol::At) {
            annotations.push(self.parse_annotation_use()?);
        }

        let kw = self.bump_val_or_var_keyword()?;

        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ast::ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ast::ValKind::Var,
            _ => unreachable!("kw 已经被 peek_keyword 过滤"),
        };

        // `var (a, b) = ...`：按 spec 不支持。
        //
        // 这里主动吞掉 `( ... )`，把 cursor 恢复到 `=` 附近，避免 block 内错误恢复
        // 把 `(` 当作“下一条语句起始”从而产生级联错误（ManyErrors）。
        if kind == ast::ValKind::Var && self.peek_symbol(Symbol::LParen) {
            let tok = *self.peek();
            let _ = self.consume_balanced(Symbol::LParen, Symbol::RParen)?;
            return Err(ParseError::Expected {
                expected: "变量名（标识符）",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        // T0244/T0460：`val` 支持解构绑定（tuple/struct/variant pattern）；`var` 按 spec 不支持。
        let should_parse_pattern = kind == ast::ValKind::Val
            && (self.peek_symbol(Symbol::LParen)
                || self.looks_like_struct_pattern_ahead()
                || self.looks_like_variant_pattern_ahead());
        let (binding, binding_end) = if should_parse_pattern {
            let pat = self.parse_pattern()?;
            let end = pat.span.end;
            (ast::ValBinding::Pattern(pat), end)
        } else {
            let name_tok = self.expect_kind(TokenKind::Ident, "变量名（标识符）")?;
            let name = ast::Ident::new(name_tok.span);
            // `var Point { ... } = ...` 这种形态通常是“误写解构”；给出更明确的语法错误位置。
            if kind == ast::ValKind::Var && self.peek_symbol(Symbol::LBrace) {
                let tok = *self.peek();
                let _ = self.consume_balanced(Symbol::LBrace, Symbol::RBrace)?;
                return Err(ParseError::Expected {
                    expected: "`:` / `=`（`var` 不支持解构绑定）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }
            (ast::ValBinding::Name(name), name_tok.span.end)
        };

        let ty = if matches!(binding, ast::ValBinding::Name(_)) && self.eat_symbol(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        let mut last_end = ty.as_ref().map(|t| t.span().end).unwrap_or(binding_end);

        let init = if self.eat_symbol(Symbol::Eq) {
            if self.peek_kind(TokenKind::Eof)
                || self.peek_symbol(Symbol::Semicolon)
                || self.peek_symbol(Symbol::RBrace)
            {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（initializer）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }

            let init_start = self.peek().span.start;
            match self.try_parse_expr()? {
                Some(expr) => {
                    last_end = expr.span.end;
                    Some(expr)
                }
                None => {
                    // initializer 不是当前表达式子集的起始 token（例如 `-1` / `when (...) { ... }`）：
                    // 当前阶段不报错，消耗一个 token 并降级为 Missing，保证 cursor 前进。
                    let tok = self.bump();
                    last_end = tok.span.end;
                    Some(ast::Expr::missing(Span::new(init_start, last_end)))
                }
            }
        } else if matches!(binding, ast::ValBinding::Pattern(_)) {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`=`（解构绑定需要 initializer）",
                found: tok.kind,
                span: tok.span.into(),
            });
        } else {
            None
        };

        Ok(ast::ValDecl {
            span: Span::new(kw.span.start, last_end),
            annotations,
            modifiers: Vec::new(),
            kind,
            binding,
            ty,
            init,
        })
    }

    fn parse_missing_stmt(&mut self) -> ast::Stmt {
        // 错误恢复与“未实现语句”fallback 都会走到这里：
        // - 必须保证 cursor 前进（避免死循环）
        // - 同时不要吞掉 block 的 `}`（否则会导致外层的 `{ ... }` 不平衡）
        if self.peek_kind(TokenKind::Eof) || self.peek_symbol(Symbol::RBrace) {
            let pos = self.peek().span.start;
            return ast::Stmt {
                span: Span::new(pos, pos),
                kind: ast::StmtKind::Missing,
                has_trailing_semi: false,
            };
        }

        let start = self.peek().span.start;
        let mut last_end;

        // 粗粒度“语句恢复”：
        // - 在括号深度为 0 时，遇到 `;` / `}` 认为到达语句边界。
        // - 在括号深度为 0 时，遇到“像是下一条语句起始”的 token，也认为到达边界。
        // - 其余 token 全部吞掉，直到边界出现（但不吞 `}`）。
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        // 先吞掉一个 token，确保前进。
        let first = self.bump();
        last_end = first.span.end;
        if let TokenKind::Symbol(sym) = first.kind {
            match sym {
                Symbol::LParen => depth_paren += 1,
                Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                Symbol::LBracket => depth_bracket += 1,
                Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                Symbol::LBrace => depth_brace += 1,
                Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                _ => {}
            }
        }

        while !self.peek_kind(TokenKind::Eof) {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.peek_symbol(Symbol::Semicolon)
                    || self.peek_symbol(Symbol::RBrace)
                    || self.is_recovery_boundary_stmt_start())
            {
                break;
            }

            let tok = self.bump();
            last_end = tok.span.end;

            if let TokenKind::Symbol(sym) = tok.kind {
                match sym {
                    Symbol::LParen => depth_paren += 1,
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    _ => {}
                }
            }
        }

        // 若以 `;` 作为分隔符结束，则把分号也吞掉，避免外层再额外产出一个 Empty stmt。
        let mut has_trailing_semi = false;
        if self.peek_symbol(Symbol::Semicolon) {
            let semi = self.bump();
            last_end = semi.span.end;
            has_trailing_semi = true;
        }

        ast::Stmt {
            span: Span::new(start, last_end),
            kind: ast::StmtKind::Missing,
            has_trailing_semi,
        }
    }

    fn recover_stmt_after_error(&mut self) -> ast::Stmt {
        // 若当前位置已经是一个”潜在的语句边界”，则不要吞掉它，
        // 这样外层循环还能继续尝试解析后续语句。
        if self.peek_kind(TokenKind::Eof)
            || self.peek_symbol(Symbol::RBrace)
            || self.is_recovery_boundary_stmt_start()
        {
            let pos = self.peek().span.start;
            return ast::Stmt {
                span: Span::new(pos, pos),
                kind: ast::StmtKind::Missing,
                has_trailing_semi: false,
            };
        }

        self.parse_missing_stmt()
    }

    fn is_recovery_boundary_stmt_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Return
                    | Keyword::Comptime
                    | Keyword::For
                    | Keyword::While
                    | Keyword::Break
                    | Keyword::Continue
            )
        )
    }
}
