//! 语句 lowering（覆盖全部 `StmtKind`）。

use scoop2_base::Span;
use scoop2_syntax::ast::{self, Block, Stmt, StmtKind};

use crate::mir::lower::FnLowering;
use crate::mir::{Operand, Terminator, TerminatorKind, UnwindAction};

/// lower 一个 block：lower 所有语句；返回块值（尾表达式或 Unit）。
pub fn lower_block(builder: &mut FnLowering, block: &Block) -> Operand {
    let unit_ty = builder.types.unit();
    let mut last_val = Operand::Const(crate::mir::ConstValue::Unit);
    let n = block.stmts.len();
    for (i, stmt) in block.stmts.iter().enumerate() {
        match &stmt.kind {
            StmtKind::Empty => {}
            StmtKind::Expr(e) => {
                let v = super::expr::lower_expr(builder, e);
                last_val = if i == n - 1 && !block.last_trailing_semi {
                    v
                } else {
                    last_val
                };
            }
            StmtKind::Assign { target, value } => {
                let val = super::expr::lower_expr(builder, value);
                super::stmt::lower_assign(builder, target, val, value.span);
            }
            StmtKind::LocalVal(val) => {
                super::stmt::lower_local_val(builder, val);
            }
            StmtKind::Return { value } => {
                let v = value
                    .as_ref()
                    .map(|e| super::expr::lower_expr(builder, e))
                    .unwrap_or(Operand::Const(crate::mir::ConstValue::Unit));
                let unwind = builder.build_unwind();
                builder.terminate(
                    Terminator {
                        span: stmt.span,
                        kind: TerminatorKind::Return { value: Some(v) },
                        unwind,
                    },
                    builder.current_bb,
                );
                // return 之后的基本块不可达；开新块继续（后续语句落到死块）。
                let dead = builder.new_block();
                builder.current_bb = dead;
            }
            StmtKind::While { cond, body } => {
                super::stmt::lower_while(builder, cond, body, stmt.span);
            }
            StmtKind::For {
                binder,
                iter,
                body,
            } => {
                super::stmt::lower_for(builder, binder, iter, body, stmt.span);
            }
            StmtKind::Break => {
                if let Some(loop_ctx) = builder.loop_stack.last().copied() {
                    builder.goto(loop_ctx.break_target, stmt.span);
                    let dead = builder.new_block();
                    builder.current_bb = dead;
                } else {
                    builder.error(crate::diagnostics::BREAK_OUTSIDE_LOOP, stmt.span, "`break` 只能出现在循环体内");
                }
            }
            StmtKind::Continue => {
                if let Some(loop_ctx) = builder.loop_stack.last().copied() {
                    builder.goto(loop_ctx.continue_target, stmt.span);
                    let dead = builder.new_block();
                    builder.current_bb = dead;
                } else {
                    builder.error(crate::diagnostics::CONTINUE_OUTSIDE_LOOP, stmt.span, "`continue` 只能出现在循环体内");
                }
            }
        }
    }
    let _ = unit_ty;
    last_val
}

