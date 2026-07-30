//! 语句 lowering（覆盖全部 `StmtKind`）。

use scoop2_base::Span;
use scoop2_syntax::ast::{self, Block, Stmt, StmtKind};

use crate::mir::lower::FnLowering;
use crate::mir::{Operand, Terminator, TerminatorKind};

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
                builder.terminate(
                    Terminator {
                        span: stmt.span,
                        kind: TerminatorKind::Return { value: Some(v) },
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
            // For 已在 typecheck 阶段 desugar 为 do{var __it; while{when next}}，
            // 不再到达 MIR（此处不会触发；若触发说明 desugar 未运行，报错）。
            StmtKind::For { .. } => {
                builder.error(
                    crate::diagnostics::LOWER_UNRESOLVED,
                    stmt.span,
                    "for-loop 应在 typecheck 阶段 desugar，不应到达 MIR",
                );
            }
            StmtKind::Break => {
                if let Some(loop_ctx) = builder.loop_stack.last().copied() {
                    builder.goto(loop_ctx.break_target, stmt.span);
                    let dead = builder.new_block();
                    builder.current_bb = dead;
                } else {
                    builder.error(
                        crate::diagnostics::BREAK_OUTSIDE_LOOP,
                        stmt.span,
                        "`break` 只能出现在循环体内",
                    );
                }
            }
            StmtKind::Continue => {
                if let Some(loop_ctx) = builder.loop_stack.last().copied() {
                    builder.goto(loop_ctx.continue_target, stmt.span);
                    let dead = builder.new_block();
                    builder.current_bb = dead;
                } else {
                    builder.error(
                        crate::diagnostics::CONTINUE_OUTSIDE_LOOP,
                        stmt.span,
                        "`continue` 只能出现在循环体内",
                    );
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
            if let Some(place) = builder.hir.assign_place(builder.file_id, target.id) {
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
                        let name_str = builder.hir.interner.resolve(*member_name).to_string();
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
                        && let scoop2_hir::hir::ResolvedPlace::MemberField { receiver_ty, .. } =
                            place
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
                builder.error(
                    crate::diagnostics::PRELUDE_SYMBOL_MISSING,
                    span,
                    "prelude 必需符号未注册：operator set（检查 sysroot / prelude 加载）",
                );
            }
        }
    }
}

/// lower 局部 val/var（含解构）。
pub fn lower_local_val(builder: &mut FnLowering, val: &ast::ValDecl) {
    use ast::ValBinding;
    // 空数组字面量 `[]` 在 typecheck 中得到 Nothing 类型（让 check_assignable 通过）。
    // 但 val 若有显式类型注解（如 `val x: Array<Int> = []`），local 应取注解类型，
    // 否则 codegen 会把数组 local 当 Nothing 处理。这里检测空数组 + 注解类型，
    // 用注解类型作为 local 类型。
    let init_is_empty_array = val
        .init
        .as_ref()
        .is_some_and(|e| matches!(&e.kind, ast::ExprKind::ArrayLit(els) if els.is_empty()));
    let declared_ty = val
        .ty
        .as_ref()
        .map(|t| {
            builder
                .hir
                .type_ref_resolution(builder.file_id, t.id)
                .unwrap_or_else(|| builder.types.unit())
        });
    let init_ty_raw = val
        .init
        .as_ref()
        .map(|e| builder.expr_ty(e.id))
        .unwrap_or_else(|| builder.types.unit());
    // `val ys: MutableArray<T> = [a, b]`：typecheck 允许 Array 字面量赋给
    // MutableArray 声明（上下文相关转换），但字面量标注类型仍是 Array<T>。
    // 这里用声明类型作为字面量结果类型，使 MakeArray 不 freeze（构造可变数组），
    // local 类型也与后续 `ys.set(...)` 的 MutableArray 布局分派一致。
    let init_is_array_lit = val
        .init
        .as_ref()
        .is_some_and(|e| matches!(&e.kind, ast::ExprKind::ArrayLit(_)));
    let declared_is_mutable_array = declared_ty.is_some_and(|t| {
        let fqn = match builder.types.kind(t) {
            scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => {
                builder.hir.interner.resolve(n.fqn)
            }
            _ => "",
        };
        fqn.ends_with(".MutableArray")
    });
    let use_declared_for_lit = declared_ty.is_some()
        && (init_is_empty_array || (init_is_array_lit && declared_is_mutable_array));
    let init_ty = if use_declared_for_lit {
        declared_ty.unwrap_or(init_ty_raw)
    } else {
        init_ty_raw
    };
    let init_operand = match &val.init {
        Some(e) if init_is_array_lit && declared_is_mutable_array => {
            let els = match &e.kind {
                ast::ExprKind::ArrayLit(els) => els,
                _ => unreachable!("init_is_array_lit 已判定为 ArrayLit"),
            };
            Some(super::expr::lower_array_lit(builder, els, e.span, init_ty))
        }
        Some(e) => Some(super::expr::lower_expr(builder, e)),
        None => None,
    };
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
                let elem_ty =
                    tuple_elem_ty(builder, src_ty, i).unwrap_or_else(|| builder.types.unit());
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
            // variant 解构 `val Result.Ok(v) = r` / `val Some(x) = opt`（spec §3420 允许）。
            // 与 when arm 的 bind_pattern_arm 同源：binder 按其在 args 中的字段位置
            // 经 PatternExtract（VariantField path）从 payload 提取——不能绑定整个
            // subject（单字段 variant 的 payload 也只是其中一个 slot）。
            let variant_name = path
                .segments
                .last()
                .map(|s| builder.hir.interner.resolve(s.symbol).to_string())
                .unwrap_or_default();
            if let Some(args) = args {
                for (i, a) in args.iter().enumerate() {
                    match &a.kind {
                        PatternKind::Bind(n) => {
                            // 字段类型：优先 pattern_bindings 侧表（typecheck 按字段
                            // 声明类型记录），缺失时按字段位置从 HIR 登记表推。
                            let bty = builder
                                .hir
                                .pattern_bindings(builder.file_id, pat.id)
                                .and_then(|bs| {
                                    bs.iter().find(|b| b.name == n.symbol).map(|b| b.ty)
                                })
                                .or_else(|| {
                                    super::expr::variant_payload_field_ty(
                                        builder, src_ty, path, i,
                                    )
                                })
                                .unwrap_or(src_ty);
                            let lid = builder.alloc_named_mutable(
                                builder.hir.interner.resolve(n.symbol).to_string(),
                                bty,
                                n.span,
                                mutable,
                            );
                            builder.symbol_locals.insert(n.symbol, lid);
                            builder.assign(
                                lid,
                                crate::mir::Rvalue::PatternExtract {
                                    subject: src.clone(),
                                    path: vec![
                                        crate::mir::transport::PatternBindingStep::VariantField {
                                            variant: variant_name.clone(),
                                            field_index: i,
                                        },
                                    ],
                                    result_ty: bty,
                                },
                                n.span,
                            );
                        }
                        PatternKind::Variant { .. }
                        | PatternKind::Tuple(_)
                        | PatternKind::Struct { .. } => {
                            // 嵌套子模式：提取字段后递归绑定。
                            if let Some(field_ty) =
                                super::expr::variant_payload_field_ty(builder, src_ty, path, i)
                            {
                                let tmp = builder.alloc_temp(field_ty, a.span);
                                builder.assign(
                                    tmp,
                                    crate::mir::Rvalue::PatternExtract {
                                        subject: src.clone(),
                                        path: vec![
                                            crate::mir::transport::PatternBindingStep::VariantField {
                                                variant: variant_name.clone(),
                                                field_index: i,
                                            },
                                        ],
                                        result_ty: field_ty,
                                    },
                                    a.span,
                                );
                                super::stmt::bind_pattern(
                                    builder,
                                    a,
                                    Operand::Local(tmp),
                                    field_ty,
                                    mutable,
                                );
                            }
                        }
                        _ => {} // Wildcard/Rest 等不绑定
                    }
                }
            }
        }
        PatternKind::Struct { fields, .. } => {
            // struct 解构 `val Point(x, y) = p` / `val Point { x, y } = p`。
            // 绑定类型优先取 HIR pattern_bindings 侧表（typecheck 按字段声明类型
            // 记录）；缺失时回退 Any（旧行为）。绑定类型错误（如 Any）会让后续
            // 二元运算解析不到 owner（`plus` 等 callee 缺前缀）。
            let recorded: Vec<(scoop2_base::Symbol, scoop2_hir::ty::TypeId)> =
                if let Some(bs) = builder.hir.pattern_bindings(builder.file_id, pat.id) {
                    bs.iter().map(|b| (b.name, b.ty)).collect()
                } else {
                    Vec::new()
                };
            for f in fields {
                if let Some(p) = &f.pattern {
                    super::stmt::bind_pattern(builder, p, src.clone(), src_ty, mutable);
                    continue;
                }
                let bname = f.name.symbol;
                let fty = recorded
                    .iter()
                    .find(|(n, _)| *n == bname)
                    .map(|(_, t)| *t)
                    .unwrap_or_else(|| builder.types.any());
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
        },
        body_bb,
    );
    // body 块。
    builder
        .loop_stack
        .push(crate::mir::lower::builder::LoopContext {
            break_target: exit_bb,
            continue_target: body_bb,
        });
    super::stmt::lower_block(builder, body);
    builder.loop_stack.pop();
    // body 末尾回到 cond。
    builder.goto(cond_bb, span);
    // 继续 exit。
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
            .unwrap_or_else(|| builder.types.unit()),
        Operand::Const(c) => const_ty(builder, c),
    }
}

