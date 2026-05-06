//! 语句 lowering（TODO T0103d）。
//!
//! 说明：
//! - 该模块负责 AST → HIR 的语句部分 lowering；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::span::Span;

use super::HirLowering;
use super::ValScope;
use super::types::ExpectedExpr;

use super::super::{
    Block, CallArg, Expr, ExprKind, LiteralKind, MemberAccess, MemberRef, Stmt, StmtKind, ValDecl,
    ValueRef,
};

impl<'a> HirLowering<'a> {
    fn lower_synthetic_dispatch_call(
        &mut self,
        span: Span,
        receiver: Expr,
        receiver_ty: crate::ty::TypeId,
        method_fqn: &str,
        return_ty: crate::ty::TypeId,
    ) -> Expr {
        let mut target_fqn = method_fqn.to_string();
        if let Some((owner_fqn, member_name)) = method_fqn.rsplit_once('.') {
            let dispatch_kind = if matches!(
                self.type_kinds.get(owner_fqn),
                Some(ast::TypeKind::Interface)
            ) {
                Some(crate::hir::DispatchCallKind::Interface)
            } else if matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class))
                && self.class_vtables.get(owner_fqn).is_some_and(|slots| {
                    slots
                        .iter()
                        .any(|slot| slot.name == member_name && slot.params_len == 0)
                })
            {
                Some(crate::hir::DispatchCallKind::Virtual)
            } else {
                None
            };

