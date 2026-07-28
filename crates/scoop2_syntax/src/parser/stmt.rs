//! 语句与块解析（grammar §7）。

use scoop2_base::Span;

use crate::ast::decl::{ValBinding, ValDecl, ValKind};
use crate::ast::expr::*;
use crate::token::{Keyword, Symbol, Token, TokenKind};

use super::expr::InfixMode;
use super::{PResult, Parser};

impl<'a> Parser<'a> {
    /// 解析块：`{ stmt* }`。
    pub(crate) fn parse_block(&mut self) -> PResult<Block> {
        let open = self.expect_sym(Symbol::LBrace)?;
        self.parse_block_with_open(open, None)
    }

    /// 在已消费 `{` 后解析块内容；`semi_flags`（若提供）逐语句记录“是否吃掉了尾随 `;`”
    /// （lambda 的解包规则需要，§8.2）。
    pub(crate) fn parse_block_with_open(
        &mut self,
        open: Token,
        mut semi_flags: Option<&mut Vec<bool>>,
    ) -> PResult<Block> {
        let start = open.span.start;
        let block_id = self.nid();

        let mut stmts = Vec::new();
        while !self.at_eof() && !self.at_sym(Symbol::RBrace) {
            // 孤立的 `;`：空语句。
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                stmts.push(Stmt {
                    id: self.nid(),
                    span: semi.span,
                    kind: StmtKind::Empty,
                });
                if let Some(flags) = semi_flags.as_deref_mut() {
                    flags.push(true);
                }
                continue;
            }

            match self.parse_stmt() {
                Ok(stmt) => {
                    if let Some(flags) = semi_flags.as_deref_mut() {
                        flags.push(stmt_had_trailing_semi(&stmt, self.source));
                    }
                    stmts.push(stmt);
                }
                Err(_abort) => {
                    // 错误已记录；语句级恢复：跳到下一个语句边界（不产出节点）。
                    self.recover_stmt_after_error();
                }
            }
        }

