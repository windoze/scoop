//! 语句 lowering（TODO T0103d）。
//!
//! 说明：
//! - 该模块负责 AST → HIR 的语句部分 lowering；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::span::Span;
use crate::ty::TypeId;

use super::HirLowering;
use super::ValScope;
use super::types::ExpectedExpr;

use super::super::{
    AssignPlaceContract, AssignPlaceKind, Block, CallArg, ClassInitStep, Expr, ExprKind, File,
    FunDecl, GenericClassDecl, HandleExpr, Item, LiteralKind, MemberAccess, MemberRef, ObjectInit,
    ObjectInitStep, Stmt, StmtKind, ValDecl, ValueRef,
};

impl<'a> HirLowering<'a> {
    pub(super) fn lower_stmt_into_with_tail(
        &mut self,
        pkg_prefix: &str,
        s: &ast::Stmt,
        out: &mut Vec<Stmt>,
        expected: ExpectedExpr,
        is_tail: bool,
    ) -> bool {
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

        out.push(self.lower_stmt_single(pkg_prefix, s, expected));
        false
    }

    fn lower_stmt_single(
        &mut self,
        pkg_prefix: &str,
        s: &ast::Stmt,
        expected: ExpectedExpr,
    ) -> Stmt {
        let (kind, ty) = match &s.kind {
            ast::StmtKind::Empty => (StmtKind::Empty, self.builtins.unit),
            ast::StmtKind::Expr(e) => {
                // 赋值在 AST 中以表达式节点承载，但在 HIR 中视为语句（便于后续 MIR lowering）。
                if let ast::ExprKind::Assign { lhs, eq_span, rhs } = &e.kind {
                    if let Some(call) =
                        self.try_lower_computed_property_assign(pkg_prefix, e.span, lhs, rhs)
                    {
                        (StmtKind::Expr(call), self.builtins.unit)
                    } else if let Some(call) =
                        self.try_lower_delegated_property_assign(pkg_prefix, e.span, lhs, rhs)
                    {
                        (StmtKind::Expr(call), self.builtins.unit)
                    } else {
                        let lhs = self.lower_expr(pkg_prefix, lhs);
                        self.record_assign_place_contract(s.span, e.span, &lhs);
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
                let value = value
                    .as_ref()
                    .map(|e| self.lower_expr_with_expected(pkg_prefix, e, expected));
                (StmtKind::Return { value }, self.builtins.nothing)
            }
            ast::StmtKind::Missing => {
                self.record_stage_error(
                    s.span,
                    "parser recovery Missing statement cannot enter HIR lowering",
                    "HIR statement lowering",
                );
                (StmtKind::Empty, self.builtins.unit)
            }
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
        };

        Stmt {
            span: s.span,
            ty,
            kind,
        }
    }

    fn record_assign_place_contract(&mut self, stmt_span: Span, assign_span: Span, lhs: &Expr) {
        let Some(typechecked) = self.typechecked_assign_place_contract(assign_span) else {
            return;
        };

        let kind = match (&typechecked.kind, &lhs.kind) {
            (
                ast::AssignPlaceContractKind::Local { .. },
                ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            ) => AssignPlaceKind::Local {
                id: *id,
                name: name.clone(),
                decl_span: *decl_span,
            },
            (
                ast::AssignPlaceContractKind::TopLevel { .. },
                ExprKind::VarRef(ValueRef::TopLevel { id, fqn }),
            ) => AssignPlaceKind::TopLevel {
                id: *id,
                fqn: fqn.clone(),
            },
            (
                ast::AssignPlaceContractKind::Member {
                    owner_fqn,
                    member_fqn,
                    member_name,
                    receiver_ty,
                },
                ExprKind::MemberAccess { receiver, member },
            ) => AssignPlaceKind::Member {
                receiver_ty: receiver_ty.unwrap_or(receiver.ty),
                owner_fqn: owner_fqn.clone(),
                member_fqn: member_fqn.clone(),
                member_name: member_name.clone(),
                member_span: member.span,
                resolved: member.resolved.clone(),
            },
            _ => return,
        };

        self.assign_place_contracts.insert(
            self.call_site(stmt_span),
            AssignPlaceContract {
                span: stmt_span,
                kind,
                place_ty: typechecked.place_ty,
                value_ty: typechecked.value_ty,
                mutable: typechecked.mutable,
                write_barrier: typechecked.write_barrier,
                unsafe_required: typechecked.unsafe_required,
            },
        );
    }

    fn typechecked_assign_place_contract(
        &mut self,
        assign_span: Span,
    ) -> Option<ast::AssignPlaceContract> {
        let typecheck_types = self.typecheck_types?;
        let contract = self.file.assign_place_contract(assign_span)?;
        Some(self.re_intern_assign_place_contract(typecheck_types, contract))
    }

    fn re_intern_assign_place_contract(
        &mut self,
        typecheck_types: &crate::ty::TypeStore,
        contract: ast::AssignPlaceContract,
    ) -> ast::AssignPlaceContract {
        let place_ty = self
            .types
            .re_intern_from(typecheck_types, contract.place_ty);
        let value_ty = self
            .types
            .re_intern_from(typecheck_types, contract.value_ty);
        let write_barrier = match contract.write_barrier {
            ast::AssignWriteBarrierRequirement::NotRequired => {
                ast::AssignWriteBarrierRequirement::NotRequired
            }
            ast::AssignWriteBarrierRequirement::StorageSlot { slot_ty } => {
                let slot_ty = self.types.re_intern_from(typecheck_types, slot_ty);
                ast::AssignWriteBarrierRequirement::StorageSlot {
                    slot_ty: self.apply_active_type_param_bindings(slot_ty),
                }
            }
        };
        let kind = match contract.kind {
            ast::AssignPlaceContractKind::Member {
                owner_fqn,
                member_fqn,
                member_name,
                receiver_ty,
            } => ast::AssignPlaceContractKind::Member {
                owner_fqn,
                member_fqn,
                member_name,
                receiver_ty: receiver_ty
                    .map(|ty| self.types.re_intern_from(typecheck_types, ty))
                    .map(|ty| self.apply_active_type_param_bindings(ty)),
            },
            ast::AssignPlaceContractKind::Local { name, decl_span } => {
                ast::AssignPlaceContractKind::Local { name, decl_span }
            }
            ast::AssignPlaceContractKind::TopLevel { fqn } => {
                ast::AssignPlaceContractKind::TopLevel { fqn }
            }
        };

        ast::AssignPlaceContract {
            kind,
            place_ty: self.apply_active_type_param_bindings(place_ty),
            value_ty: self.apply_active_type_param_bindings(value_ty),
            mutable: contract.mutable,
            write_barrier,
            unsafe_required: contract.unsafe_required,
        }
    }

    pub(super) fn record_missing_assign_place_contracts_in_file(&mut self, file: &File) {
        for item in &file.items {
            match item {
                Item::Fun(fun) => {
                    if let Some(body) = &fun.body {
                        self.record_missing_assign_place_contracts_in_block(body);
                    }
                }
                Item::Val(val) => {
                    if let Some(init) = &val.init {
                        self.record_missing_assign_place_contracts_in_expr(init);
                    }
                }
                Item::Todo { .. } => {}
            }
        }
    }

    pub(super) fn record_missing_assign_place_contracts_in_funs(&mut self, funs: &[FunDecl]) {
        for fun in funs {
            if let Some(body) = &fun.body {
                self.record_missing_assign_place_contracts_in_block(body);
            }
        }
    }

    pub(super) fn record_missing_assign_place_contracts_in_object_inits(
        &mut self,
        inits: &std::collections::HashMap<String, ObjectInit>,
    ) {
        for init in inits.values() {
            for step in &init.steps {
                match step {
                    ObjectInitStep::PropertyInit { init, .. } => {
                        self.record_missing_assign_place_contracts_in_expr(init);
                    }
                    ObjectInitStep::InitBlock { block } => {
                        self.record_missing_assign_place_contracts_in_block(block);
                    }
                }
            }
        }
    }

    pub(super) fn record_missing_assign_place_contracts_in_class_inits(
        &mut self,
        inits: &std::collections::HashMap<String, GenericClassDecl>,
    ) {
        for init in inits.values() {
            for arg in &init.super_ctor_args {
                self.record_missing_assign_place_contracts_in_call_arg(arg);
            }
            for step in &init.steps {
                match step {
                    ClassInitStep::PropertyInit { init, .. } => {
                        self.record_missing_assign_place_contracts_in_expr(init);
                    }
                    ClassInitStep::InitBlock { block } => {
                        self.record_missing_assign_place_contracts_in_block(block);
                    }
                }
            }
            for ctor in &init.ctors {
                for param in &ctor.params {
                    if let Some(default_value) = &param.default_value {
                        self.record_missing_assign_place_contracts_in_expr(default_value);
                    }
                }
                if let Some(delegation) = &ctor.delegation {
                    for arg in &delegation.args {
                        self.record_missing_assign_place_contracts_in_call_arg(arg);
                    }
                }
                if let Some(body) = &ctor.body {
                    self.record_missing_assign_place_contracts_in_block(body);
                }
            }
        }
    }

    fn record_missing_assign_place_contracts_in_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.record_missing_assign_place_contracts_in_stmt(stmt);
        }
    }