/// lower 赋值（`target = value`）。
pub fn lower_assign(
    builder: &mut FnLowering,
    target: &ast::AssignTarget,
    value: Operand,
    span: Span,
) {
    use ast::AssignTargetKind;
    let val_ty = operand_ty(builder, &value);
    match &target.kind {
        AssignTargetKind::Ident(ident) => {
            // 优先局部。
            if let Some(&lid) = builder.symbol_locals.get(&ident.symbol) {
                builder.assign(lid, crate::mir::Rvalue::Use(value), span);
                return;
            }
            // assign_places 侧表：TopLevelVar？
            if let Some(place) = builder
                .hir
                .assign_place(builder.file_id, target.id)
            {
                match place {
                    scoop2_hir::hir::ResolvedPlace::TopLevelVar { fqn, .. } => {
                        builder.push_stmt(crate::mir::Statement {
                            span,
                            kind: crate::mir::StatementKind::StoreTopLevelVar {
                                fqn: *fqn,
                                value,
                                value_ty: val_ty,
                            },
                        });
                        return;
                    }
                    scoop2_hir::hir::ResolvedPlace::MemberField {
                        receiver_ty,
                        owner_fqn,
                        member_name,
                        ..
                    } => {
                        // 裸 Ident 赋值解析为 this.field = value（成员函数体内）。
                        // this 是隐式接收者：用 receiver_ty 分配/复用 this local。
                        let this_lid = builder.this_local.unwrap_or_else(|| {
                            let lid = builder.alloc_temp(*receiver_ty, span);
                            builder.this_local = Some(lid);
                            lid
                        });
                        let name_str =
                            builder.hir.interner.resolve(*member_name).to_string();
                        builder.push_stmt(crate::mir::Statement {
                            span,
                            kind: crate::mir::StatementKind::StoreMember {
                                receiver: Operand::Local(this_lid),
                                member: builder.member_access_metadata(&name_str, *receiver_ty),
                                value,
                                value_ty: val_ty,
                                continuation_route:
                                    crate::mir::transport::StoredContinuationRoutePublication::None,
                            },
                        });
                        let _ = owner_fqn;
                        return;
                    }
                    _ => {}
                }
            }
            // assign_place 未记录：尽力而为，按局部名分配（避免笼统报错）。
            let lid = builder.alloc_named_mutable(
                builder.hir.interner.resolve(ident.symbol).to_string(),
                val_ty,
                span,
                false,
            );
            builder.symbol_locals.insert(ident.symbol, lid);
            builder.assign(lid, crate::mir::Rvalue::Use(value), span);
        }
        AssignTargetKind::Member { receiver, member } => {
            let recv = super::expr::lower_expr(builder, receiver);
            let recv_ty = operand_ty(builder, &recv);
            match member {
                ast::MemberName::Named(name) => {
                    let name_str = builder.hir.interner.resolve(name.symbol).to_string();
                    if let Some(place) = builder.hir.assign_place(builder.file_id, target.id)
                        && let scoop2_hir::hir::ResolvedPlace::MemberField { receiver_ty, .. } = place
                    {
                        builder.push_stmt(crate::mir::Statement {
                            span,
                            kind: crate::mir::StatementKind::StoreMember {
                                receiver: recv,
                                member: builder.member_access_metadata(&name_str, *receiver_ty),
                                value,
                                value_ty: val_ty,
                                continuation_route:
                                    crate::mir::transport::StoredContinuationRoutePublication::None,
                            },
                        });
                        return;
                    }
                    // assign_place 未记录 MemberField：尽力而为，用默认 owner 发射
                    // StoreMember（recv.member = value 的结构已足够；owner 用于后端布局提示）。
                    builder.push_stmt(crate::mir::Statement {
                        span,
                        kind: crate::mir::StatementKind::StoreMember {
                            receiver: recv,
                            member: builder.member_access_metadata(&name_str, recv_ty),
                            value,
                            value_ty: val_ty,
                            continuation_route:
                                crate::mir::transport::StoredContinuationRoutePublication::None,
                        },
                    });
                }
                ast::MemberName::TupleIndex { value: idx, .. } => {
                    builder.push_stmt(crate::mir::Statement {
                        span,
                        kind: crate::mir::StatementKind::StoreTupleIndex {
                            receiver: recv,
                            index: *idx,
                            value,
                            value_ty: val_ty,
                        },
                    });
                }
            }
        }
        AssignTargetKind::Index { receiver, indices } => {
            let recv = super::expr::lower_expr(builder, receiver);
            let recv_ty = operand_ty(builder, &recv);
            // operator set：lower 为 method call。
            let set_sym = builder.hir.interner.get("set");
            if let Some(set_sym) = set_sym {
                let set_name = builder.hir.interner.resolve(set_sym).to_string();
                let owner_str = "";
                let mut args: Vec<crate::mir::CallArg> = Vec::new();
                for idx in indices {
                    let iv = super::expr::lower_expr(builder, idx);
                    let iv_ty = operand_ty(builder, &iv);
                    args.push(crate::mir::CallArg {
                        name: None,
                        is_spread: false,
                        value: iv,
                        value_ty: iv_ty,
                    });
                }
                args.push(crate::mir::CallArg {
                    name: None,
                    is_spread: false,
                    value,
                    value_ty: val_ty,
                });
                let result_ty = builder.types.unit();
                let tmp = builder.alloc_temp(result_ty, span);
                let set_site_id = builder.next_site_id();
                let set_transport = builder.call_transport(result_ty);
                let set_dispatch = crate::mir::transport::DispatchMetadata {
                    owner_fqn: owner_str.to_string(),
                    member_name: set_name.clone(),
                    member_fqn: format!("{}.{}", owner_str, set_name),
                    member_decl_span: None,
                    receiver_ty: recv_ty,
                    stable_candidate_keys: Vec::new(),
                    stable_template_key: None,
                    generic_type_args: Vec::new(),
                    generic_eff_args: Vec::new(),
                };
                let set_kind = builder.make_dispatch_call_kind(
                    resolve_owner_fqn_from_operand(builder, &recv),
                    recv,
                    set_dispatch,
                );
                builder.assign(
                    tmp,
                    crate::mir::Rvalue::Call {
                        site_id: Some(set_site_id),
                        kind: set_kind,
                        args,
                        transport: set_transport,
                    },
                    span,
                );
            } else {
                builder.error(crate::diagnostics::PRELUDE_SYMBOL_MISSING, span, "prelude 必需符号未注册：operator set（检查 sysroot / prelude 加载）");
            }
        }
    }
}