            if let Some(dispatch_kind) = dispatch_kind {
                if self.devirtualize_dispatch_calls {
                    if let Some(devirtualized_target_fqn) =
                        crate::devirtualize::try_devirtualize_dispatch_target(
                            dispatch_kind,
                            owner_fqn,
                            member_name,
                            0,
                            receiver_ty,
                            self.types,
                            crate::devirtualize::DispatchTargetFacts {
                                known_receiver_subclasses: self.known_receiver_subclasses,
                                class_vtables: self.class_vtables,
                                interfaces: self.interfaces,
                                class_itables: self.class_itables,
                            },
                        )
                    {
                        target_fqn = self.materialized_devirtualized_dispatch_target_fqn(
                            span,
                            &devirtualized_target_fqn,
                        );
                    } else {
                        self.dispatch_call_sites
                            .insert(self.dispatch_call_site(span, receiver_ty), dispatch_kind);
                    }
                } else {
                    self.dispatch_call_sites
                        .insert(self.dispatch_call_site(span, receiver_ty), dispatch_kind);
                }
            }
        }

        Expr {
            span,
            ty: return_ty,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(target_fqn.clone()),
                        fqn: target_fqn,
                    }),
                }),
                args: vec![CallArg::Positional(receiver)],
            },
        }
    }

    pub(super) fn lower_stmt_into_with_tail(
        &mut self,
        pkg_prefix: &str,
        s: &ast::Stmt,
        out: &mut Vec<Stmt>,
        expected: ExpectedExpr,
        is_tail: bool,
    ) -> bool {
        if self.runtime_comptime_plan.is_some() {
            match &s.kind {
                ast::StmtKind::ComptimeBlock { body, .. } => {
                    return self
                        .lower_comptime_block_into(pkg_prefix, body, out, expected, is_tail);
                }
                ast::StmtKind::ComptimeIf(ci) => {
                    return self.lower_comptime_if_into(pkg_prefix, ci, out, expected, is_tail);
                }
                ast::StmtKind::ComptimeFor(cf) => {
                    return self.lower_comptime_for_into(pkg_prefix, cf, out, expected, is_tail);
                }
                _ => {}
            }
        }

        if let ast::StmtKind::Val(v) = &s.kind
            && matches!(v.binding, ast::ValBinding::Pattern(_))
        {
            self.lower_local_pattern_val_stmt(pkg_prefix, s.span, v, out);
            return false;
        }

        if is_tail
            && let ast::StmtKind::Expr(expr) = &s.kind
            && !matches!(expr.kind, ast::ExprKind::Assign { .. })
        {
            let expr = self.lower_expr_with_expected(pkg_prefix, expr, expected);
            out.push(Stmt {
                span: s.span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(expr),
            });
            return true;
        }

        out.push(self.lower_stmt_single(pkg_prefix, s));
        false
    }

    pub(super) fn lower_block_stmts_into(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        out: &mut Vec<Stmt>,
        expected: ExpectedExpr,
        outer_tail: bool,
    ) -> bool {
        let mut last_is_tail = false;
        for (index, stmt) in body.stmts.iter().enumerate() {
            let is_last = index + 1 == body.stmts.len();
            let stmt_tail = outer_tail && is_last && !stmt.has_trailing_semi;
            last_is_tail =
                self.lower_stmt_into_with_tail(pkg_prefix, stmt, out, expected, stmt_tail);
        }
        last_is_tail
    }

    fn lower_comptime_block_into(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        out: &mut Vec<Stmt>,
        expected: ExpectedExpr,
        is_tail: bool,
    ) -> bool {
        self.lower_block_stmts_into(pkg_prefix, body, out, expected, is_tail)
    }

    fn lower_comptime_if_into(
        &mut self,
        pkg_prefix: &str,
        ci: &ast::ComptimeIf,
        out: &mut Vec<Stmt>,
        expected: ExpectedExpr,
        is_tail: bool,
    ) -> bool {
        let Some(plan) = self.runtime_comptime_plan else {
            out.push(Stmt {
                span: ci.span,
                ty: self.builtins.unit,
                kind: StmtKind::Todo("comptime_if"),
            });
            return false;
        };
        let Some(cond) = plan.comptime_if_condition(ci.span) else {
            out.push(Stmt {
                span: ci.span,
                ty: self.builtins.unit,
                kind: StmtKind::Todo("comptime_if"),
            });
            return false;
        };

        if cond {
            return self.lower_block_stmts_into(
                pkg_prefix,
                &ci.then_branch,
                out,
                expected,
                is_tail,
            );
        }

        match &ci.else_branch {
            None => false,
            Some(else_branch) => match &**else_branch {
                ast::ComptimeIfElse::Block(block) => {
                    self.lower_block_stmts_into(pkg_prefix, block, out, expected, is_tail)
                }
                ast::ComptimeIfElse::If(next) => {
                    self.lower_comptime_if_into(pkg_prefix, next, out, expected, is_tail)
                }
            },
        }
    }

    fn lower_comptime_for_into(
        &mut self,
        pkg_prefix: &str,
        cf: &ast::ComptimeFor,
        out: &mut Vec<Stmt>,
        expected: ExpectedExpr,
        is_tail: bool,
    ) -> bool {
        let Some(plan) = self.runtime_comptime_plan else {
            out.push(Stmt {
                span: cf.span,
                ty: self.builtins.unit,
                kind: StmtKind::Todo("comptime_for"),
            });
            return false;
        };
        let Some(iterations) = plan.comptime_for_iterations(cf.span) else {
            out.push(Stmt {
                span: cf.span,
                ty: self.builtins.unit,
                kind: StmtKind::Todo("comptime_for"),
            });
            return false;
        };
        let iterations = iterations.to_vec();
        let local_decl_spans = collect_local_decl_spans_in_block(&cf.body);
        let mut last_is_tail = false;
        let iteration_count = iterations.len();

        for (index, value) in iterations.into_iter().enumerate() {
            let mut overrides = std::collections::HashMap::new();
            for span in &local_decl_spans {
                let (synthetic_span, _, _) =
                    self.fresh_synthetic_local(*span, "__comptime_unroll", false);
                overrides.insert(*span, synthetic_span);
            }
            let iteration_tail = is_tail && index + 1 == iteration_count;
            last_is_tail = self.with_comptime_value_binding(cf.binder.span, value, |this| {
                this.with_local_decl_span_overrides(overrides, |this| {
                    this.lower_block_stmts_into(pkg_prefix, &cf.body, out, expected, iteration_tail)
                })
            });
        }

        last_is_tail
    }

    fn lower_stmt_single(&mut self, pkg_prefix: &str, s: &ast::Stmt) -> Stmt {
        let (kind, ty) = match &s.kind {
            ast::StmtKind::Empty => (StmtKind::Empty, self.builtins.unit),
            ast::StmtKind::Expr(e) => {
                // 赋值在 AST 中以表达式节点承载，但在 HIR 中视为语句（便于后续 MIR lowering）。
                if let ast::ExprKind::Assign { lhs, eq_span, rhs } = &e.kind {
                    if let Some(call) =
                        self.try_lower_delegated_property_assign(pkg_prefix, e.span, lhs, rhs)
                    {
                        (StmtKind::Expr(call), self.builtins.unit)
                    } else {
                        let lhs = self.lower_expr(pkg_prefix, lhs);
                        let rhs = self.lower_expr(pkg_prefix, rhs);
                        (
                            StmtKind::Assign {
                                lhs,
                                eq_span: *eq_span,
                                rhs,
                            },
                            self.builtins.unit,
                        )
                    }
                } else {
                    let e = self.lower_expr(pkg_prefix, e);
                    (StmtKind::Expr(e), self.builtins.unit)
                }
            }
            ast::StmtKind::Val(v) => {
                let v = self.lower_val_decl(pkg_prefix, v, ValScope::Local);
                (StmtKind::Val(v), self.builtins.unit)
            }
            ast::StmtKind::Return { value, .. } => {
                let value = value.as_ref().map(|e| self.lower_expr(pkg_prefix, e));
                (StmtKind::Return { value }, self.builtins.nothing)
            }
            ast::StmtKind::Missing => (StmtKind::Todo("missing_stmt"), self.builtins.unit),
            ast::StmtKind::While { cond, body, .. } => (
                StmtKind::While {
                    cond: self.lower_expr(pkg_prefix, cond),
                    body: self.lower_block(pkg_prefix, body),
                },
                self.builtins.unit,
            ),
            ast::StmtKind::For(f) => {
                return self.lower_for_stmt(pkg_prefix, s.span, f);
            }
            ast::StmtKind::Break { break_span } => (
                StmtKind::Break {
                    break_span: *break_span,
                },
                self.builtins.unit,
            ),
            ast::StmtKind::Continue { continue_span } => (
                StmtKind::Continue {
                    continue_span: *continue_span,
                },
                self.builtins.unit,
            ),
            ast::StmtKind::ComptimeBlock { .. } => {
                (StmtKind::Todo("comptime_block"), self.builtins.unit)
            }
            ast::StmtKind::ComptimeIf(_) => (StmtKind::Todo("comptime_if"), self.builtins.unit),
            ast::StmtKind::ComptimeFor(_) => (StmtKind::Todo("comptime_for"), self.builtins.unit),
        };

        Stmt {
            span: s.span,
            ty,
            kind,
        }
    }

    /// `for (x in xs) { body }` の HIR lowering（T0110）。
    ///
    /// typecheck 写回の `resolved_for_info` に基づき、型別の降糖を行う：
    /// - `ArrayInt`: 索引ベースの while ループ
    /// - `IntProgression`: progression while ループ
    /// - `Custom`: `iterator()/next(): Option<T>` を while + when に展開
    fn lower_for_stmt(&mut self, pkg_prefix: &str, stmt_span: Span, f: &ast::ForStmt) -> Stmt {
        let info = f.resolved_for_info.get();
        let kind = info.map(|i| &i.kind);

        match kind {
            Some(ast::ForLoopIterableKind::ArrayInt) => {
                self.lower_for_array_int(pkg_prefix, stmt_span, f)
            }
            Some(ast::ForLoopIterableKind::IntProgression) => {
                self.lower_for_int_progression(pkg_prefix, stmt_span, f)
            }
            Some(ast::ForLoopIterableKind::Custom) => {
                let Some(custom) = info.and_then(|info| info.custom.as_ref()) else {
                    return Stmt {
                        span: stmt_span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Todo("for_custom_iterator"),
                    };
                };
                self.lower_for_custom_iterator(pkg_prefix, stmt_span, f, custom)
            }
            _ => {
                // Custom iterator or dump-hir (no typecheck) fallback.
                Stmt {
                    span: stmt_span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Todo("for_custom_iterator"),
                }
            }
        }
    }

    /// `for (x in arr) { body }` — Array<Int> 降糖：
    ///
    /// ```text
    /// {
    ///     val __for_arr = arr
    ///     var __for_i = 0
    ///     while (__for_i < size(__for_arr)) {
    ///         val x = get(__for_arr, __for_i)
    ///         ...body_stmts...
    ///         __for_i = __for_i + 1
    ///     }
    /// }
    /// ```
    fn lower_for_array_int(&mut self, pkg_prefix: &str, stmt_span: Span, f: &ast::ForStmt) -> Stmt {
        let span = f.span;
        let for_span = f.for_span;

        // Lower the iterable expression.
        let iter_lowered = self.lower_expr(pkg_prefix, &f.iter);

        // 各合成変数に異なる decl_span を付与（同一 span → 同一 SymbolId を回避）。
        let arr_span = Span::new(for_span.start, for_span.start + 1);
        let idx_span = Span::new(for_span.start + 1, for_span.start + 2);

        // Synthesize: val __for_arr = <iter>
        let arr_id = self.intern_local_symbol(arr_span, false);
        let arr_name = "__for_arr".to_string();
        // Array<T> は Ref 型なので、iter_lowered.ty がすでに Ref であるべきだが、
        // dump-hir (typecheck なし) パスでは any になる場合がある。any も CgTy::Ref に落ちるので OK。
        let arr_ty = iter_lowered.ty;
        let arr_decl = Stmt {
            span: arr_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span: arr_span,
                id: Some(arr_id),
                name: Some(arr_name.clone()),
                mutable: false,
                ty: arr_ty,
                init: Some(iter_lowered),
            }),
        };

        let int = self.builtins.int;
        let bool_ = self.builtins.bool_;
        let unit = self.builtins.unit;
        let any = self.builtins.any;

        // Synthesize: var __for_i = 0
        let idx_id = self.intern_local_symbol(idx_span, true);
        let idx_name = "__for_i".to_string();
        let zero = Expr {
            span: idx_span,
            ty: int,
            kind: ExprKind::Literal(LiteralKind::SynthInt(0)),
        };
        let idx_decl = Stmt {
            span: idx_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: idx_span,
                id: Some(idx_id),
                name: Some(idx_name.clone()),
                mutable: true,
                ty: int,
                init: Some(zero),
            }),
        };

        // Helper: __for_arr ref
        let arr_ref = |span: Span| Expr {
            span,
            ty: arr_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: arr_id,
                name: arr_name.clone(),
                decl_span: arr_span,
            }),
        };

        // Helper: __for_i ref
        let idx_ref = |span: Span| Expr {
            span,
            ty: int,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: idx_id,
                name: idx_name.clone(),
                decl_span: idx_span,
            }),
        };

        // size(__for_arr)
        let size_fqn = "scoop.core.size".to_string();
        let size_call = Expr {
            span: for_span,
            ty: int,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span: for_span,
                    ty: any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(size_fqn.clone()),
                        fqn: size_fqn,
                    }),
                }),
                args: vec![CallArg::Positional(arr_ref(for_span))],
            },
        };

        // __for_i < size(__for_arr)
        let cond = Expr {
            span: for_span,
            ty: bool_,
            kind: ExprKind::Binary {
                lhs: Box::new(idx_ref(for_span)),
                op: ast::BinaryOp::Lt,
                op_span: for_span,
                rhs: Box::new(size_call),
            },
        };

        // get(__for_arr, __for_i)
        let get_fqn = "scoop.core.get".to_string();
        let get_call = Expr {
            span: for_span,
            ty: int,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span: for_span,
                    ty: any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(get_fqn.clone()),
                        fqn: get_fqn,
                    }),
                }),
                args: vec![
                    CallArg::Positional(arr_ref(for_span)),
                    CallArg::Positional(idx_ref(for_span)),
                ],
            },
        };

        // val x = get(__for_arr, __for_i)
        let binder_name = self.source.slice(f.binder.span).to_string();
        let binder_id = self.intern_local_symbol(f.binder.span, false);
        let binder_decl = Stmt {
            span: f.binder.span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: f.binder.span,
                id: Some(binder_id),
                name: Some(binder_name),
                mutable: false,
                ty: int,
                init: Some(get_call),
            }),
        };

        // Lower the body stmts
        let body_block = self.lower_block(pkg_prefix, &f.body);

        // __for_i = __for_i + 1
        let one = Expr {
            span: for_span,
            ty: int,
            kind: ExprKind::Literal(LiteralKind::SynthInt(1)),
        };
        let increment = Expr {
            span: for_span,
            ty: int,
            kind: ExprKind::Binary {
                lhs: Box::new(idx_ref(for_span)),
                op: ast::BinaryOp::Add,
                op_span: for_span,
                rhs: Box::new(one),
            },
        };
        let assign_stmt = Stmt {
            span: for_span,
            ty: unit,
            kind: StmtKind::Assign {
                lhs: idx_ref(for_span),
                eq_span: for_span,
                rhs: increment,
            },
        };

        // Build the while body: val x = get(...); body_stmts; __for_i = __for_i + 1
        let mut while_stmts = Vec::with_capacity(body_block.stmts.len() + 2);
        while_stmts.push(binder_decl);
        while_stmts.extend(body_block.stmts);
        while_stmts.push(assign_stmt);

        let while_body = Block {
            span,
            ty: unit,
            stmts: while_stmts,
        };

        // while (__for_i < size(__for_arr)) { ... }
        let while_stmt = Stmt {
            span,
            ty: unit,
            kind: StmtKind::While {
                cond,
                body: while_body,
            },
        };

        // Wrap in block: { val __for_arr = ...; var __for_i = 0; while ... }
        let block = Block {
            span,
            ty: unit,
            stmts: vec![arr_decl, idx_decl, while_stmt],
        };

        Stmt {
            span: stmt_span,
            ty: unit,
            kind: StmtKind::Expr(Expr {
                span,
                ty: unit,
                kind: ExprKind::Block(block),
            }),
        }
    }

    /// `for (x in prog) { body }` — IntProgression 降糖：
    ///
    /// ```text
    /// {
    ///     val __for_prog = prog
    ///     var __for_cur = __for_prog.first
    ///     if (__for_prog.increasing) {
    ///         while (__for_cur <= __for_prog.last) {
    ///             val x = __for_cur
    ///             ...body_stmts...
    ///             __for_cur = __for_cur + __for_prog.step
    ///         }
    ///     } else {
    ///         while (__for_cur >= __for_prog.last) {
    ///             val x = __for_cur
    ///             ...body_stmts...
    ///             __for_cur = __for_cur - __for_prog.step
    ///         }
    ///     }
    /// }
    /// ```
    fn lower_for_int_progression(
        &mut self,
        pkg_prefix: &str,
        stmt_span: Span,
        f: &ast::ForStmt,
    ) -> Stmt {
        let span = f.span;
        let for_span = f.for_span;

        let iter_lowered = self.lower_expr(pkg_prefix, &f.iter);

        let int = self.builtins.int;
        let bool_ = self.builtins.bool_;
        let unit = self.builtins.unit;
        let prog_ty =
            self.intern_nominal("scoop.core.IntProgression".to_string(), Vec::new(), None);

        // 各合成変数に異なる decl_span を付与（同一 span → 同一 SymbolId を回避）。
        let prog_span = Span::new(for_span.start, for_span.start + 1);
        let cur_span = Span::new(for_span.start + 1, for_span.start + 2);

        // val __for_prog = <iter>
        let prog_id = self.intern_local_symbol(prog_span, false);
        let prog_name = "__for_prog".to_string();
        let prog_decl = Stmt {
            span: prog_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: prog_span,
                id: Some(prog_id),
                name: Some(prog_name.clone()),
                mutable: false,
                ty: prog_ty,
                init: Some(iter_lowered),
            }),
        };

        // Helper: __for_prog ref
        let prog_ref = |span: Span| Expr {
            span,
            ty: prog_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: prog_id,
                name: prog_name.clone(),
                decl_span: prog_span,
            }),
        };

        // Helper: field access on __for_prog (field_ty: Int for first/last/step, Bool for increasing)
        let prog_field = |me: &mut Self, span: Span, field: &str, field_ty| -> Expr {
            let fqn = format!("scoop.core.IntProgression.{field}");
            Expr {
                span,
                ty: field_ty,
                kind: ExprKind::MemberAccess {
                    receiver: Box::new(prog_ref(span)),
                    member: MemberAccess {
                        span,
                        name: field.to_string(),
                        resolved: Some(MemberRef::Value {
                            id: me.symbols.intern_top_level(fqn.clone()),
                            fqn,
                        }),
                    },
                },
            }
        };

        // var __for_cur = __for_prog.first
        let cur_id = self.intern_local_symbol(cur_span, true);
        let cur_name = "__for_cur".to_string();
        let first_access = prog_field(self, for_span, "first", int);
        let cur_decl = Stmt {
            span: cur_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: cur_span,
                id: Some(cur_id),
                name: Some(cur_name.clone()),
                mutable: true,
                ty: int,
                init: Some(first_access),
            }),
        };

        // Helper: __for_cur ref
        let cur_ref = |span: Span| Expr {
            span,
            ty: int,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: cur_id,
                name: cur_name.clone(),
                decl_span: cur_span,
            }),
        };

        // Build the inner while body for a given direction.
        let build_while = |me: &mut Self, ascending: bool| -> Stmt {
            // condition: __for_cur <= __for_prog.last (ascending) or __for_cur >= __for_prog.last (descending)
            let cmp_op = if ascending {
                ast::BinaryOp::Le
            } else {
                ast::BinaryOp::Ge
            };
            let last_access = prog_field(me, for_span, "last", int);
            let cond = Expr {
                span: for_span,
                ty: bool_,
                kind: ExprKind::Binary {
                    lhs: Box::new(cur_ref(for_span)),
                    op: cmp_op,
                    op_span: for_span,
                    rhs: Box::new(last_access),
                },
            };

            // val x = __for_cur
            let binder_name = me.source.slice(f.binder.span).to_string();
            let binder_id = me.intern_local_symbol(f.binder.span, false);
            let binder_decl = Stmt {
                span: f.binder.span,
                ty: unit,
                kind: StmtKind::Val(ValDecl {
                    span: f.binder.span,
                    id: Some(binder_id),
                    name: Some(binder_name),
                    mutable: false,
                    ty: int,
                    init: Some(cur_ref(for_span)),
                }),
            };

            // Lower the body
            let body_block = me.lower_block(pkg_prefix, &f.body);

            // __for_cur = __for_cur +/- __for_prog.step
            let step_op = if ascending {
                ast::BinaryOp::Add
            } else {
                ast::BinaryOp::Sub
            };
            let step_access = prog_field(me, for_span, "step", int);
            let step_expr = Expr {
                span: for_span,
                ty: int,
                kind: ExprKind::Binary {
                    lhs: Box::new(cur_ref(for_span)),
                    op: step_op,
                    op_span: for_span,
                    rhs: Box::new(step_access),
                },
            };
            let assign_stmt = Stmt {
                span: for_span,
                ty: unit,
                kind: StmtKind::Assign {
                    lhs: cur_ref(for_span),
                    eq_span: for_span,
                    rhs: step_expr,
                },
            };

            // while body
            let mut while_stmts = Vec::with_capacity(body_block.stmts.len() + 2);
            while_stmts.push(binder_decl);
            while_stmts.extend(body_block.stmts);
            while_stmts.push(assign_stmt);

            let while_body = Block {
                span,
                ty: unit,
                stmts: while_stmts,
            };

            Stmt {
                span,
                ty: unit,
                kind: StmtKind::While {
                    cond,
                    body: while_body,
                },
            }
        };

        let asc_while = build_while(self, true);
        let desc_while = build_while(self, false);

        // if (__for_prog.increasing) { asc_while } else { desc_while }
        let increasing_access = prog_field(self, for_span, "increasing", bool_);
        let if_stmt = Stmt {
            span,
            ty: unit,
            kind: StmtKind::Expr(Expr {
                span,
                ty: unit,
                kind: ExprKind::If {
                    cond: Box::new(increasing_access),
                    then_branch: Box::new(Expr {
                        span,
                        ty: unit,
                        kind: ExprKind::Block(Block {
                            span,
                            ty: unit,
                            stmts: vec![asc_while],
                        }),
                    }),
                    else_branch: Some(Box::new(Expr {
                        span,
                        ty: unit,
                        kind: ExprKind::Block(Block {
                            span,
                            ty: unit,
                            stmts: vec![desc_while],
                        }),
                    })),
                },
            }),
        };

        // Wrap: { val __for_prog = ...; var __for_cur = ...; if (...) { ... } else { ... } }
        let block = Block {
            span,
            ty: unit,
            stmts: vec![prog_decl, cur_decl, if_stmt],
        };

        Stmt {
            span: stmt_span,
            ty: unit,
            kind: StmtKind::Expr(Expr {
                span,
                ty: unit,
                kind: ExprKind::Block(block),
            }),
        }
    }

    /// `for (x in iterable) { body }` — 自定义 iterator 协议降糖：
    ///
    /// ```text
    /// {
    ///     val __for_iterable = iterable
    ///     val __for_iter = __for_iterable.iterator()
    ///     var __for_running = true
    ///     while (__for_running) {
    ///         when (__for_iter.next()) {
    ///             Some(__for_value) -> {
    ///                 val x = __for_value
    ///                 ...body_stmts...
    ///             }
    ///             None -> { __for_running = false }
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// 关键约束：
    /// - `iterable` 与 `iterator()` 只求值一次；
    /// - `next()` 每轮循环只求值一次，并由 `Option<T>` 驱动退出。
    fn lower_for_custom_iterator(
        &mut self,
        pkg_prefix: &str,
        stmt_span: Span,
        f: &ast::ForStmt,
        info: &ast::ForLoopCustomResolvedInfo,
    ) -> Stmt {
        let span = f.span;
        let for_span = f.for_span;
        let unit = self.builtins.unit;
        let bool_ = self.builtins.bool_;

        let iterable_lowered = self.lower_expr(pkg_prefix, &f.iter);
        let iterable_ty = iterable_lowered.ty;
        let iterator_ty = self
            .typecheck_types
            .map(|typecheck_types| self.types.re_intern_from(typecheck_types, info.iterator_ty))
            .unwrap_or(self.builtins.any);
        let elem_ty = self
            .typecheck_types
            .map(|typecheck_types| self.types.re_intern_from(typecheck_types, info.elem_ty))
            .unwrap_or(self.builtins.any);
        let next_result_ty = self.types.ty_option(elem_ty);

        // 各合成局部使用不同的 span，避免与用户 binder 或其它临时变量复用同一个 SymbolId。
        let iterable_span = Span::new(for_span.start, for_span.start + 1);
        let iterator_span = Span::new(for_span.start + 1, for_span.start + 2);
        let running_span = Span::new(for_span.start + 2, for_span.start + 3);
        let item_span = Span::new(for_span.start + 3, for_span.start + 4);

        // val __for_iterable = <iterable>
        let iterable_id = self.intern_local_symbol(iterable_span, false);
        let iterable_name = "__for_iterable".to_string();
        let iterable_decl = Stmt {
            span: iterable_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: iterable_span,
                id: Some(iterable_id),
                name: Some(iterable_name.clone()),
                mutable: false,
                ty: iterable_ty,
                init: Some(iterable_lowered),
            }),
        };

        let iterable_ref = |span: Span| Expr {
            span,
            ty: iterable_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: iterable_id,
                name: iterable_name.clone(),
                decl_span: iterable_span,
            }),
        };

        // val __for_iter = Iterable.iterator(__for_iterable)
        let iterator_call = self.lower_synthetic_dispatch_call(
            for_span,
            iterable_ref(for_span),
            iterable_ty,
            &info.iterator_method_fqn,
            iterator_ty,
        );
        let iterator_id = self.intern_local_symbol(iterator_span, false);
        let iterator_name = "__for_iter".to_string();
        let iterator_decl = Stmt {
            span: iterator_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: iterator_span,
                id: Some(iterator_id),
                name: Some(iterator_name.clone()),
                mutable: false,
                ty: iterator_ty,
                init: Some(iterator_call),
            }),
        };

        // var __for_running = true
        let running_id = self.intern_local_symbol(running_span, true);
        let running_name = "__for_running".to_string();
        let running_decl = Stmt {
            span: running_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: running_span,
                id: Some(running_id),
                name: Some(running_name.clone()),
                mutable: true,
                ty: bool_,
                init: Some(Expr {
                    span: running_span,
                    ty: bool_,
                    kind: ExprKind::Literal(LiteralKind::Bool(true)),
                }),
            }),
        };

        let iterator_ref = |span: Span| Expr {
            span,
            ty: iterator_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: iterator_id,
                name: iterator_name.clone(),
                decl_span: iterator_span,
            }),
        };

        let running_ref = |span: Span| Expr {
            span,
            ty: bool_,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: running_id,
                name: running_name.clone(),
                decl_span: running_span,
            }),
        };

        // __for_iter.next()
        let next_call = self.lower_synthetic_dispatch_call(
            for_span,
            iterator_ref(for_span),
            iterator_ty,
            &info.next_method_fqn,
            next_result_ty,
        );

        // Some(__for_value) -> { val x = __for_value; ...body... }
        let item_id = self.intern_local_symbol(item_span, false);
        let item_name = "__for_value".to_string();
        let item_ref = |span: Span| Expr {
            span,
            ty: elem_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: item_id,
                name: item_name.clone(),
                decl_span: item_span,
            }),
        };

        let binder_name = self.source.slice(f.binder.span).to_string();
        let binder_id = self.intern_local_symbol(f.binder.span, false);
        let binder_decl = Stmt {
            span: f.binder.span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: f.binder.span,
                id: Some(binder_id),
                name: Some(binder_name),
                mutable: false,
                ty: elem_ty,
                init: Some(item_ref(f.binder.span)),
            }),
        };

        let body_block = self.lower_block(pkg_prefix, &f.body);
        let mut some_body_stmts = Vec::with_capacity(body_block.stmts.len() + 1);
        some_body_stmts.push(binder_decl);
        some_body_stmts.extend(body_block.stmts);

        self.record_when_pat_binding_ty(item_span, elem_ty);
        let some_arm = super::super::WhenArm {
            span: for_span,
            pat: super::super::WhenPat::Variant {
                span: for_span,
                name_span: for_span,
                name: "Some".to_string(),
                args: vec![super::super::WhenPat::Bind {
                    span: item_span,
                    id: item_id,
                    name: item_name.clone(),
                }],
            },
            guard: None,
            arrow_span: for_span,
            body: Expr {
                span,
                ty: unit,
                kind: ExprKind::Block(Block {
                    span,
                    ty: unit,
                    stmts: some_body_stmts,
                }),
            },
        };

        let none_arm = super::super::WhenArm {
            span: for_span,
            pat: super::super::WhenPat::Variant {
                span: for_span,
                name_span: for_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: for_span,
            body: Expr {
                span,
                ty: unit,
                kind: ExprKind::Block(Block {
                    span,
                    ty: unit,
                    stmts: vec![Stmt {
                        span: for_span,
                        ty: unit,
                        kind: StmtKind::Assign {
                            lhs: running_ref(for_span),
                            eq_span: for_span,
                            rhs: Expr {
                                span: for_span,
                                ty: bool_,
                                kind: ExprKind::Literal(LiteralKind::Bool(false)),
                            },
                        },
                    }],
                }),
            },
        };

        let when_stmt = Stmt {
            span: for_span,
            ty: unit,
            kind: StmtKind::Expr(Expr {
                span: for_span,
                ty: unit,
                kind: ExprKind::When {
                    subject: Box::new(next_call),
                    arms: vec![some_arm, none_arm],
                },
            }),
        };

        let while_stmt = Stmt {
            span,
            ty: unit,
            kind: StmtKind::While {
                cond: running_ref(for_span),
                body: Block {
                    span,
                    ty: unit,
                    stmts: vec![when_stmt],
                },
            },
        };

        let block = Block {
            span,
            ty: unit,
            stmts: vec![iterable_decl, iterator_decl, running_decl, while_stmt],
        };

        Stmt {
            span: stmt_span,
            ty: unit,
            kind: StmtKind::Expr(Expr {
                span,
                ty: unit,
                kind: ExprKind::Block(block),
            }),
        }
    }
}