/// 类型 → 其方法分发的 owner FQN Symbol。
///
/// 覆盖内建类型（Bool/Char/Int*/UInt*/Float*/String）与 nominal class/struct/interface。
/// 无法静态确定 owner（Any/Function/Union/类型参数等）时返回 `Symbol::default()`。
/// 注意：`Symbol::default()` 可能 resolve 出无关字符串（它是真实 Symbol(0)），
/// 调用方不得把它当作有效 owner 使用。
pub fn owner_fqn_of_type(builder: &FnLowering, ty: scoop2_hir::ty::TypeId) -> scoop2_base::Symbol {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    match builder.types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => n.fqn,
        TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn,
        TypeKind::Ref(RefTypeKind::String) => builder
            .hir
            .interner
            .get("scoop.core.String")
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::Bool) => builder
            .hir
            .interner
            .get("scoop.core.Bool")
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::Char) => builder
            .hir
            .interner
            .get("scoop.core.Char")
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::Int) => builder
            .hir
            .interner
            .get("scoop.core.Int")
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::UInt) => builder
            .hir
            .interner
            .get("scoop.core.UInt")
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => builder
            .hir
            .interner
            .get(&format!("scoop.core.Int{bits}"))
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => builder
            .hir
            .interner
            .get(&format!("scoop.core.UInt{bits}"))
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::Float32) => builder
            .hir
            .interner
            .get("scoop.core.Float32")
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::Float64) => builder
            .hir
            .interner
            .get("scoop.core.Float64")
            .unwrap_or_default(),
        _ => scoop2_base::Symbol::default(),
    }
}