        if self.at_eof() {
            return Err(self.err_unclosed(start, "`}`"));
        }
        let close = self.expect_sym(Symbol::RBrace)?;
        // 检测最后一条语句是否带 `;`（影响 block 值类型）。
        let last_trailing_semi = stmts
            .last()
            .is_some_and(|s| stmt_had_trailing_semi(s, self.source));
        Ok(Block {
            id: block_id,
            span: Span::new(start, close.span.end),
            stmts,
            last_trailing_semi,
        })
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        // 局部 `val/var`（含注解形式）。
        if self.at_kw(Keyword::Val)
            || self.at_kw(Keyword::Var)
            || self.looks_like_annotated_local_val_decl()
        {
            let decl = self.parse_local_val_decl()?;
            let mut span = decl_span(&decl);
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::LocalVal(Box::new(decl)),
            });
        }

        // `return expr?`。
        if self.at_kw(Keyword::Return) {
            let kw = self.bump();
            let value =
                if self.at_eof() || self.at_sym(Symbol::Semicolon) || self.at_sym(Symbol::RBrace) {
                    None
                } else if self.is_expr_start_for_return() {
                    Some(self.expr()?)
                } else if self.is_stmt_start() {
                    // 恢复启发式（§7）：下一个 token 能开始语句但不能开始表达式 → 无值 return。
                    None
                } else {
                    let tok = self.peek();
                    return Err(self.err_expected("表达式（return 的返回值）", tok));
                };

            let mut span = Span::new(
                kw.span.start,
                value.as_ref().map(|e| e.span.end).unwrap_or(kw.span.end),
            );
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::Return { value },
            });
        }

        // `while (cond) { ... }`。
        if self.at_kw(Keyword::While) {
            let kw = self.bump();
            self.expect_sym(Symbol::LParen)?;
            let cond = self.expr()?;
            self.expect_sym(Symbol::RParen)?;
            let body = self.parse_block()?;
            let mut span = Span::new(kw.span.start, body.span.end);
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::While { cond, body },
            });
        }

        // `for (x in xs) { ... }`（binder 是单个标识符，§7/§11）。
        if self.at_kw(Keyword::For) {
            let kw = self.bump();
            self.expect_sym(Symbol::LParen)?;
            let binder_tok = self.expect_ident("循环变量名")?;
            let binder = self.ident(binder_tok);
            self.expect_kw(Keyword::In)?;
            let iter = self.expr()?;
            self.expect_sym(Symbol::RParen)?;
            let body = self.parse_block()?;
            let mut span = Span::new(kw.span.start, body.span.end);
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::For { binder, iter, body },
            });
        }

        if self.at_kw(Keyword::Break) {
            let kw = self.bump();
            let mut span = kw.span;
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::Break,
            });
        }

        if self.at_kw(Keyword::Continue) {
            let kw = self.bump();
            let mut span = kw.span;
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::Continue,
            });
        }

        // 表达式语句 / 赋值语句（`stmtExpr ::= expr ('=' expr)?`，§7）。
        self.parse_expr_stmt()
    }

    /// 语句位表达式：允许恰好一个 `=`（赋值目标三态：Ident / Member / Index）。
    fn parse_expr_stmt(&mut self) -> PResult<Stmt> {
        let lhs = self.parse_expr_bp(0, InfixMode::Normal)?;

        if self.at_sym(Symbol::Eq) {
            let eq = self.bump();
            let Some(target) = self.classify_assign_target(lhs) else {
                // 非法 LHS：`?.` 链 / 调用 / 字面量等（§7）。
                return Err(self.err_assignment_target_invalid(eq));
            };
            let value = self.expr()?;
            let mut span = Span::new(target.span.start, value.span.end);
            if self.at_sym(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }
            return Ok(Stmt {
                id: self.nid(),
                span,
                kind: StmtKind::Assign { target, value },
            });
        }

        let mut span = lhs.span;
        if self.at_sym(Symbol::Semicolon) {
            let semi = self.bump();
            span = Span::new(span.start, semi.span.end);
        }
        Ok(Stmt {
            id: self.nid(),
            span,
            kind: StmtKind::Expr(lhs),
        })
    }

    fn err_assignment_target_invalid(&mut self, eq: Token) -> super::Abort {
        self.record(
            scoop2_base::diag::Diagnostic::error(
                "scoop::parse::assignment_expression_not_allowed",
                "语法错误：赋值左侧必须是标识符、成员访问或下标（`a[i]`）",
            )
            .with_primary(eq.span, "非法的赋值左侧")
            .with_help(
                "合法形式：`x = v` / `a.b = v` / `a[i] = v`；`?.` 链与调用结果不能作为赋值目标",
            ),
        );
        super::Abort
    }

    /// 赋值 LHS 三态分类（§7）：`x` / `a.b`（含 `t.0`）/ `a[i, j]`。
    fn classify_assign_target(&mut self, expr: Expr) -> Option<AssignTarget> {
        let span = expr.span;
        match expr.kind {
            ExprKind::Ident(ident) => Some(AssignTarget {
                id: self.nid(),
                span,
                kind: AssignTargetKind::Ident(ident),
            }),
            ExprKind::MemberAccess { receiver, member } => Some(AssignTarget {
                id: self.nid(),
                span,
                kind: AssignTargetKind::Member { receiver, member },
            }),
            ExprKind::Index { receiver, indices } => Some(AssignTarget {
                id: self.nid(),
                span,
                kind: AssignTargetKind::Index { receiver, indices },
            }),
            _ => None,
        }
    }

    fn is_expr_start_for_return(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Ident
                | TokenKind::IntLiteral
                | TokenKind::FloatLiteral
                | TokenKind::CharLiteral
                | TokenKind::StringLiteral(_)
                | TokenKind::Keyword(
                    Keyword::If
                        | Keyword::When
                        | Keyword::Handle
                        | Keyword::Try
                        | Keyword::Do
                        | Keyword::Object
                )
                | TokenKind::Symbol(
                    Symbol::LBrace
                        | Symbol::LParen
                        | Symbol::LBracket
                        | Symbol::At
                        | Symbol::Bang
                        | Symbol::Minus
                        | Symbol::Tilde
                )
        )
    }

    // --------------------------------------------------------------
    // 局部 val/var（§7 localValDecl）
    // --------------------------------------------------------------

    fn looks_like_annotated_local_val_decl(&self) -> bool {
        if !self.at_sym(Symbol::At) {
            return false;
        }
        let mut idx = self.i;
        while self.tokens.get(idx).map(|t| t.kind) == Some(TokenKind::Symbol(Symbol::At)) {
            idx = self.skip_one_annotation_idx(idx);
        }
        matches!(
            self.tokens.get(idx).map(|t| t.kind),
            Some(TokenKind::Keyword(Keyword::Val | Keyword::Var))
        )
    }

    fn parse_local_val_decl(&mut self) -> PResult<ValDecl> {
        let mut annotations = Vec::new();
        while self.at_sym(Symbol::At) {
            annotations.push(self.parse_annotation_use()?);
        }

        let kw = self.bump();
        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ValKind::Var,
            _ => {
                let tok = self.peek();
                return Err(self.err_expected_token("`val` / `var`", tok));
            }
        };

        // `var` 解构是专用 parse error（§3.3/§7；消费平衡分组以便恢复）。
        if kind == ValKind::Var
            && (self.at_sym(Symbol::LParen)
                || self.looks_like_struct_pattern_ahead()
                || self.looks_like_variant_pattern_ahead())
        {
            let tok = self.peek();
            // 找到模式的开括号并消费平衡分组，避免级联错误。
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
            if self.at_eof() || self.at_sym(Symbol::Semicolon) || self.at_sym(Symbol::RBrace) {
                let tok = self.peek();
                return Err(self.err_expected("表达式（initializer）", tok));
            }
            Some(self.expr()?)
        } else if matches!(binding, ValBinding::Pattern(_)) {
            let tok = self.peek();
            return Err(self.err_expected("`=`（解构绑定需要 initializer）", tok));
        } else {
            None
        };

        Ok(ValDecl {
            annotations,
            modifiers: Vec::new(),
            kind,
            binding,
            ty,
            init,
        })
    }

    // --------------------------------------------------------------
    // 恢复
    // --------------------------------------------------------------

    fn recover_stmt_after_error(&mut self) {
        // 若当前位置已经是潜在的语句边界，则不消费任何 token（外层循环继续）。
        if self.at_eof() || self.at_sym(Symbol::RBrace) || self.is_recovery_boundary_stmt_start() {
            return;
        }

        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        // 先吞掉一个 token，确保前进。
        let tok = self.bump();
        track_depth(tok, &mut depth_paren, &mut depth_brace, &mut depth_bracket);

        while !self.at_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.at_sym(Symbol::Semicolon)
                    || self.at_sym(Symbol::RBrace)
                    || self.is_recovery_boundary_stmt_start())
            {
                break;
            }
            let tok = self.bump();
            track_depth(tok, &mut depth_paren, &mut depth_brace, &mut depth_bracket);
        }

        // 若以 `;` 结束，吞掉分号，避免外层再产出空语句。
        self.eat_sym(Symbol::Semicolon);
    }
}