fn collect_local_decl_spans_in_block(block: &ast::Block) -> Vec<Span> {
    let mut out = Vec::new();
    for stmt in &block.stmts {
        collect_local_decl_spans_in_stmt(stmt, &mut out);
    }
    out
}

fn collect_local_decl_spans_in_stmt(stmt: &ast::Stmt, out: &mut Vec<Span>) {
    match &stmt.kind {
        ast::StmtKind::Val(v) => {
            for ident in v.binding.bound_idents() {
                out.push(ident.span);
            }
            if let Some(init) = &v.init {
                collect_local_decl_spans_in_expr(init, out);
            }
        }
        ast::StmtKind::Expr(expr) => collect_local_decl_spans_in_expr(expr, out),
        ast::StmtKind::Return { value, .. } => {
            if let Some(value) = value {
                collect_local_decl_spans_in_expr(value, out);
            }
        }
        ast::StmtKind::While { cond, body, .. } => {
            collect_local_decl_spans_in_expr(cond, out);
            collect_local_decl_spans_in_block_into(body, out);
        }
        ast::StmtKind::For(f) => {
            out.push(f.binder.span);
            collect_local_decl_spans_in_expr(&f.iter, out);
            collect_local_decl_spans_in_block_into(&f.body, out);
        }
        ast::StmtKind::ComptimeBlock { body, .. } => {
            collect_local_decl_spans_in_block_into(body, out)
        }
        ast::StmtKind::ComptimeIf(ci) => collect_local_decl_spans_in_comptime_if(ci, out),
        ast::StmtKind::ComptimeFor(cf) => {
            out.push(cf.binder.span);
            collect_local_decl_spans_in_expr(&cf.iter, out);
            collect_local_decl_spans_in_block_into(&cf.body, out);
        }
        ast::StmtKind::Break { .. }
        | ast::StmtKind::Continue { .. }
        | ast::StmtKind::Empty
        | ast::StmtKind::Missing => {}
    }
}