/// lower 局部 val/var（含解构）。
pub fn lower_local_val(builder: &mut FnLowering, val: &ast::ValDecl) {
    use ast::ValBinding;
    let init_ty = val
        .init
        .as_ref()
        .map(|e| builder.expr_ty(e.id))
        .unwrap_or_else(|| builder.types.nothing());
    let init_operand = val
        .init
        .as_ref()
        .map(|e| super::expr::lower_expr(builder, e));
    match &val.binding {
        ValBinding::Name(name) => {
            let is_var = val.kind == ast::ValKind::Var;
            let lid = builder.alloc_named_mutable(
                builder.hir.interner.resolve(name.symbol).to_string(),
                init_ty,
                name.span,
                is_var,
            );
            builder.symbol_locals.insert(name.symbol, lid);
            if let Some(v) = init_operand {
                builder.assign(lid, crate::mir::Rvalue::Use(v), name.span);
            }
        }
        ValBinding::Pattern(p) => {
            let is_var = val.kind == ast::ValKind::Var;
            let src = match init_operand {
                Some(o) => o,
                None => return,
            };
            super::stmt::bind_pattern(builder, p, src, init_ty, is_var);
        }
    }
}

/// 按模式绑定（解构 `val (a, b) = src`）。
pub fn bind_pattern(
    builder: &mut FnLowering,
    pat: &ast::Pattern,
    src: Operand,
    src_ty: scoop2_hir::ty::TypeId,
    mutable: bool,
) {
    use ast::PatternKind;
    match &pat.kind {
        PatternKind::Wildcard => {}
        PatternKind::Bind(name) => {
            let lid = builder.alloc_named_mutable(
                builder.hir.interner.resolve(name.symbol).to_string(),
                src_ty,
                name.span,
                mutable,
            );
            builder.symbol_locals.insert(name.symbol, lid);
            builder.assign(lid, crate::mir::Rvalue::Use(src), name.span);
        }
        PatternKind::Tuple(els) => {
            for (i, sub) in els.iter().enumerate() {
                let elem_ty = tuple_elem_ty(builder, src_ty, i).unwrap_or_else(|| builder.types.nothing());
                let tmp = builder.alloc_temp(elem_ty, sub.span);
                builder.assign(
                    tmp,
                    crate::mir::Rvalue::TupleIndex {
                        receiver: src.clone(),
                        index: i as u128,
                        element_ty: elem_ty,
                    },
                    sub.span,
                );
                super::stmt::bind_pattern(builder, sub, Operand::Local(tmp), elem_ty, mutable);
            }
        }
        PatternKind::Rest => {}
        PatternKind::Variant { path, args } => {
            // variant 解构 `val Result.Ok(v) = r`：spec §3420 允许。
            // 从 pattern_bindings 侧表取绑定的字段类型；variant 的 payload 通常是单字段，
            // subject 即 payload（单字段）或 tuple（多字段）。
            let binders: Vec<(scoop2_base::Symbol, scoop2_hir::ty::TypeId, scoop2_base::Span)> =
                if let Some(bs) = builder.hir.pattern_bindings(builder.file_id, pat.id) {
                    bs.iter()
                        .map(|b| (b.name, b.ty, b.span))
                        .collect()
                } else if let Some(args) = args {
                    // 回退：pattern_bindings 未记录时，按位置用 src_ty。
                    args.iter()
                        .filter_map(|a| match &a.kind {
                            PatternKind::Bind(n) => Some((n.symbol, src_ty, n.span)),
                            _ => None,
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            let _ = path;
            for (i, (bname, bty, bspan)) in binders.iter().enumerate() {
                let lid = builder.alloc_named_mutable(
                    builder.hir.interner.resolve(*bname).to_string(),
                    *bty,
                    *bspan,
                    mutable,
                );
                builder.symbol_locals.insert(*bname, lid);
                if binders.len() == 1 {
                    // 单字段 variant：subject 即 payload。
                    builder.assign(lid, crate::mir::Rvalue::Use(src.clone()), *bspan);
                } else {
                    // 多字段 variant：按 tuple-index 从 payload 提取。
                    builder.assign(
                        lid,
                        crate::mir::Rvalue::TupleIndex {
                            receiver: src.clone(),
                            index: i as u128,
                            element_ty: *bty,
                        },
                        *bspan,
                    );
                }
            }
        }
        PatternKind::Struct { fields, .. } => {
            // struct 解构 `val Point(x, y) = p` / `val Point { x, y } = p`。
            for f in fields {
                if let Some(p) = &f.pattern {
                    super::stmt::bind_pattern(builder, p, src.clone(), src_ty, mutable);
                    continue;
                }
                let bname = f.name.symbol;
                let fty = builder.types.any();
                let bname_str = builder.hir.interner.resolve(bname).to_string();
                let lid = builder.alloc_named_mutable(bname_str.clone(), fty, f.name.span, mutable);
                builder.symbol_locals.insert(bname, lid);
                let site_id = builder.next_site_id();
                let member = builder.member_access_metadata(&bname_str, src_ty);
                builder.assign(
                    lid,
                    crate::mir::Rvalue::MemberAccess {
                        site_id: Some(site_id),
                        receiver: src.clone(),
                        member,
                    },
                    f.name.span,
                );
            }
        }
        _ => {
            // literal / is / or 模式在 val 解构上下文不合法（refutable，应用 when）；
            // 到达此处表示 HIR 应已拒绝。按 bind 兜底（绑定 subject）。
            let lid = builder.alloc_temp(src_ty, pat.span);
            builder.assign(lid, crate::mir::Rvalue::Use(src), pat.span);
        }
    }
}

/// lower `while (cond) { body }`。
pub fn lower_while(builder: &mut FnLowering, cond: &ast::Expr, body: &Block, span: Span) {
    let cond_bb = builder.new_block();
    let body_bb = builder.new_block();
    let exit_bb = builder.new_block();
    builder.goto(cond_bb, span);
    // cond 块。
    builder.current_bb = cond_bb;
    let c = super::expr::lower_expr(builder, cond);
    builder.terminate(
        Terminator {
            span: cond.span,
            kind: TerminatorKind::CondBr {
                cond: c,
                then_target: body_bb,
                else_target: exit_bb,
            },
            unwind: UnwindAction::NoUnwind,
        },
        body_bb,
    );
    // body 块。
    builder.loop_stack.push(crate::mir::lower::builder::LoopContext { break_target: exit_bb, continue_target: body_bb });
    super::stmt::lower_block(builder, body);
    builder.loop_stack.pop();
    // body 末尾回到 cond。
    builder.goto(cond_bb, span);
    // 继续 exit。
    builder.current_bb = exit_bb;
}

/// lower `for (binder in iter) { body }`（desugar: iterator() + when + break，spec §16.2）。
pub fn lower_for(
    builder: &mut FnLowering,
    binder: &ast::Ident,
    iter: &ast::Expr,
    body: &Block,
    span: Span,
) {
    // 1. iter → Iterator：lower `iter.iterator()`（method call）。
    let iter_val = super::expr::lower_expr(builder, iter);
    let iter_ty = builder.expr_ty(iter.id);
    // iterator() 的返回类型：保留 Iterator<T> 的 T（从 for 表达式的类型查询元素类型）。
    // for (x in xs) 中 xs 的元素类型 = expr_types[xs] 的 nominal args[0]（若 xs 是 Array<T>）。
    // 精确保留元素类型，不退化为 Any。
    let iter_obj_ty = for_loop_element_type(builder, iter_ty);
    let iter_obj = builder.alloc_temp(iter_obj_ty, span);
    let it_method = builder.hir.interner.get("iterator");
    let it_args = Vec::new();
    let it_kind = if let Some(m) = it_method {
        let method_str = builder.hir.interner.resolve(m).to_string();
        let owner_str = "";
        let it_dispatch = crate::mir::transport::DispatchMetadata {
            owner_fqn: owner_str.to_string(),
            member_name: method_str.clone(),
            member_fqn: format!("{}.{}", owner_str, method_str),
            member_decl_span: None,
            receiver_ty: iter_ty,
            stable_candidate_keys: Vec::new(),
            stable_template_key: None,
            generic_type_args: Vec::new(),
            generic_eff_args: Vec::new(),
        };
        builder.make_dispatch_call_kind(resolve_owner_fqn_from_operand(builder, &iter_val), iter_val, it_dispatch)
    } else {
        builder.error(crate::diagnostics::PRELUDE_SYMBOL_MISSING, span, "prelude 必需符号未注册：iterator（检查 sysroot / prelude 加载）");
        return;
    };
    let it_site_id = builder.next_site_id();
    let it_transport = builder.call_transport(iter_obj_ty);
    builder.assign(
        iter_obj,
        crate::mir::Rvalue::Call {
            site_id: Some(it_site_id),
            kind: it_kind,
            args: it_args,
            transport: it_transport,
        },
        span,
    );
    // 2. loop: cond = iter.hasNext(); if !cond break; val e = iter.next(); body。
    let cond_bb = builder.new_block();
    let body_bb = builder.new_block();
    let exit_bb = builder.new_block();
    builder.goto(cond_bb, span);
    // cond。
    builder.current_bb = cond_bb;
    let has_next_sym = builder.hir.interner.get("hasNext");
    let next_sym = builder.hir.interner.get("next");
    let (has_next_sym, next_sym) = match (has_next_sym, next_sym) {
        (Some(h), Some(n)) => (h, n),
        _ => {
            builder.error(crate::diagnostics::PRELUDE_SYMBOL_MISSING, span, "prelude 必需符号未注册：hasNext / next（检查 sysroot / prelude 加载）");
            return;
        }
    };
    let bool_ty = builder.types.bool();
    let cond_tmp = builder.alloc_temp(bool_ty, span);
    let has_next_str = builder.hir.interner.resolve(has_next_sym).to_string();
    let has_next_owner = "";
    let iter_obj_ty_for_dispatch = operand_ty(builder, &Operand::Local(iter_obj));
    let has_next_site_id = builder.next_site_id();
    let has_next_transport = builder.call_transport(bool_ty);
    let has_next_dispatch = crate::mir::transport::DispatchMetadata {
        owner_fqn: has_next_owner.to_string(),
        member_name: has_next_str.clone(),
        member_fqn: format!("{}.{}", has_next_owner, has_next_str),
        member_decl_span: None,
        receiver_ty: iter_obj_ty_for_dispatch,
        stable_candidate_keys: Vec::new(),
        stable_template_key: None,
        generic_type_args: Vec::new(),
        generic_eff_args: Vec::new(),
    };
    let has_next_kind = builder.make_dispatch_call_kind(
        resolve_owner_fqn_from_operand(builder, &Operand::Local(iter_obj)),
        Operand::Local(iter_obj),
        has_next_dispatch,
    );
    builder.assign(
        cond_tmp,
        crate::mir::Rvalue::Call {
            site_id: Some(has_next_site_id),
            kind: has_next_kind,
            args: Vec::new(),
            transport: has_next_transport,
        },
        span,
    );
    builder.terminate(
        Terminator {
            span,
            kind: TerminatorKind::CondBr {
                cond: Operand::Local(cond_tmp),
                then_target: body_bb,
                else_target: exit_bb,
            },
            unwind: UnwindAction::NoUnwind,
        },
        body_bb,
    );
    // body: binder = iter.next(); <body>。
    builder.loop_stack.push(crate::mir::lower::builder::LoopContext { break_target: exit_bb, continue_target: body_bb });
    let elem_ty = iter_obj_ty; // for 循环 binder 类型 = iterator 元素类型（精确保留）。
    let elem = builder.alloc_named_mutable(
        builder.hir.interner.resolve(binder.symbol).to_string(),
        elem_ty,
        binder.span,
        false,
    );
    builder.symbol_locals.insert(binder.symbol, elem);
    let next_str = builder.hir.interner.resolve(next_sym).to_string();
    let next_owner = "";
    let iter_obj_ty_for_next = operand_ty(builder, &Operand::Local(iter_obj));
    let next_site_id = builder.next_site_id();
    let next_transport = builder.call_transport(elem_ty);
    let next_dispatch = crate::mir::transport::DispatchMetadata {
        owner_fqn: next_owner.to_string(),
        member_name: next_str.clone(),
        member_fqn: format!("{}.{}", next_owner, next_str),
        member_decl_span: None,
        receiver_ty: iter_obj_ty_for_next,
        stable_candidate_keys: Vec::new(),
        stable_template_key: None,
        generic_type_args: Vec::new(),
        generic_eff_args: Vec::new(),
    };
    let next_kind = builder.make_dispatch_call_kind(
        resolve_owner_fqn_from_operand(builder, &Operand::Local(iter_obj)),
        Operand::Local(iter_obj),
        next_dispatch,
    );
    builder.assign(
        elem,
        crate::mir::Rvalue::Call {
            site_id: Some(next_site_id),
            kind: next_kind,
            args: Vec::new(),
            transport: next_transport,
        },
        span,
    );
    super::stmt::lower_block(builder, body);
    builder.loop_stack.pop();
    builder.goto(cond_bb, span);
    builder.current_bb = exit_bb;
}

/// 取一个 operand 的类型（best-effort）。
pub fn operand_ty(builder: &mut FnLowering, op: &Operand) -> scoop2_hir::ty::TypeId {
    match op {
        Operand::Local(l) => builder
            .body
            .locals
            .get(l.0 as usize)
            .map(|d| d.ty)
            .unwrap_or_else(|| builder.types.nothing()),
        Operand::Const(c) => const_ty(builder, c),
    }
}

/// 从 operand 的类型解析 owner FQN Symbol（用于区分 interface vs class 分发）。
///
/// 取 operand 的类型 → 若是 `Ref(Nominal)` 或 `Value(Nominal)`，返回 nominal fqn。
/// 否则返回 `Symbol::default()`（无法解析时退回原行为）。
pub fn resolve_owner_fqn_from_operand(builder: &FnLowering, op: &Operand) -> scoop2_base::Symbol {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    let ty = match op {
        Operand::Local(l) => builder
            .body
            .locals
            .get(l.0 as usize)
            .map(|d| d.ty),
        Operand::Const(_) => return scoop2_base::Symbol::default(),
    };
    let Some(ty) = ty else {
        return scoop2_base::Symbol::default();
    };
    match builder.types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => n.fqn,
        TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn,
        _ => scoop2_base::Symbol::default(),
    }
}

fn const_ty(builder: &mut FnLowering, c: &crate::mir::ConstValue) -> scoop2_hir::ty::TypeId {
    match c {
        crate::mir::ConstValue::Bool(_) => builder.types.bool(),
        crate::mir::ConstValue::Char(_) => builder.types.char(),
        crate::mir::ConstValue::Unit => builder.types.unit(),
        crate::mir::ConstValue::Int(_, _) => builder.types.int(),
        crate::mir::ConstValue::Float(_, _) => builder.types.float64(),
        crate::mir::ConstValue::String(_) => builder.types.string(),
        crate::mir::ConstValue::Null => builder.types.any(),
    }
}

/// for 循环元素类型：从 iterable 类型提取元素 T。
/// - `Array<T>` (nominal with args[0]=T) → T;
/// - `Iterator<T>` (nominal with args[0]=T) → T;
/// - 其余 nominal → 尝试 args[0]；失败 → Any。
fn for_loop_element_type(
    builder: &mut FnLowering,
    iterable_ty: scoop2_hir::ty::TypeId,
) -> scoop2_hir::ty::TypeId {
    use scoop2_hir::ty::{TypeKind, ValueTypeKind, RefTypeKind};
    match builder.types.kind(iterable_ty) {
        // Array<T> / Iterator<T> as reference nominal.
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            if let Some(&elem_ty) = n.args.first() {
                return elem_ty;
            }
            builder.types.any()
        }
        // value nominal (struct Array<T>).
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            if let Some(&elem_ty) = n.args.first() {
                return elem_ty;
            }
            builder.types.any()
        }
        _ => builder.types.any(),
    }
}

/// tuple 元素类型查询。
fn tuple_elem_ty(
    builder: &FnLowering,
    tuple_ty: scoop2_hir::ty::TypeId,
    index: usize,
) -> Option<scoop2_hir::ty::TypeId> {
    use scoop2_hir::ty::{TypeKind, ValueTypeKind};
    match builder.types.kind(tuple_ty) {
        TypeKind::Value(ValueTypeKind::Tuple(elems)) => elems.get(index).copied(),
        _ => None,
    }
}

// 兼容：旧代码引用 lower_block 的别名。
pub use lower_block as lower_stmt_block;