/// 从 operand 的类型解析 owner FQN Symbol（用于区分 interface vs class 分发）。
///
/// 取 operand 的类型 → `owner_fqn_of_type`（含内建类型与常量 operand）。
/// 无法解析时返回 `Symbol::default()`。
pub fn resolve_owner_fqn_from_operand(builder: &FnLowering, op: &Operand) -> scoop2_base::Symbol {
    let ty = match op {
        Operand::Local(l) => builder.body.locals.get(l.0 as usize).map(|d| d.ty),
        // 常量 operand：按常量种类映射到内建 owner 类型 FQN。
        Operand::Const(c) => {
            let fqn = match c {
                crate::mir::ConstValue::String(_) => Some("scoop.core.String"),
                crate::mir::ConstValue::Bool(_) => Some("scoop.core.Bool"),
                crate::mir::ConstValue::Char(_) => Some("scoop.core.Char"),
                crate::mir::ConstValue::Int(_, _) => Some("scoop.core.Int"),
                crate::mir::ConstValue::Float(_, None) => Some("scoop.core.Float64"),
                crate::mir::ConstValue::Float(_, Some(_)) => Some("scoop.core.Float32"),
                crate::mir::ConstValue::Unit | crate::mir::ConstValue::Null => None,
            };
            return fqn
                .and_then(|f| builder.hir.interner.get(f))
                .unwrap_or_default();
        }
    };
    let Some(ty) = ty else {
        return scoop2_base::Symbol::default();
    };
    owner_fqn_of_type(builder, ty)
}

fn const_ty(builder: &mut FnLowering, c: &crate::mir::ConstValue) -> scoop2_hir::ty::TypeId {
    match c {
        crate::mir::ConstValue::Bool(_) => builder.types.bool(),
        crate::mir::ConstValue::Char(_) => builder.types.char(),
        crate::mir::ConstValue::Unit => builder.types.unit(),
        crate::mir::ConstValue::Int(_, _) => builder.types.int(),
        crate::mir::ConstValue::Float(_, None) => builder.types.float64(),
        crate::mir::ConstValue::Float(_, Some(_)) => builder.types.float32(),
        crate::mir::ConstValue::String(_) => builder.types.string(),
        crate::mir::ConstValue::Null => builder.types.any(),
    }
}

/// for 循环元素类型：从 iterable 类型提取元素 T。
/// - `Array<T>` (nominal with args[0]=T) → T;
/// - `Iterator<T>` (nominal with args[0]=T) → T;
/// - 其余 nominal → 尝试 args[0]；失败 → Any。

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