fn collect_local_decl_spans_in_block_into(block: &ast::Block, out: &mut Vec<Span>) {
    for stmt in &block.stmts {
        collect_local_decl_spans_in_stmt(stmt, out);
    }
}

fn collect_local_decl_spans_in_comptime_if(ci: &ast::ComptimeIf, out: &mut Vec<Span>) {
    collect_local_decl_spans_in_expr(&ci.cond, out);
    collect_local_decl_spans_in_block_into(&ci.then_branch, out);
    if let Some(else_branch) = &ci.else_branch {
        match &**else_branch {
            ast::ComptimeIfElse::Block(block) => collect_local_decl_spans_in_block_into(block, out),
            ast::ComptimeIfElse::If(next) => collect_local_decl_spans_in_comptime_if(next, out),
        }
    }
}

fn collect_local_decl_spans_in_expr(expr: &ast::Expr, out: &mut Vec<Span>) {
    match &expr.kind {
        ast::ExprKind::Block(block)
        | ast::ExprKind::DoBlock { body: block, .. }
        | ast::ExprKind::UnsafeBlock { body: block, .. }
        | ast::ExprKind::SafeBlock { body: block, .. }
        | ast::ExprKind::Async { body: block }
        | ast::ExprKind::Spawn { body: block } => {
            collect_local_decl_spans_in_block_into(block, out)
        }
        ast::ExprKind::Lambda(lambda) => {
            for param in &lambda.params {
                out.push(param.name.span);
            }
            collect_local_decl_spans_in_expr(&lambda.body, out);
        }
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_local_decl_spans_in_expr(cond, out);
            collect_local_decl_spans_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_local_decl_spans_in_expr(else_branch, out);
            }
        }
        ast::ExprKind::When { subject, arms } => {
            collect_local_decl_spans_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_local_decl_spans_in_expr(guard, out);
                }
                collect_local_decl_spans_in_expr(&arm.body, out);
            }
        }
        ast::ExprKind::Call { callee, args } => {
            collect_local_decl_spans_in_expr(callee, out);
            for arg in args {
                collect_local_decl_spans_in_expr(arg, out);
            }
        }
        ast::ExprKind::TupleLit { elements } | ast::ExprKind::ArrayLit { elements } => {
            for element in elements {
                collect_local_decl_spans_in_expr(element, out);
            }
        }
        ast::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_local_decl_spans_in_expr(&field.value, out);
            }
        }
        ast::ExprKind::MemberAccess { receiver, .. }
        | ast::ExprKind::SafeMemberAccess { receiver, .. }
        | ast::ExprKind::NotNullAssert { expr: receiver, .. }
        | ast::ExprKind::Await { expr: receiver, .. }
        | ast::ExprKind::Join { expr: receiver, .. } => {
            collect_local_decl_spans_in_expr(receiver, out)
        }
        ast::ExprKind::SpliceField { receiver, field } => {
            collect_local_decl_spans_in_expr(receiver, out);
            collect_local_decl_spans_in_expr(field, out);
        }
        ast::ExprKind::TypeApply { callee, .. } => collect_local_decl_spans_in_expr(callee, out),
        ast::ExprKind::SpreadArg { expr, .. }
        | ast::ExprKind::Annotated { expr, .. }
        | ast::ExprKind::Unary { expr, .. }
        | ast::ExprKind::Cast { expr, .. }
        | ast::ExprKind::TypeCheck { expr, .. } => collect_local_decl_spans_in_expr(expr, out),
        ast::ExprKind::NamedArg { value, .. } => collect_local_decl_spans_in_expr(value, out),
        ast::ExprKind::Binary { lhs, rhs, .. } => {
            collect_local_decl_spans_in_expr(lhs, out);
            collect_local_decl_spans_in_expr(rhs, out);
        }
        ast::ExprKind::Assign { lhs, rhs, .. } => {
            collect_local_decl_spans_in_expr(lhs, out);
            collect_local_decl_spans_in_expr(rhs, out);
        }
        ast::ExprKind::WithUpdate { base, updates, .. } => {
            collect_local_decl_spans_in_expr(base, out);
            for update in updates {
                collect_local_decl_spans_in_expr(&update.value, out);
            }
        }
        ast::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let ast::InterpolatedStringPart::Expr { expr } = part {
                    collect_local_decl_spans_in_expr(expr, out);
                }
            }
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => {
            collect_local_decl_spans_in_block_into(body, out);
            for arm in arms {
                collect_local_decl_spans_in_expr(&arm.body, out);
            }
            if let Some(finally) = finally {
                collect_local_decl_spans_in_block_into(finally, out);
            }
        }
        ast::ExprKind::Missing
        | ast::ExprKind::Ident(_)
        | ast::ExprKind::IntLit
        | ast::ExprKind::FloatLit
        | ast::ExprKind::CharLit
        | ast::ExprKind::StringLit
        | ast::ExprKind::UnitLit
        | ast::ExprKind::ClassLit { .. } => {}
    }
}