    fn record_missing_assign_place_contracts_in_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Assign { lhs, rhs, .. } => {
                self.synthesize_assign_place_contract_if_missing(stmt.span, lhs, rhs);
                self.record_missing_assign_place_contracts_in_expr(lhs);
                self.record_missing_assign_place_contracts_in_expr(rhs);
            }
            StmtKind::Expr(expr) => self.record_missing_assign_place_contracts_in_expr(expr),
            StmtKind::Val(val) => {
                if let Some(init) = &val.init {
                    self.record_missing_assign_place_contracts_in_expr(init);
                }
            }
            StmtKind::While { cond, body } => {
                self.record_missing_assign_place_contracts_in_expr(cond);
                self.record_missing_assign_place_contracts_in_block(body);
            }
            StmtKind::Return { value } => {
                if let Some(value) = value {
                    self.record_missing_assign_place_contracts_in_expr(value);
                }
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }

    fn record_missing_assign_place_contracts_in_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.record_missing_assign_place_contracts_in_expr(&field.value);
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.record_missing_assign_place_contracts_in_expr(element);
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let super::super::InterpolatedStringPart::Expr { expr } = part {
                        self.record_missing_assign_place_contracts_in_expr(expr);
                    }
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. }
            | ExprKind::MemberAccess { receiver: expr, .. } => {
                self.record_missing_assign_place_contracts_in_expr(expr);
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.record_missing_assign_place_contracts_in_expr(lhs);
                self.record_missing_assign_place_contracts_in_expr(rhs);
            }
            ExprKind::Block(block) => self.record_missing_assign_place_contracts_in_block(block),
            ExprKind::Closure(closure) => {
                self.record_missing_assign_place_contracts_in_expr(&closure.body);
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.record_missing_assign_place_contracts_in_expr(cond);
                self.record_missing_assign_place_contracts_in_expr(then_branch);
                if let Some(else_branch) = else_branch {
                    self.record_missing_assign_place_contracts_in_expr(else_branch);
                }
            }
            ExprKind::When { subject, arms } => {
                self.record_missing_assign_place_contracts_in_expr(subject);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.record_missing_assign_place_contracts_in_expr(guard);
                    }
                    self.record_missing_assign_place_contracts_in_expr(&arm.body);
                }
            }
            ExprKind::Call { callee, args } => {
                self.record_missing_assign_place_contracts_in_expr(callee);
                for arg in args {
                    self.record_missing_assign_place_contracts_in_call_arg(arg);
                }
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    self.record_missing_assign_place_contracts_in_call_arg(arg);
                }
            }
            ExprKind::Handle(handle) => {
                self.record_missing_assign_place_contracts_in_handle(handle)
            }
            ExprKind::Missing
            | ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::ClassLiteral(_)
            | ExprKind::Todo(_) => {}
        }
    }

    fn record_missing_assign_place_contracts_in_handle(&mut self, handle: &HandleExpr) {
        self.record_missing_assign_place_contracts_in_block(&handle.body);
        for arm in &handle.arms {
            self.record_missing_assign_place_contracts_in_expr(&arm.body);
        }
        if let Some(finally) = &handle.finally {
            self.record_missing_assign_place_contracts_in_block(finally);
        }
    }

    fn record_missing_assign_place_contracts_in_call_arg(&mut self, arg: &CallArg) {
        match arg {
            CallArg::Positional(expr) => self.record_missing_assign_place_contracts_in_expr(expr),
            CallArg::Named { value, .. } => {
                self.record_missing_assign_place_contracts_in_expr(value)
            }
        }
    }

    fn synthesize_assign_place_contract_if_missing(&mut self, span: Span, lhs: &Expr, rhs: &Expr) {
        let site = self.call_site(span);
        if self.assign_place_contracts.contains_key(&site) {
            return;
        }
        let Some(kind) = self.synthetic_assign_place_kind(lhs) else {
            return;
        };
        let write_barrier = if matches!(kind, AssignPlaceKind::Local { .. }) {
            ast::AssignWriteBarrierRequirement::NotRequired
        } else {
            ast::AssignWriteBarrierRequirement::StorageSlot { slot_ty: lhs.ty }
        };
        self.assign_place_contracts.insert(
            site,
            AssignPlaceContract {
                span,
                kind,
                place_ty: lhs.ty,
                value_ty: rhs.ty,
                mutable: true,
                write_barrier,
                unsafe_required: false,
            },
        );
    }

    fn synthetic_assign_place_kind(&self, lhs: &Expr) -> Option<AssignPlaceKind> {
        match &lhs.kind {
            ExprKind::VarRef(ValueRef::Local {
                id,
                name,
                decl_span,
            }) => Some(AssignPlaceKind::Local {
                id: *id,
                name: name.clone(),
                decl_span: *decl_span,
            }),
            ExprKind::VarRef(ValueRef::TopLevel { id, fqn }) => Some(AssignPlaceKind::TopLevel {
                id: *id,
                fqn: fqn.clone(),
            }),
            ExprKind::MemberAccess { receiver, member } => {
                let (owner_fqn, member_fqn) = member_binding_from_member_access(member);
                Some(AssignPlaceKind::Member {
                    receiver_ty: receiver.ty,
                    owner_fqn,
                    member_fqn: member_fqn.unwrap_or_else(|| member.name.clone()),
                    member_name: member.name.clone(),
                    member_span: member.span,
                    resolved: member.resolved.clone(),
                })
            }
            _ => None,
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
            Some(ast::ForLoopIterableKind::IntProgression) => self.lower_for_scalar_progression(
                pkg_prefix,
                stmt_span,
                f,
                Self::INT_PROGRESSION_FQN,
                self.builtins.int,
            ),
            Some(ast::ForLoopIterableKind::LongProgression) => {
                let elem_ty = self.types.ty_int_n(64);
                self.lower_for_scalar_progression(
                    pkg_prefix,
                    stmt_span,
                    f,
                    Self::LONG_PROGRESSION_FQN,
                    elem_ty,
                )
            }
            Some(ast::ForLoopIterableKind::UIntProgression) => self.lower_for_scalar_progression(
                pkg_prefix,
                stmt_span,
                f,
                Self::UINT_PROGRESSION_FQN,
                self.builtins.uint,
            ),
            Some(ast::ForLoopIterableKind::ULongProgression) => {
                let elem_ty = self.types.ty_uint_n(64);
                self.lower_for_scalar_progression(
                    pkg_prefix,
                    stmt_span,
                    f,
                    Self::ULONG_PROGRESSION_FQN,
                    elem_ty,
                )
            }
            Some(ast::ForLoopIterableKind::Custom) => {
                let Some(custom) = info.and_then(|info| info.custom.as_ref()) else {
                    self.record_stage_error(
                        stmt_span,
                        "custom for-loop missing typechecked iterator/next contract",
                        "HIR for-loop lowering",
                    );
                    return self.empty_unit_stmt(stmt_span);
                };
                self.lower_for_custom_iterator(pkg_prefix, stmt_span, f, custom)
            }
            _ => {
                self.record_stage_error(
                    stmt_span,
                    "for-loop missing typechecked iterator contract",
                    "HIR for-loop lowering",
                );
                self.empty_unit_stmt(stmt_span)
            }
        }
    }

    fn empty_unit_stmt(&self, span: Span) -> Stmt {
        Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Empty,
        }
    }

    /// `for (x in arr) { body }` — Array<Int> 降糖：
    ///
    /// ```text
    /// {
    ///     val __for_arr = arr
    ///     var __for_i = 0
    ///     while (__for_i < Array.size(__for_arr)) {
    ///         val x = Array.get(__for_arr, __for_i)
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

        // Array.size(__for_arr)
        let size_call = self.call_top_level_fun_with_synthetic_binding(
            for_span,
            "scoop.core.Array.size",
            vec![arr_ref(for_span)],
            int,
            Some("array_size_inline"),
        );

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

        // Array.get(__for_arr, __for_i)
        let get_call = self.call_top_level_fun_with_synthetic_binding(
            for_span,
            "scoop.core.Array.get",
            vec![arr_ref(for_span), idx_ref(for_span)],
            int,
            Some("array_get_inline"),
        );

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

    /// `for (x in prog) { body }` — scalar Progression 降糖：
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
    fn lower_for_scalar_progression(
        &mut self,
        pkg_prefix: &str,
        stmt_span: Span,
        f: &ast::ForStmt,
        progression_fqn: &str,
        elem_ty: TypeId,
    ) -> Stmt {
        let span = f.span;
        let for_span = f.for_span;

        let iter_lowered = self.lower_expr(pkg_prefix, &f.iter);

        let bool_ = self.builtins.bool_;
        let unit = self.builtins.unit;
        let prog_ty = self.intern_nominal(progression_fqn.to_string(), Vec::new(), None);

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

        // Helper: field access on __for_prog (field_ty: element type for first/last/step, Bool for increasing)
        let prog_field = |me: &mut Self, span: Span, field: &str, field_ty| -> Expr {
            let fqn = format!("{progression_fqn}.{field}");
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
        let first_access = prog_field(self, for_span, "first", elem_ty);
        let cur_decl = Stmt {
            span: cur_span,
            ty: unit,
            kind: StmtKind::Val(ValDecl {
                span: cur_span,
                id: Some(cur_id),
                name: Some(cur_name.clone()),
                mutable: true,
                ty: elem_ty,
                init: Some(first_access),
            }),
        };

        // Helper: __for_cur ref
        let cur_ref = |span: Span| Expr {
            span,
            ty: elem_ty,
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
            let last_access = prog_field(me, for_span, "last", elem_ty);
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
                    ty: elem_ty,
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
            let step_access = prog_field(me, for_span, "step", elem_ty);
            let step_expr = Expr {
                span: for_span,
                ty: elem_ty,
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

        let iterator_call_span = Span::new(for_span.start + 4, for_span.start + 5);
        let next_call_span = Span::new(for_span.start + 5, for_span.start + 6);

        // val __for_iter = Iterable.iterator(__for_iterable)
        let iterator_call = self.lower_synthetic_member_call_with_receiver_ty(
            iterator_call_span,
            iterable_ref(for_span),
            iterable_ty,
            &info.iterator_method_fqn,
            Vec::new(),
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
        let next_call = self.lower_synthetic_member_call_with_receiver_ty(
            next_call_span,
            iterator_ref(for_span),
            iterator_ty,
            &info.next_method_fqn,
            Vec::new(),
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

fn member_binding_from_member_access(member: &MemberAccess) -> (Option<String>, Option<String>) {
    let fqn = match member.resolved.as_ref() {
        Some(MemberRef::Value { fqn, .. })
        | Some(MemberRef::Fun { fqn, .. })
        | Some(MemberRef::ExtensionValue { fqn, .. })
        | Some(MemberRef::ExtensionFun { fqn, .. }) => fqn,
        None => return (None, None),
    };
    let owner = fqn
        .strip_suffix(&member.name)
        .and_then(|prefix| prefix.strip_suffix('.'))
        .filter(|owner| !owner.is_empty())
        .map(ToOwned::to_owned);
    (owner, Some(fqn.clone()))
}