fn track_depth(
    tok: Token,
    depth_paren: &mut usize,
    depth_brace: &mut usize,
    depth_bracket: &mut usize,
) {
    match tok.kind {
        TokenKind::Symbol(Symbol::LParen) => *depth_paren += 1,
        TokenKind::Symbol(Symbol::RParen) => *depth_paren = depth_paren.saturating_sub(1),
        TokenKind::Symbol(Symbol::LBrace) => *depth_brace += 1,
        TokenKind::Symbol(Symbol::RBrace) => *depth_brace = depth_brace.saturating_sub(1),
        TokenKind::Symbol(Symbol::LBracket) => *depth_bracket += 1,
        TokenKind::Symbol(Symbol::RBracket) => *depth_bracket = depth_bracket.saturating_sub(1),
        _ => {}
    }
}

fn decl_span(decl: &ValDecl) -> Span {
    let start = decl
        .annotations
        .first()
        .map(|a| a.span.start)
        .unwrap_or_else(|| match &decl.binding {
            ValBinding::Name(name) => name.span.start,
            ValBinding::Pattern(pat) => pat.span.start,
        });
    let end = decl
        .init
        .as_ref()
        .map(|e| e.span.end)
        .or_else(|| decl.ty.as_ref().map(|t| t.span.end))
        .unwrap_or(match &decl.binding {
            ValBinding::Name(name) => name.span.end,
            ValBinding::Pattern(pat) => pat.span.end,
        });
    Span::new(start, end)
}

/// 判断语句是否吃掉了尾随 `;`（span 被扩展到 `;`）：比较 stmt span 末端与
/// 源码（`parse_block_with_open` 中 lambda 解包规则需要）。
fn stmt_had_trailing_semi(stmt: &Stmt, source: &str) -> bool {
    stmt.span.end > 0 && source.get(stmt.span.end - 1..stmt.span.end) == Some(";")
}
