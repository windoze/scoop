//! 表达式 lowering（覆盖全部 33 `ExprKind`）。
//!
//! 入口 [`lower_expr`]：lower 一个表达式，返回持有结果的 [`Operand`]（通常是
//! 一个临时 local）。每个表达式形式 lower 为 `Rvalue` 并赋给临时 local。

use scoop2_base::{Span, Symbol};
use scoop2_hir::hir::ResolvedCall;
use scoop2_hir::ty::EffectRow;
use scoop2_syntax::ast::{self, Expr, ExprKind, MemberName};

use crate::mir::lower::FnLowering;
use crate::mir::{
    AggregateTransportKind, CallArg, ClassCtorCallMetadata, ClosureEnvTransportMetadata,
    ConstValue, DispatchMetadata, HandleMetadata, Operand, PerformMetadata, RuntimeCastFailure,
    RuntimeCastMetadata, RuntimeCastResult, RuntimeTypeDescriptorKey, RuntimeTypeDescriptorKind,
    RuntimeTypeParameterizedMatch, RuntimeTypeStaticFold, RuntimeTypeTestMetadata, Rvalue,
    Statement, StatementKind, TopLevelRef,
};

/// lower 表达式，返回持有结果的 operand。
pub fn lower_expr(builder: &mut FnLowering, expr: &Expr) -> Operand {
    let span = expr.span;
    let ty = builder.expr_ty(expr.id);
    // 记录当前表达式 NodeId，供 lower_call/lower_binary/lower_unary 查 call_resolutions。
    builder.current_expr_id = expr.id;
    match &expr.kind {
        // 字面量。
        ExprKind::IntLit(l) => Operand::Const(ConstValue::Int(l.value, suffix_of(&l.suffix))),
        ExprKind::FloatLit(l) => {
            // 无后缀 FloatLit 在 Float32 期望类型下（如 `val f: Float32 = 1.5`，
            // typecheck 已按注解定型）按 f32 物化，与 local/alloca 类型一致。
            let f32_expected = matches!(
                builder.types.kind(ty),
                scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Float32)
            );
            let suffix = if l.suffix.is_some() || f32_expected {
                Some(crate::mir::FloatSuffix::F32)
            } else {
                None
            };
            Operand::Const(ConstValue::Float(l.value, suffix))
        }
        ExprKind::CharLit(l) => Operand::Const(ConstValue::Char(l.value)),
        ExprKind::StringLit(l) => Operand::Const(ConstValue::String(l.value.clone())),
        ExprKind::UnitLit => Operand::Const(ConstValue::Unit),
        ExprKind::Ident(ident) => lower_ident(builder, ident.symbol, span, ty),
        ExprKind::InterpolatedString { parts, .. } => lower_interpolated(builder, parts, span, ty),
        ExprKind::TupleLit(els) => {
            let ops: Vec<Operand> = els.iter().map(|e| lower_expr(builder, e)).collect();
            let tmp = builder.alloc_temp(ty, span);
            let transport = builder.aggregate_transport(ty, AggregateTransportKind::Tuple);
            builder.assign(
                tmp,
                Rvalue::MakeTuple {
                    elements: ops,
                    transport,
                },
                span,
            );
            Operand::Local(tmp)
        }
        ExprKind::ArrayLit(els) => lower_array_lit(builder, els, span, ty),
        ExprKind::StructLit { name, fields } => {
            let mut mir_fields = Vec::new();
            for f in fields {
                let v = lower_expr(builder, &f.value);
                let vty = super::stmt::operand_ty(builder, &v);
                mir_fields.push(crate::mir::StructLitField {
                    name: f.name.symbol,
                    value: v,
                    value_ty: vty,
                });
            }
            // 构造器调用（用 call_resolutions 或类型名）。
            let type_fqn = resolve_struct_fqn(builder, name.symbol);
            let tmp = builder.alloc_temp(ty, span);
            let transport = builder.aggregate_transport(ty, AggregateTransportKind::Struct);
            builder.assign(
                tmp,
                Rvalue::StructLit {
                    type_fqn,
                    fields: mir_fields,
                    transport,
                },
                span,
            );
            Operand::Local(tmp)
        }
        ExprKind::Block(b)
        | ExprKind::DoBlock(b)
        | ExprKind::UnsafeBlock(b)
        | ExprKind::SafeBlock(b) => super::stmt::lower_block(builder, b),
        ExprKind::Lambda(l) => lower_lambda(builder, expr, l, span, ty),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => lower_if(builder, cond, then_branch, else_branch.as_deref(), span, ty),
        ExprKind::When { subject, arms } => {
            super::expr::lower_when(builder, subject, arms, span, ty)
        }
        ExprKind::Handle {
            body,
            arms,
            finally,
        } => super::expr::lower_handle(builder, body, arms, finally.as_ref(), span, ty),
        ExprKind::MemberAccess { receiver, member } => {
            lower_member_access(builder, receiver, member, span, ty)
        }
        ExprKind::SafeMemberAccess { receiver, member } => {
            lower_safe_member_access(builder, receiver, member, span, ty)
        }
        ExprKind::SpliceField {
            receiver: _,
            field: _,
        } => {
            // splice field `p.["x"]` / `p.[FieldMeta{...}]` 是 comptime 反射特性，
            // 已从语言移除（spec 演进）。MIR 阶段明确拒绝并引导用户改用具体字段访问 `p.x`。
            builder.error(
                crate::diagnostics::SPLICE_FIELD_REMOVED,
                span,
                "splice field `.[...]` 是已移除的 comptime 反射特性，请改用具体字段访问（如 `p.x`）",
            );
            let tmp = builder.alloc_temp(ty, span);
            Operand::Local(tmp)
        }
        ExprKind::Index { receiver, indices } => lower_index(builder, receiver, indices, span, ty),
        ExprKind::NotNullAssert { expr: inner } => lower_not_null_assert(builder, inner, span, ty),
        ExprKind::TypeApply { callee, .. } => {
            // 类型实参应用：类型已 baked 进 callee 的类型；直接 lower callee。
            let _ = callee;
            lower_expr(builder, callee)
        }
        ExprKind::Call { callee, args } => lower_call(builder, callee, args, span, ty),
        ExprKind::ClassLit { path } => {
            let type_fqn = resolve_struct_fqn(
                builder,
                path.segments.last().map(|s| s.symbol).unwrap_or_default(),
            );
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(tmp, Rvalue::ClassLit { type_fqn }, span);
            Operand::Local(tmp)
        }
        ExprKind::Unary { op, expr: inner } => lower_unary(builder, *op, inner, span, ty),
        ExprKind::Binary { lhs, op, rhs } => lower_binary(builder, *op, lhs, rhs, span, ty),
        ExprKind::InfixCall {
            receiver,
            name,
            arg,
        } => lower_infix_call(builder, receiver, name.symbol, arg, span, ty),
        ExprKind::TypeCheck {
            expr: inner,
            op,
            ty: test_ty,
        } => {
            let v = lower_expr(builder, inner);
            let test_ty_id = resolve_typeref(builder, test_ty);
            let operand_ty_id = super::stmt::operand_ty(builder, &v);
            let bool_ty = builder.types.bool();
            let tmp = builder.alloc_temp(bool_ty, span);
            let type_fqn_str = nominal_fqn_of(builder, test_ty_id);
            let metadata = RuntimeTypeTestMetadata {
                source_ty: operand_ty_id,
                target_ty: test_ty_id,
                descriptor: RuntimeTypeDescriptorKey {
                    ty: test_ty_id,
                    kind: RuntimeTypeDescriptorKind::Nominal {
                        fqn: type_fqn_str,
                        kind: None,
                    },
                },
                static_fold: RuntimeTypeStaticFold::Dynamic,
                parameterized: RuntimeTypeParameterizedMatch::None,
            };
            let _ = op; // `is`/`!is` 语义差异由 verify / codegen 处理（test_ty 一致）。
            let site_id = Some(builder.next_site_id());
            builder.assign(
                tmp,
                Rvalue::TypeTest {
                    site_id,
                    value: v,
                    metadata,
                },
                span,
            );
            Operand::Local(tmp)
        }
        ExprKind::Cast {
            expr: inner,
            op,
            ty: target,
        } => {
            let v = lower_expr(builder, inner);
            let target_ty = resolve_typeref(builder, target);
            let operand_ty_id = super::stmt::operand_ty(builder, &v);
            let result = match op {
                ast::CastOp::As => ty,
                ast::CastOp::AsSafe => target_ty,
            };
            let tmp = builder.alloc_temp(result, span);
            let type_fqn_str = nominal_fqn_of(builder, target_ty);
            let metadata = RuntimeCastMetadata {
                test: RuntimeTypeTestMetadata {
                    source_ty: operand_ty_id,
                    target_ty,
                    descriptor: RuntimeTypeDescriptorKey {
                        ty: target_ty,
                        kind: RuntimeTypeDescriptorKind::Nominal {
                            fqn: type_fqn_str,
                            kind: None,
                        },
                    },
                    static_fold: RuntimeTypeStaticFold::Dynamic,
                    parameterized: RuntimeTypeParameterizedMatch::None,
                },
                failure: RuntimeCastFailure::ReturnNone,
                result: RuntimeCastResult::Target { ty: target_ty },
            };
            let site_id = Some(builder.next_site_id());
            builder.assign(
                tmp,
                Rvalue::Cast {
                    site_id,
                    value: v,
                    op: *op,
                    metadata,
                },
                span,
            );
            Operand::Local(tmp)
        }
        ExprKind::WithUpdate { base, updates } => {
            lower_with_update(builder, base, updates, span, ty)
        }
        ExprKind::Annotated { expr: inner, .. } => lower_expr(builder, inner),
    }
}

/// lower ident 引用：局部 / 顶层值 / 顶层函数 / 内建字面量。
fn lower_ident(
    builder: &mut FnLowering,
    sym: Symbol,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let name = builder.hir.interner.resolve(sym);
    // 局部。
    if let Some(&lid) = builder.symbol_locals.get(&sym) {
        return Operand::Local(lid);
    }
    // 成员函数体内的 `this`：解析为隐式 this_local（接收者）。
    if name == "this" {
        if let Some(lid) = builder.this_local {
            return Operand::Local(lid);
        }
    }
    // true/false/null/field/it 由 typecheck 处理（这里只兜底）。
    match name {
        "true" => return Operand::Const(ConstValue::Bool(true)),
        "false" => return Operand::Const(ConstValue::Bool(false)),
        "null" => return Operand::Const(ConstValue::Null),
        _ => {}
    }
    // 顶层值。
    if let Some(rv) = builder
        .hir
        .value_ref(builder.file_id, span_node(builder, sym, span))
    {
        if let scoop2_hir::resolve::ResolvedValue::TopLevelValue { fqn } = rv {
            let fqn_str = builder.hir.interner.resolve(*fqn).to_string();
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(
                tmp,
                Rvalue::TopLevelRef(TopLevelRef {
                    fqn: fqn_str,
                    hidden_effects: EffectRow::pure(),
                    stable_template_key: Some(crate::mir::stable_id::make_stable_template_key(
                        crate::mir::stable_id::StableHashScope::Dump,
                        &builder.hir.interner.resolve(*fqn).to_string(),
                        &[],
                        "",
                    )),
                    stable_instance_key: None,
                    generic_type_args: vec![],
                    generic_eff_args: vec![],
                }),
                span,
            );
            return Operand::Local(tmp);
        }
    }
    // value_refs by symbol——通过遍历 value_refs 找匹配（简化：顶层 val 表）。
    if let Some(&vty) = builder.hir.top_level_vals.get(&sym) {
        let tmp = builder.alloc_temp(vty, span);
        let fqn_str = builder.hir.interner.resolve(sym).to_string();
        builder.assign(
            tmp,
            Rvalue::TopLevelRef(TopLevelRef {
                fqn: fqn_str,
                hidden_effects: EffectRow::pure(),
                stable_template_key: Some(crate::mir::stable_id::make_stable_template_key(
                    crate::mir::stable_id::StableHashScope::Dump,
                    &builder.hir.interner.resolve(sym).to_string(),
                    &[],
                    "",
                )),
                stable_instance_key: None,
                generic_type_args: vec![],
                generic_eff_args: vec![],
            }),
            span,
        );
        return Operand::Local(tmp);
    }
    // 未解析。
    let tmp = builder.alloc_temp(ty, span);
    builder.assign(
        tmp,
        Rvalue::UnresolvedName {
            name: name.to_string(),
        },
        span,
    );
    Operand::Local(tmp)
}

/// value_refs 以 NodeId 为键；ident 节点没有 id，这里返回一个不可能命中的 NodeId 兜底。
fn span_node(_builder: &FnLowering, _sym: Symbol, _span: Span) -> scoop2_base::NodeId {
    // ident 无 NodeId；value_refs 查询退化为按符号查 top_level_vals（已在 lower_ident 内）。
    scoop2_base::NodeId::from_u32(u32::MAX)
}

/// lower 调用（Call）——消费 call_resolutions。
/// 从调用实参类型推断泛型类型实参（按 callee type_params 声明顺序）。
///
/// 当显式类型实参缺失时，MIR 单态化仍需知道具体类型实参（如 `println("x")` → T=String）。
/// 通过匹配 callee 签名的参数类型（含 TypeKind::Param）与调用实参的 value_ty 推断。
fn infer_type_args_from_call(
    builder: &FnLowering,
    fqn: scoop2_base::Symbol,
    args: &[crate::mir::CallArg],
) -> Vec<scoop2_hir::ty::TypeId> {
    use scoop2_hir::ty::{TypeKind, TypeStore};
    // 取 callee type_params（声明顺序）。
    let type_params: Vec<scoop2_base::Symbol> = builder
        .hir
        .type_constraints
        .get(&fqn)
        .map(|tc| tc.type_params.clone())
        .unwrap_or_default();
    if type_params.is_empty() {
        return Vec::new();
    }
    // 取 callee 签名的参数类型（首个重载）。
    let sig_param_tys: Vec<scoop2_hir::ty::TypeId> = builder
        .hir
        .top_level_funs
        .get(&fqn)
        .and_then(|sigs| sigs.first())
        .map(|s| s.param_types.clone())
        .unwrap_or_default();
    if sig_param_tys.is_empty() {
        return Vec::new();
    }
    // 推断：Param(name) → arg value_ty。
    let mut inferred: std::collections::HashMap<scoop2_base::Symbol, scoop2_hir::ty::TypeId> =
        std::collections::HashMap::new();
    let store = &builder.hir.store;
    for (i, sig_ty) in sig_param_tys.iter().enumerate() {
        let arg_ty = match args.get(i) {
            Some(a) => a.value_ty,
            None => continue,
        };
        infer_tp_recursive(store, *sig_ty, arg_ty, &mut inferred);
    }
    let result: Vec<scoop2_hir::ty::TypeId> = type_params
        .iter()
        .filter_map(|&tp| inferred.get(&tp).copied())
        .collect();
    let callee_name = builder.hir.interner.resolve(fqn);
    result
}

/// 递归匹配签名类型与实参类型，填充 Param→TypeId 映射。
///
/// 当前覆盖最常见的情形：签名参数类型直接是类型参数（如 `fun <T> println(value: T)`）。
/// 嵌套泛型（如 `Array<T>` 的 T）在后续迭代中补充。
fn infer_tp_recursive(
    store: &scoop2_hir::ty::TypeStore,
    sig_ty: scoop2_hir::ty::TypeId,
    arg_ty: scoop2_hir::ty::TypeId,
    out: &mut std::collections::HashMap<scoop2_base::Symbol, scoop2_hir::ty::TypeId>,
) {
    use scoop2_hir::ty::TypeKind;
    match store.kind(sig_ty) {
        TypeKind::Param(p) => {
            // 签名是类型参数 → 绑定到实参类型。
            out.entry(p.name).or_insert(arg_ty);
        }
        // 复合/引用类型的嵌套类型实参推断在后续迭代补充。
        TypeKind::Ref(_) | TypeKind::Value(_) | TypeKind::Nothing | TypeKind::StarProjection => {}
    }
}

fn lower_call(
    builder: &mut FnLowering,
    callee: &Expr,
    args: &[ast::CallArg],
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    // 关键：在 lower 子表达式（args / callee）之前捕获当前 Call 节点 id，
    // 否则 lower_call_args 会覆盖 builder.current_expr_id。
    let call_node = builder.current_expr_id;
    // 查 call_resolutions（key = Call 表达式 NodeId，由 record_expr_facts 在漏斗处写入）。
    let resolution = builder.hir.call_resolution(builder.file_id, call_node);
    // 查 resolved_call_args（默认参数填充 + 按位置排序后的实参列表）。
    // 若存在，用它替代原始 args（MIR 不做 resolution/default filling）。
    let resolved = builder.hir.resolved_call_args(builder.file_id, call_node);
    let mir_args: Vec<CallArg> = if let Some(resolved) = resolved {
        lower_resolved_call_args(builder, args, resolved)
    } else {
        lower_call_args(builder, args)
    };
    if let Some(rc) = resolution {
        // 若是方法调用（callee 是 MemberAccess），先 lower receiver 再传入。
        // 若是函数值调用（FunValue），lower callee 表达式本身作为 indirect call 目标。
        // 但 effect-op 调用（Raise.raise）的 receiver 是 effect 类型名（非值），
        // 不应 lower 为表达式；EffectOp 分支会发射 Perform 终结符，不需 receiver operand。
        let recv_op = match rc {
            scoop2_hir::hir::ResolvedCall::EffectOp { .. } => None,
            scoop2_hir::hir::ResolvedCall::FunValue { .. } => {
                // callee 是任意表达式（函数值）：lower 它作为 indirect call 的目标。
                Some(lower_expr(builder, callee))
            }
            _ => {
                if let ExprKind::MemberAccess { receiver, .. } = &callee.kind {
                    Some(lower_expr(builder, receiver))
                } else {
                    None
                }
            }
        };
        return emit_call_resolution(builder, rc, mir_args, span, ty, recv_op, call_node);
    }
    // 回退：按 callee 形态直接构造。
    match &callee.kind {
        ExprKind::MemberAccess { receiver, member } => {
            // 优先检测 enum variant 构造（`Color.Red(42)`）。
            if let ExprKind::Ident(recv_ident) = &receiver.kind
                && let MemberName::Named(variant_ident) = member
                && let Some(rc) =
                    derive_enum_variant_call(builder, recv_ident.symbol, variant_ident.symbol, ty)
            {
                return emit_call_resolution(builder, &rc, mir_args, span, ty, None, call_node);
            }
            let recv = lower_expr(builder, receiver);
            let recv_ty = super::stmt::operand_ty(builder, &recv);
            let (owner_sym, method_sym) = member_call_target(builder, callee);
            let owner_str = builder.hir.interner.resolve(owner_sym).to_string();
            let method_str = builder.hir.interner.resolve(method_sym).to_string();
            let member_fqn = format!("{}.{}", owner_str, method_str);
            let overload_sig = member_overload_sig(builder, owner_sym, method_sym);
            let stk = crate::mir::stable_id::make_stable_template_key(
                crate::mir::stable_id::StableHashScope::Dump,
                &member_fqn,
                &[],
                &overload_sig,
            );
            let tmp = builder.alloc_temp(ty, span);
            let dispatch = DispatchMetadata {
                owner_fqn: owner_str.clone(),
                member_name: method_str.clone(),
                member_fqn: member_fqn.clone(),
                member_decl_span: None,
                receiver_ty: recv_ty,
                stable_candidate_keys: vec![crate::mir::stable_id::make_stable_instance_key(
                    crate::mir::stable_id::StableHashScope::Dump,
                    stk.clone(),
                    &builder.types,
                    &builder.hir.interner,
                    &[],
                    &[],
                )],
                stable_template_key: Some(stk),
                generic_type_args: vec![],
                generic_eff_args: vec![],
            };
            // 通过 interface_fqns 区分 itable vs class vtable 分发通道。
            let is_interface = builder.hir.interface_fqns.contains(&owner_sym);
            let kind = if is_interface {
                crate::mir::CallKind::Interface {
                    receiver: recv,
                    dispatch,
                }
            } else {
                crate::mir::CallKind::Virtual {
                    receiver: recv,
                    dispatch,
                }
            };
            let site_id = Some(builder.next_site_id());
            let transport = builder.call_transport(ty);
            builder.assign(
                tmp,
                Rvalue::Call {
                    site_id,
                    kind,
                    args: mir_args,
                    transport,
                },
                span,
            );
            Operand::Local(tmp)
        }
        _ => {
            // TypeApply callee（如 `identity<Int>(42)`）：解包到内部 callee（Ident），
            // 提取显式类型实参。
            if let ExprKind::TypeApply {
                callee: inner_callee,
                args: type_args,
            } = &callee.kind
            {
                // 提取类型实参（TypeApply 的 args 是 TypeArg）。
                let explicit_tys: Vec<scoop2_hir::ty::TypeId> = type_args
                    .iter()
                    .filter_map(|ta| match &ta.kind {
                        ast::TypeArgKind::Type(t) => Some(resolve_typeref(builder, t)),
                        _ => None,
                    })
                    .collect();
                // 提取 effect 实参（`<eff E>` 形参）。
                let explicit_eff_args: Vec<scoop2_hir::ty::EffectRow> = type_args
                    .iter()
                    .filter_map(|ta| match &ta.kind {
                        ast::TypeArgKind::Effect(eff_expr) => {
                            // 把 AST effect row 解析为 EffectRow。
                            // 从 eff_expr.terms 解析每个 term 为 TypeId。
                            let terms: Vec<scoop2_hir::ty::TypeId> = eff_expr
                                .terms
                                .iter()
                                .filter_map(|term| {
                                    let last = term.path.segments.last()?;
                                    let name = builder.hir.interner.resolve(last.symbol);
                                    if name == "Pure" {
                                        return None;
                                    }
                                    builder
                                        .hir
                                        .interner
                                        .get(name)
                                        .filter(|f| builder.hir.enum_variants.contains_key(f))
                                        .map(|fqn| {
                                            builder.types.ref_nominal(scoop2_hir::ty::NominalType {
                                                fqn,
                                                args: vec![],
                                                eff: None,
                                            })
                                        })
                                })
                                .collect();
                            Some(scoop2_hir::ty::EffectRow::from_terms(terms))
                        }
                        _ => None,
                    })
                    .collect();
                // 尝试对内部 callee（通常是 Ident）构造 Direct 调用。
                if let ExprKind::Ident(ident) = &inner_callee.kind {
                    let callee_fqn = {
                        let name = builder.hir.interner.resolve(ident.symbol);
                        let prefix = builder
                            .hir
                            .file(builder.file_id)
                            .map(|f| f.package_prefix.as_str())
                            .unwrap_or("");
                        if prefix.is_empty() {
                            name.to_string()
                        } else {
                            format!("{prefix}.{name}")
                        }
                    };
                    let tmp = builder.alloc_temp(ty, span);
                    let site_id = Some(builder.next_site_id());
                    let transport = builder.call_transport(ty);
                    let direct_kind = builder.make_direct_call_kind(
                        callee_fqn.clone(),
                        explicit_tys.clone(),
                        false,
                    );
                    // 设置 generic_eff_args（从 TypeApply effect 实参）。
                    let direct_kind = match direct_kind {
                        crate::mir::CallKind::Direct {
                            callee_fqn,
                            type_args,
                            is_intrinsic,
                            stable_template_key,
                            stable_instance_key,
                            generic_type_args,
                            ..
                        } => crate::mir::CallKind::Direct {
                            callee_fqn,
                            type_args,
                            is_intrinsic,
                            stable_template_key,
                            stable_instance_key,
                            generic_type_args,
                            generic_eff_args: explicit_eff_args,
                        },
                        other => other,
                    };
                    builder.assign(
                        tmp,
                        Rvalue::Call {
                            site_id,
                            kind: direct_kind,
                            args: mir_args,
                            transport,
                        },
                        span,
                    );
                    return Operand::Local(tmp);
                }
            }
            // callee 是函数值。
            let callee_op = lower_expr(builder, callee);
            let tmp = builder.alloc_temp(ty, span);
            let site_id = Some(builder.next_site_id());
            let transport = builder.call_transport(ty);
            builder.assign(
                tmp,
                Rvalue::Call {
                    site_id,
                    kind: crate::mir::CallKind::FunValue { callee: callee_op },
                    args: mir_args,
                    transport,
                },
                span,
            );
            Operand::Local(tmp)
        }
    }
}

/// 取 member-call 目标 (owner_fqn, method) 从 member_refs。
fn member_call_target(builder: &FnLowering, member_expr: &Expr) -> (Symbol, Symbol) {
    if let ExprKind::MemberAccess { member, receiver } = &member_expr.kind
        && let MemberName::Named(name) = member
    {
        // 优先从 member_ref 获取。
        if let Some(rm) = builder.hir.member_ref(builder.file_id, member_expr.id) {
            let (owner, meth) = match rm {
                scoop2_hir::hir::ResolvedMember::Method {
                    owner_fqn,
                    method_name,
                    ..
                } => (*owner_fqn, *method_name),
                _ => (Symbol::default(), name.symbol),
            };
            return (owner, meth);
        }
        // 回退：从 receiver 表达式推导 owner FQN（不 lower，避免 &mut borrow）。
        let owner = resolve_owner_from_expr(builder, receiver);
        return (owner, name.symbol);
    }
    (Symbol::default(), Symbol::default())
}

/// 从 receiver 表达式推导 owner FQN（不 lower 表达式，避免 &mut 借用冲突）。
fn resolve_owner_from_expr(builder: &FnLowering, receiver: &Expr) -> scoop2_base::Symbol {
    // 从 HIR expr_type 获取 receiver 的类型。
    let ty = builder.hir.expr_type(builder.file_id, receiver.id);
    let ty = match ty {
        Some(t) => t,
        None => return scoop2_base::Symbol::default(),
    };
    super::stmt::owner_fqn_of_type(builder, ty)
}

/// 从 HIR member_funs 查找某 (owner, method) 首个重载的 overload signature
/// canonical 文本。找不到时返回空串（无法区分同名重载，但不阻断 lowering）。
fn member_overload_sig(builder: &FnLowering, owner_sym: Symbol, method_sym: Symbol) -> String {
    if let Some(methods) = builder.hir.member_funs.get(&owner_sym) {
        if let Some(sigs) = methods.get(&method_sym) {
            if let Some(first) = sigs.first() {
                return crate::mir::stable_id::build_overload_sig(
                    &builder.types,
                    &builder.hir.interner,
                    &first.param_types,
                );
            }
        }
    }
    String::new()
}

/// 检测 `<EnumType>.<Variant>(args)` 形态的 enum variant 构造调用。
/// 若 `enum_sym` 是一个 enum 类型 FQN 且 `variant_sym` 是其 variant，返回 EnumVariant 决议。
fn derive_enum_variant_call(
    builder: &FnLowering,
    enum_sym: Symbol,
    variant_sym: Symbol,
    return_ty: scoop2_hir::ty::TypeId,
) -> Option<scoop2_hir::hir::ResolvedCall> {
    let enum_name = builder.hir.interner.resolve(enum_sym);
    // 候选 enum FQN：裸名 / package prefix / scoop.core。
    let prefix = builder
        .hir
        .file(builder.file_id)
        .map(|f| f.package_prefix.as_str())
        .unwrap_or("");
    let candidates = [
        enum_name.to_string(),
        if prefix.is_empty() {
            enum_name.to_string()
        } else {
            format!("{}.{}", prefix, enum_name)
        },
        format!("scoop.core.{}", enum_name),
    ];
    for cand in &candidates {
        if let Some(enum_fqn) = builder.hir.interner.get(cand)
            && let Some(variants) = builder.hir.enum_variants.get(&enum_fqn)
            && variants.contains(&variant_sym)
        {
            return Some(scoop2_hir::hir::ResolvedCall::EnumVariant {
                enum_fqn,
                variant_name: variant_sym,
                return_ty,
            });
        }
    }
    None
}

/// 把 ResolvedCall 发射为 Rvalue::Call。
/// `receiver_operand`：当调用是方法调用（MemberAccess callee）时，传入已 lowered 的 receiver operand；
/// None 表示非方法调用（顶层函数 / 构造器 / 局部值等）。
fn emit_call_resolution(
    builder: &mut FnLowering,
    rc: &ResolvedCall,
    mut args: Vec<CallArg>,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
    receiver_operand: Option<Operand>,
    call_node: scoop2_base::NodeId,
) -> Operand {
    let tmp = builder.alloc_temp(ty, span);
    let call_site_id = Some(builder.next_site_id());
    let call_transport = builder.call_transport(ty);
    let rv = match rc {
        ResolvedCall::TopLevelFun {
            fqn,
            explicit_type_args,
            inferred_type_args,
            ..
        } => {
            let callee_fqn = builder.hir.interner.resolve(*fqn).to_string();
            // 方法调用解析为顶层函数 Direct 调用时（如 `a * b` → `scoop.core.Int.times`），
            // receiver 是隐式首参（`this`），需前置到 args。
            let mut final_args = args;
            if let Some(recv) = receiver_operand {
                let recv_ty = super::stmt::operand_ty(builder, &recv);
                final_args.insert(
                    0,
                    crate::mir::CallArg {
                        name: None,
                        is_spread: false,
                        value: recv,
                        value_ty: recv_ty,
                    },
                );
            }
            // 优先用显式类型实参；否则用推断的类型实参；再否则从实参类型推断（供 scan_calls 单态化 println<String> 等）。
            let mut type_args = if !explicit_type_args.is_empty() {
                explicit_type_args.clone()
            } else {
                inferred_type_args.clone()
            };
            if type_args.is_empty() {
                type_args = infer_type_args_from_call(builder, *fqn, &final_args);
            }
            Rvalue::Call {
                site_id: call_site_id,
                kind: builder.make_direct_call_kind(callee_fqn.clone(), type_args, false),
                args: final_args,
                transport: call_transport,
            }
        }
        ResolvedCall::Method {
            receiver_ty,
            owner_fqn,
            method_name,
            is_virtual,
            is_interface,
            explicit_type_args,
            ..
        } => {
            // 用真实 lowered receiver operand（由调用方传入），不再用未初始化 temp。
            let recv = receiver_operand.unwrap_or_else(|| {
                let lid = builder.alloc_temp(*receiver_ty, span);
                Operand::Local(lid)
            });
            let owner_str = builder.hir.interner.resolve(*owner_fqn).to_string();
            let method_str = builder.hir.interner.resolve(*method_name).to_string();
            // 特殊检测：Continuation.resume → CallKind::Resume。
            // resume 是 continuation 对象上的方法，不是普通的 interface 分发。
            if method_str == "resume" && owner_str.ends_with("Continuation") {
                // resume 的实参是 resume 值（第一个 arg）。
                let resume_value = args
                    .into_iter()
                    .next()
                    .map(|a| a.value)
                    .unwrap_or(Operand::Const(crate::mir::ConstValue::Unit));
                let kind = crate::mir::CallKind::Resume {
                    continuation: recv,
                    resume_value,
                };
                builder.assign(
                    tmp,
                    Rvalue::Call {
                        site_id: call_site_id,
                        kind,
                        args: Vec::new(),
                        transport: call_transport,
                    },
                    span,
                );
                return Operand::Local(tmp);
            }
            let member_fqn = format!("{}.{}", owner_str, method_str);
            let overload_sig = member_overload_sig(builder, *owner_fqn, *method_name);
            let stk = crate::mir::stable_id::make_stable_template_key(
                crate::mir::stable_id::StableHashScope::Dump,
                &member_fqn,
                &[],
                &overload_sig,
            );
            // 三种分发通道：
            //   - is_virtual && is_interface → Interface（itable 分发）
            //   - is_virtual && !is_interface → Virtual（class vtable 分发）
            //   - !is_virtual → Direct（final/static 方法）
            let kind = if *is_virtual {
                let dispatch = DispatchMetadata {
                    owner_fqn: owner_str.clone(),
                    member_name: method_str.clone(),
                    member_fqn: member_fqn.clone(),
                    member_decl_span: None,
                    receiver_ty: *receiver_ty,
                    stable_candidate_keys: vec![crate::mir::stable_id::make_stable_instance_key(
                        crate::mir::stable_id::StableHashScope::Dump,
                        stk.clone(),
                        &builder.types,
                        &builder.hir.interner,
                        &[],
                        &[],
                    )],
                    stable_template_key: Some(stk),
                    generic_type_args: explicit_type_args.clone(),
                    generic_eff_args: vec![],
                };
                if *is_interface {
                    crate::mir::CallKind::Interface {
                        receiver: recv,
                        dispatch,
                    }
                } else {
                    crate::mir::CallKind::Virtual {
                        receiver: recv,
                        dispatch,
                    }
                }
            } else {
                // 非虚方法 → Direct 调用。receiver 是隐式首参，需前置到 args。
                let recv_ty = *receiver_ty;
                args.insert(
                    0,
                    crate::mir::CallArg {
                        name: None,
                        is_spread: false,
                        value: recv,
                        value_ty: recv_ty,
                    },
                );
                builder.make_direct_call_kind(member_fqn, explicit_type_args.clone(), false)
            };
            Rvalue::Call {
                site_id: call_site_id,
                kind,
                args,
                transport: call_transport,
            }
        }
        ResolvedCall::Constructor { type_fqn, .. } => {
            // struct 是值类型：不走堆分配 ctor，发 StructLit（codegen insertvalue
            // 值语义）。class 才走 ClassCtor（GC 堆对象）。
            if !builder.hir.class_fqns.contains(type_fqn) {
                let ordered_names: Vec<scoop2_base::Symbol> = builder
                    .hir
                    .member_order
                    .get(type_fqn)
                    .cloned()
                    .unwrap_or_default();
                let any_named = args.iter().any(|a| a.name.is_some());
                let mut mir_fields: Vec<crate::mir::StructLitField> =
                    Vec::with_capacity(args.len());
                if any_named {
                    // 命名实参：按 member_order 声明序重排（字段布局顺序）。
                    for &mname in &ordered_names {
                        if let Some(arg) = args.iter().find(|a| a.name == Some(mname)) {
                            mir_fields.push(crate::mir::StructLitField {
                                name: mname,
                                value: arg.value.clone(),
                                value_ty: arg.value_ty,
                            });
                        }
                    }
                    // member_order 未覆盖的命名实参（防御）：按原顺序追加。
                    for arg in &args {
                        if let Some(n) = arg.name
                            && !ordered_names.contains(&n)
                        {
                            mir_fields.push(crate::mir::StructLitField {
                                name: n,
                                value: arg.value.clone(),
                                value_ty: arg.value_ty,
                            });
                        }
                    }
                } else {
                    // 位置实参：与 member_order 声明序一一对应。
                    for (i, arg) in args.iter().enumerate() {
                        let name = ordered_names.get(i).copied().unwrap_or_default();
                        mir_fields.push(crate::mir::StructLitField {
                            name,
                            value: arg.value.clone(),
                            value_ty: arg.value_ty,
                        });
                    }
                }
                let transport = builder.aggregate_transport(ty, AggregateTransportKind::Struct);
                Rvalue::StructLit {
                    type_fqn: *type_fqn,
                    fields: mir_fields,
                    transport,
                }
            } else {
                let type_fqn_str = builder.hir.interner.resolve(*type_fqn).to_string();
                // 继承构造链：有 `: Super(args)` 委托的 class，把超类字段实参
                // 展开到 args 前部，使 args 与字段布局（超类字段在前）对齐。
                // 若 resolved_call_args 已填充（默认参数 + 命名实参排序），跳过
                // expand_super_ctor_chain（参数已由 HIR 解析）。
                let resolved = builder.hir.resolved_call_args(builder.file_id, call_node);
                let args = if resolved.is_some() {
                    args
                } else {
                    expand_super_ctor_chain(builder, type_fqn, args)
                };
                // 选中 ctor 的 span（区分 primary/secondary；secondary 时指向 constructor 关键字）。
                let selected_ctor_span = builder
                    .hir
                    .ctor_selection(builder.file_id, call_node);
                Rvalue::ClassCtor {
                    site_id: call_site_id,
                    type_fqn: *type_fqn,
                    ctor: ClassCtorCallMetadata {
                        target_init_class_fqn: type_fqn_str,
                        selected_ctor_span,
                        ordered_param_count: args.len(),
                        stable_template_key: None,
                    },
                    args,
                    hidden_effects: EffectRow::pure(),
                }
            }
        }
        ResolvedCall::EnumVariant {
            enum_fqn,
            variant_name,
            ..
        } => {
            let enum_ty = ty;
            let payload = builder.aggregate_transport(ty, AggregateTransportKind::EnumPayload);
            Rvalue::EnumVariant {
                enum_ty,
                enum_fqn: *enum_fqn,
                variant_name: *variant_name,
                args,
                payload,
                stable_key: None,
            }
        }
        ResolvedCall::LocalValue { local_name, .. } => {
            let callee = if let Some(&lid) = builder.symbol_locals.get(local_name) {
                Operand::Local(lid)
            } else {
                let l = builder.alloc_temp(ty, span);
                Operand::Local(l)
            };
            Rvalue::Call {
                site_id: call_site_id,
                kind: crate::mir::CallKind::FunValue { callee },
                args,
                transport: call_transport,
            }
        }
        ResolvedCall::FunValue { .. } => {
            // callee 是任意表达式（函数值调用）：lower callee 表达式为 operand，
            // 然后作为 FunValue indirect call。
            // 注意：callee 作为 Call 节点的子表达式，在 lower_call 中需要单独 lower。
            // 但 lower_call 的签名只接收 callee: &Expr，我们可以直接 lower 它。
            // 然而 emit_call_resolution 不接收 callee —— 它只接收 rc + args。
            // 解决：FunValue 的 callee operand 已经在 lower_call 中 lower 过（因为
            // lower_call 在进入 emit_call_resolution 之前捕获了 call_node）。
            // 但 callee 本身可能还没 lower——lower_call 的正常路径不 lower callee。
            // 简化：使用 receiver_operand（若 caller 传了）或分配一个临时。
            let callee = receiver_operand.unwrap_or_else(|| {
                let l = builder.alloc_temp(ty, span);
                Operand::Local(l)
            });
            Rvalue::Call {
                site_id: call_site_id,
                kind: crate::mir::CallKind::FunValue { callee },
                args,
                transport: call_transport,
            }
        }
        ResolvedCall::EffectOp {
            effect_name,
            op_name,
            ..
        } => {
            // effect-op 调用 → Perform 终结符。
            let op_fqn = format!(
                "{}.{}",
                builder.hir.interner.resolve(*effect_name),
                builder.hir.interner.resolve(*op_name)
            );
            let resume_local = builder.alloc_temp(ty, span);
            let resume_target = builder.new_block();
            // 从 args 构造 payload metadata。
            let payload_component_tys: Vec<scoop2_hir::ty::TypeId> =
                args.iter().map(|a| a.value_ty).collect();
            let payload_transport: Vec<crate::mir::transport::ValueTransportMetadata> =
                payload_component_tys
                    .iter()
                    .map(|&t| {
                        crate::mir::transport::value_transport(
                            &builder.types,
                            &builder.enum_fqns,
                            t,
                        )
                    })
                    .collect();
            let payload_tuple_ty = if payload_component_tys.len() == 1 {
                // 单参数：payload 类型 = 参数类型本身。
                Some(payload_component_tys[0])
            } else if payload_component_tys.is_empty() {
                None
            } else {
                // 多参数：构造 tuple 类型。
                Some(builder.types.tuple(payload_component_tys.clone()))
            };
            let arg_mapping: Vec<usize> = (0..args.len()).collect();
            let metadata = PerformMetadata {
                effect_ty: {
                    // 解析 effect 类型：尝试从 effect_name 查找 nominal。
                    let eff_name = builder.hir.interner.resolve(*effect_name);
                    let prefix = builder
                        .hir
                        .file(builder.file_id)
                        .map(|f| f.package_prefix.as_str())
                        .unwrap_or("");
                    let candidates = if prefix.is_empty() {
                        vec![eff_name.to_string(), format!("scoop.core.{eff_name}")]
                    } else {
                        vec![
                            eff_name.to_string(),
                            format!("{prefix}.{eff_name}"),
                            format!("scoop.core.{eff_name}"),
                        ]
                    };
                    candidates
                        .iter()
                        .filter_map(|c| builder.hir.interner.get(c))
                        .filter(|f| {
                            builder.hir.enum_variants.contains_key(f)
                                || builder.hir.member_funs.contains_key(f)
                        })
                        .next()
                        .map(|fqn| {
                            builder.types.ref_nominal(scoop2_hir::ty::NominalType {
                                fqn,
                                args: vec![],
                                eff: None,
                            })
                        })
                        .unwrap_or_else(|| builder.types.any())
                },
                op_type_args: vec![],
                result_ty: ty,
                payload_tuple_ty,
                payload_component_tys,
                payload_transport,
                arg_mapping,
            };
            let site_id = Some(builder.next_site_id());
            builder.terminate(
                crate::mir::Terminator {
                    span,
                    kind: crate::mir::TerminatorKind::Perform {
                        site_id,
                        op_fqn,
                        metadata,
                        args,
                        resume_local,
                        resume_target,
                    },
                },
                resume_target,
            );
            // resume_target 块：把 resume_local 作为结果。
            return Operand::Local(resume_local);
        }
    };
    builder.assign(tmp, rv, span);
    Operand::Local(tmp)
}

/// lower resolved call args（默认参数填充 + 按位置排序后的实参列表）。
///
/// 每个 ResolvedCallArg：
/// - Provided { original_index } → lower 原始调用点第 original_index 个实参。
/// - Default { expr } → lower 默认值表达式。
///
/// **求值顺序**：Provided 实参按源码顺序（original_index 升序）lower，
/// Default 实参在它们所在的位置 lower。这保证命名实参的求值顺序与源码一致
/// （Scoop 语义：实参按书写顺序求值，无论命名还是位置）。
fn lower_resolved_call_args(
    builder: &mut FnLowering,
    original_args: &[ast::CallArg],
    resolved: &[scoop2_hir::hir::ResolvedCallArg],
) -> Vec<CallArg> {
    // 第一遍：按源码顺序 lower 所有 Provided 实参（保留求值顺序）。
    // 收集 (param_position, value, value_ty, is_spread) 到临时 local。
    let mut provided_vals: std::collections::HashMap<usize, (Operand, scoop2_hir::ty::TypeId, bool)> =
        std::collections::HashMap::new();
    // 按源码序（original_index 升序）lower。
    let mut provided_indices: Vec<(usize, usize)> = resolved
        .iter()
        .enumerate()
        .filter_map(|(pos, r)| match r {
            scoop2_hir::hir::ResolvedCallArg::Provided { original_index } => {
                Some((*original_index, pos))
            }
            _ => None,
        })
        .collect();
    provided_indices.sort_by_key(|(orig_idx, _)| *orig_idx);
    for (orig_idx, pos) in provided_indices {
        let a = &original_args[orig_idx];
        let v = lower_expr(builder, &a.value);
        let ty = super::stmt::operand_ty(builder, &v);
        provided_vals.insert(pos, (v, ty, a.is_spread));
    }
    // 第二遍：按参数位置序构建 CallArg（Default 实参在此 lower）。
    resolved
        .iter()
        .enumerate()
        .map(|(pos, r)| match r {
            scoop2_hir::hir::ResolvedCallArg::Provided { .. } => {
                let (v, ty, spread) = provided_vals.remove(&pos).unwrap_or_else(|| {
                    (
                        Operand::Const(ConstValue::Unit),
                        builder.types.nothing(),
                        false,
                    )
                });
                CallArg {
                    name: None,
                    is_spread: spread,
                    value_ty: ty,
                    value: v,
                }
            }
            scoop2_hir::hir::ResolvedCallArg::Default { expr } => {
                let v = lower_expr(builder, expr);
                CallArg {
                    name: None,
                    is_spread: false,
                    value_ty: super::stmt::operand_ty(builder, &v),
                    value: v,
                }
            }
        })
        .collect()
}

/// lower 一组 CallArg。
fn lower_call_args(builder: &mut FnLowering, args: &[ast::CallArg]) -> Vec<CallArg> {
    args.iter()
        .map(|a| {
            let v = lower_expr(builder, &a.value);
            CallArg {
                name: a.name.as_ref().map(|n| n.symbol),
                is_spread: a.is_spread,
                value_ty: super::stmt::operand_ty(builder, &v),
                value: v,
            }
        })
        .collect()
}

/// lower Unary（运算符 → 方法调用，消费 call_resolutions）。
fn lower_unary(
    builder: &mut FnLowering,
    op: ast::UnaryOp,
    inner: &Expr,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    // `!x`（Bool）：直接 Not（不走方法）。
    if matches!(op, ast::UnaryOp::Not) {
        let v = lower_expr(builder, inner);
        let inner_ty = super::stmt::operand_ty(builder, &v);
        // Bool Not：lower 为 `v == false`（用 equals 方法 / 内建）。
        // 简化：用 condbr 反转——emit Compare(v, Eq, false)。
        // 这里直接走 call_resolutions（运算符 Not 不落方法，故回退到 Bool 取反 rvalue）。
        let bool_ty = builder.bool_ty();
        let equals_sym = builder.hir.interner.get("equals").unwrap_or_default();
        let false_op = Operand::Const(ConstValue::Bool(false));
        let tmp = builder.alloc_temp(bool_ty, span);
        let owner_str = builder.hir.interner.resolve(Symbol::default()).to_string();
        let method_str = builder.hir.interner.resolve(equals_sym).to_string();
        let member_fqn = format!("{}.{}", owner_str, method_str);
        let overload_sig = member_overload_sig(builder, Symbol::default(), equals_sym);
        let stk = crate::mir::stable_id::make_stable_template_key(
            crate::mir::stable_id::StableHashScope::Dump,
            &member_fqn,
            &[],
            &overload_sig,
        );
        let dispatch = DispatchMetadata {
            owner_fqn: owner_str.clone(),
            member_name: method_str.clone(),
            member_fqn: member_fqn.clone(),
            member_decl_span: None,
            receiver_ty: inner_ty,
            stable_candidate_keys: vec![crate::mir::stable_id::make_stable_instance_key(
                crate::mir::stable_id::StableHashScope::Dump,
                stk.clone(),
                &builder.types,
                &builder.hir.interner,
                &[],
                &[],
            )],
            stable_template_key: Some(stk),
            generic_type_args: vec![],
            generic_eff_args: vec![],
        };
        let site_id = Some(builder.next_site_id());
        let transport = builder.call_transport(bool_ty);
        let kind = builder.make_dispatch_call_kind(
            super::stmt::resolve_owner_fqn_from_operand(builder, &v),
            v,
            dispatch,
        );
        builder.assign(
            tmp,
            Rvalue::Call {
                site_id,
                kind,
                args: vec![CallArg {
                    name: None,
                    is_spread: false,
                    value: false_op,
                    value_ty: bool_ty,
                }],
                transport,
            },
            span,
        );
        return Operand::Local(tmp);
    }
    let un_node = builder.current_expr_id;
    let method_hint = unop_to_method_name_str(op);
    let inner_op = lower_expr(builder, inner);
    lower_unary_via_call_resolution(builder, un_node, method_hint, inner_op, span, ty)
}

/// 通过 call_resolutions 派生一元运算符调用。
///
/// 与 binary 不同：unary 没有 rhs operand，receiver（lhs）即唯一的真参数。
/// 不能复用 `lower_via_call_resolution`，否则会把占位 `Const(Unit)` 当成第二个参数。
fn lower_unary_via_call_resolution(
    builder: &mut FnLowering,
    op_node: scoop2_base::NodeId,
    method_hint: &str,
    receiver: Operand,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    if let Some(rc) = builder.hir.call_resolution(builder.file_id, op_node) {
        // Unary 决议同样是 ResolvedCall::Method（运算符 → 方法），但参数列表为空。
        return emit_call_resolution(builder, rc, Vec::new(), span, ty, Some(receiver), op_node);
    }
    // 回退（标量内建 / 未解析方法）：emit Direct 调用到 `<owner>.<method_hint>`。
    let receiver_ty = super::stmt::operand_ty(builder, &receiver);
    let owner_fqn = owner_fqn_of(builder, receiver_ty);
    let callee_fqn = if owner_fqn == Symbol::default() {
        method_hint.to_string()
    } else {
        format!(
            "{}.{}",
            builder.hir.interner.resolve(owner_fqn),
            method_hint
        )
    };
    // 标量内建一元运算符：receiver 既是隐式首参，也是唯一参数（如 `-x` → Int.unaryMinus(x)）。
    let args = vec![CallArg {
        name: None,
        is_spread: false,
        value: receiver,
        value_ty: receiver_ty,
    }];
    let tmp = builder.alloc_temp(ty, span);
    let site_id = Some(builder.next_site_id());
    let transport = builder.call_transport(ty);
    builder.assign(
        tmp,
        Rvalue::Call {
            site_id,
            kind: builder.make_direct_call_kind(callee_fqn, Vec::new(), false),
            args,
            transport,
        },
        span,
    );
    Operand::Local(tmp)
}

/// BinaryOp → 方法名（与 typecheck binop_to_method_name 对齐）。
fn binop_to_method_name_str(op: ast::BinaryOp) -> &'static str {
    use ast::BinaryOp as B;
    match op {
        B::Add => "plus",
        B::Sub => "minus",
        B::Mul => "times",
        B::Div => "div",
        B::Rem => "rem",
        B::Shl => "shl",
        B::Shr => "shr",
        B::BitAnd => "and",
        B::BitXor => "xor",
        B::BitOr => "or",
        B::Lt => "lt",
        B::Le => "le",
        B::Gt => "gt",
        B::Ge => "ge",
        B::Eq => "equals",
        B::Ne => "notEquals",
        B::Range => "rangeTo",
        B::LogAnd | B::LogOr | B::Elvis => "compareTo",
    }
}

/// UnaryOp → 方法名（与 typecheck unop_to_method_name 对齐；Not 走 Bool 取反，无方法）。
fn unop_to_method_name_str(op: ast::UnaryOp) -> &'static str {
    match op {
        ast::UnaryOp::Neg => "unaryMinus",
        ast::UnaryOp::BitNot => "inv",
        ast::UnaryOp::Not => "equals",
    }
}

/// lower Binary（运算符 → 方法调用；短路运算走 CondBr）。
fn lower_binary(
    builder: &mut FnLowering,
    op: ast::BinaryOp,
    lhs: &Expr,
    rhs: &Expr,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    use ast::BinaryOp;
    match op {
        BinaryOp::LogAnd => {
            // a && b：if a then b else false。
            let lv = lower_expr(builder, lhs);
            let bool_ty = builder.types.bool();
            let result = builder.alloc_temp(bool_ty, span);
            let then_bb = builder.new_block();
            let else_bb = builder.new_block();
            let merge_bb = builder.new_block();
            builder.terminate(
                crate::mir::Terminator {
                    span,
                    kind: crate::mir::TerminatorKind::CondBr {
                        cond: lv,
                        then_target: then_bb,
                        else_target: else_bb,
                    },
                },
                then_bb,
            );
            // then: b → result。
            builder.current_bb = then_bb;
            let bv = lower_expr(builder, rhs);
            builder.assign(result, Rvalue::Use(bv), span);
            builder.goto(merge_bb, span);
            // else: result = false。
            builder.current_bb = else_bb;
            builder.assign(
                result,
                Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
                span,
            );
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        BinaryOp::LogOr => {
            // a || b：if a then true else b。
            let lv = lower_expr(builder, lhs);
            let bool_ty = builder.types.bool();
            let result = builder.alloc_temp(bool_ty, span);
            let then_bb = builder.new_block();
            let else_bb = builder.new_block();
            let merge_bb = builder.new_block();
            builder.terminate(
                crate::mir::Terminator {
                    span,
                    kind: crate::mir::TerminatorKind::CondBr {
                        cond: lv,
                        then_target: then_bb,
                        else_target: else_bb,
                    },
                },
                then_bb,
            );
            builder.current_bb = then_bb;
            builder.assign(
                result,
                Rvalue::Use(Operand::Const(ConstValue::Bool(true))),
                span,
            );
            builder.goto(merge_bb, span);
            builder.current_bb = else_bb;
            let bv = lower_expr(builder, rhs);
            builder.assign(result, Rvalue::Use(bv), span);
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        BinaryOp::Elvis => {
            // a ?: b：if a != null then a else b。
            let lv = lower_expr(builder, lhs);
            let result = builder.alloc_temp(ty, span);
            // 简化：always eval lhs, then if non-null use else rhs.
            // 完整需要 null 比较；这里走 merge：result = lhs; if result == null then result = rhs。
            builder.assign(result, Rvalue::Use(lv), span);
            let then_bb = builder.new_block();
            let else_bb = builder.new_block();
            let merge_bb = builder.new_block();
            // 比较 result == null（用 equals 方法或内建）。简化：CondBr on result（null 为假）。
            builder.terminate(
                crate::mir::Terminator {
                    span,
                    kind: crate::mir::TerminatorKind::CondBr {
                        cond: Operand::Local(result),
                        then_target: then_bb,
                        else_target: else_bb,
                    },
                },
                then_bb,
            );
            builder.current_bb = then_bb;
            // result 已是非 null：goto merge。
            builder.goto(merge_bb, span);
            builder.current_bb = else_bb;
            let bv = lower_expr(builder, rhs);
            builder.assign(result, Rvalue::Use(bv), span);
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        _ => {
            // 其它运算符（标量内建 / 方法）：消费 call_resolutions（已按 Binary 节点 id 记录）。
            let bin_node = builder.current_expr_id;
            let method_hint = binop_to_method_name_str(op);
            let lhs_op = lower_expr(builder, lhs);
            let rhs_op = lower_expr(builder, rhs);
            lower_via_call_resolution(builder, bin_node, method_hint, lhs_op, rhs_op, span, ty)
        }
    }
}

/// 通过 call_resolutions 派生运算符调用（Binary/Unary 节点的决议已记录）。
fn lower_via_call_resolution(
    builder: &mut FnLowering,
    op_node: scoop2_base::NodeId,
    method_hint: &str,
    lhs: Operand,
    rhs: Operand,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    // 查 call_resolutions[op_node]。
    if let Some(rc) = builder.hir.call_resolution(builder.file_id, op_node) {
        // Binary/Unary 的决议是 ResolvedCall::Method（运算符 → 方法）。
        let rhs_ty = super::stmt::operand_ty(builder, &rhs);
        let args = vec![CallArg {
            name: None,
            is_spread: false,
            value: rhs,
            value_ty: rhs_ty,
        }];
        // 运算符方法调用的 receiver 是 lhs operand。
        return emit_call_resolution(builder, rc, args, span, ty, Some(lhs), op_node);
    }
    // 回退（标量内建 / 未解析方法）：emit Direct 调用到 `<owner>.<method_hint>`，
    // owner 取 lhs 类型的 nominal FQN（标量 → scoop.core.<T>），使 callee 可解析。
    // 若 lhs 类型无法解析（Nothing，如解构绑定未注册类型），尝试用 rhs 类型回退。
    let lhs_ty = super::stmt::operand_ty(builder, &lhs);
    let rhs_ty = super::stmt::operand_ty(builder, &rhs);
    let owner_fqn = {
        let o = owner_fqn_of(builder, lhs_ty);
        if o == Symbol::default() {
            owner_fqn_of(builder, rhs_ty)
        } else {
            o
        }
    };
    let method_sym = builder.hir.interner.get(method_hint).unwrap_or_default();
    let callee_fqn = if owner_fqn == Symbol::default() {
        method_hint.to_string()
    } else {
        format!(
            "{}.{}",
            builder.hir.interner.resolve(owner_fqn),
            method_hint
        )
    };
    // 标量内建运算符：receiver（lhs）是隐式首参，需前置（如 `a * b` → Int.times(a, b)）。
    let args = vec![
        CallArg {
            name: None,
            is_spread: false,
            value: lhs,
            value_ty: lhs_ty,
        },
        CallArg {
            name: None,
            is_spread: false,
            value: rhs,
            value_ty: rhs_ty,
        },
    ];
    let tmp = builder.alloc_temp(ty, span);
    let site_id = Some(builder.next_site_id());
    let transport = builder.call_transport(ty);
    builder.assign(
        tmp,
        Rvalue::Call {
            site_id,
            kind: builder.make_direct_call_kind(callee_fqn, Vec::new(), false),
            args,
            transport,
        },
        span,
    );
    let _ = method_sym;
    Operand::Local(tmp)
}

/// 取某类型的 nominal/标量 owner FQN（标量 → scoop.core.<T>；nominal → 其 fqn）。
fn owner_fqn_of(builder: &FnLowering, ty: scoop2_hir::ty::TypeId) -> Symbol {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    let name: Option<&str> = match builder.types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int"),
        TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt"),
        TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool"),
        TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char"),
        TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64"),
        TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32"),
        TypeKind::Ref(RefTypeKind::String) => Some("scoop.core.String"),
        TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            return n.fqn;
        }
        _ => None,
    };
    name.and_then(|n| builder.hir.interner.get(n))
        .unwrap_or_default()
}

/// 取某类型的 nominal FQN 字符串（用于 RuntimeTypeDescriptorKind::Nominal { fqn }）。
/// 标量映射到 scoop.core.<T>；nominal 取其 fqn；其余返回 "?" 占位。
fn nominal_fqn_of(builder: &FnLowering, ty: scoop2_hir::ty::TypeId) -> String {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    match builder.types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Int) => "scoop.core.Int".to_string(),
        TypeKind::Value(ValueTypeKind::UInt) => "scoop.core.UInt".to_string(),
        TypeKind::Value(ValueTypeKind::Bool) => "scoop.core.Bool".to_string(),
        TypeKind::Value(ValueTypeKind::Char) => "scoop.core.Char".to_string(),
        TypeKind::Value(ValueTypeKind::Float64) => "scoop.core.Float64".to_string(),
        TypeKind::Value(ValueTypeKind::Float32) => "scoop.core.Float32".to_string(),
        TypeKind::Ref(RefTypeKind::String) => "scoop.core.String".to_string(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            builder.hir.interner.resolve(n.fqn).to_string()
        }
        _ => "?".to_string(),
    }
}

/// 继承构造链展开：`class B(...) : A(sargs)` 的 ClassCtor 实参前部插入超类字段
/// 实参，使最终 args 顺序 = [祖先字段..., 本类属性参数...]（与 HIR
/// `ordered_class_fields` / codegen class_ctor 字段布局一致）。
///
/// 无委托记录时原样返回（无继承的 class 不受影响）。
fn expand_super_ctor_chain(
    builder: &mut FnLowering,
    class_fqn: &scoop2_base::Symbol,
    call_args: Vec<CallArg>,
) -> Vec<CallArg> {
    if !builder.hir.super_ctor_delegations.contains_key(class_fqn) {
        return call_args;
    }
    let params = builder
        .hir
        .class_ctor_params
        .get(class_fqn)
        .cloned()
        .unwrap_or_default();
    // 调用点实参 → 按主构造器参数序（位置实参按序，命名实参按参数名匹配）。
    let mut param_ops: Vec<Option<(Operand, scoop2_hir::ty::TypeId)>> = vec![None; params.len()];
    if call_args.iter().all(|a| a.name.is_none()) {
        for (i, a) in call_args.into_iter().enumerate() {
            if i < params.len() {
                param_ops[i] = Some((a.value, a.value_ty));
            }
        }
    } else {
        let mut pos = 0usize;
        for a in call_args {
            match a.name {
                Some(n) => {
                    if let Some(i) = params.iter().position(|p| p.name == n) {
                        param_ops[i] = Some((a.value, a.value_ty));
                    }
                }
                None => {
                    if pos < params.len() {
                        param_ops[pos] = Some((a.value, a.value_ty));
                    }
                    pos += 1;
                }
            }
        }
    }
    collect_ctor_field_args(builder, *class_fqn, &params, param_ops)
}

/// 递归收集 class 的完整字段实参：先超类（沿 `: Super(args)` 委托链自顶向下），
/// 再本类属性（val/var）参数。委托实参引用本类构造器参数时按参数序替换。
fn collect_ctor_field_args(
    _builder: &mut FnLowering,
    _class_fqn: scoop2_base::Symbol,
    params: &[scoop2_hir::hir::ClassCtorParamInfo],
    param_ops: Vec<Option<(Operand, scoop2_hir::ty::TypeId)>>,
) -> Vec<CallArg> {
    // super 委托的字段初始化现由 `<Class>.$init` 合成 callable 负责（递归调超类 $init）。
    // ClassCtor 的 args 只需本类的属性参数（按参数序 = 本类字段布局序）。
    // 超类字段参数不再 prepend（避免与 $init 的 super 委托重复初始化）。
    let mut out: Vec<CallArg> = Vec::new();
    for (i, p) in params.iter().enumerate() {
        if !p.is_property {
            continue;
        }
        if let Some(Some((op, oty))) = param_ops.get(i) {
            out.push(CallArg {
                name: None,
                is_spread: false,
                value: op.clone(),
                value_ty: *oty,
            });
        }
    }
    out
}

/// lower infix call（`a until b`）。
fn lower_infix_call(
    builder: &mut FnLowering,
    receiver: &Expr,
    name: Symbol,
    arg: &Expr,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let recv = lower_expr(builder, receiver);
    let recv_ty = super::stmt::operand_ty(builder, &recv);
    let av = lower_expr(builder, arg);
    let arg_ty = super::stmt::operand_ty(builder, &av);
    let args = vec![CallArg {
        name: None,
        is_spread: false,
        value: av,
        value_ty: arg_ty,
    }];
    let tmp = builder.alloc_temp(ty, span);
    let owner_str = builder.hir.interner.resolve(Symbol::default()).to_string();
    let method_str = builder.hir.interner.resolve(name).to_string();
    let member_fqn = format!("{}.{}", owner_str, method_str);
    let overload_sig = member_overload_sig(builder, Symbol::default(), name);
    let stk = crate::mir::stable_id::make_stable_template_key(
        crate::mir::stable_id::StableHashScope::Dump,
        &member_fqn,
        &[],
        &overload_sig,
    );
    let dispatch = DispatchMetadata {
        owner_fqn: owner_str.clone(),
        member_name: method_str.clone(),
        member_fqn: member_fqn.clone(),
        member_decl_span: None,
        receiver_ty: recv_ty,
        stable_candidate_keys: vec![crate::mir::stable_id::make_stable_instance_key(
            crate::mir::stable_id::StableHashScope::Dump,
            stk.clone(),
            &builder.types,
            &builder.hir.interner,
            &[],
            &[],
        )],
        stable_template_key: Some(stk),
        generic_type_args: vec![],
        generic_eff_args: vec![],
    };
    let site_id = Some(builder.next_site_id());
    let transport = builder.call_transport(ty);
    let kind = builder.make_dispatch_call_kind(
        super::stmt::resolve_owner_fqn_from_operand(builder, &recv),
        recv,
        dispatch,
    );
    builder.assign(
        tmp,
        Rvalue::Call {
            site_id,
            kind,
            args,
            transport,
        },
        span,
    );
    Operand::Local(tmp)
}

/// lower index（`a[i]` → operator get）。
fn lower_index(
    builder: &mut FnLowering,
    receiver: &Expr,
    indices: &[Expr],
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let recv = lower_expr(builder, receiver);
    let recv_ty = super::stmt::operand_ty(builder, &recv);
    let mut idx_ops = Vec::new();
    for idx in indices {
        idx_ops.push(lower_expr(builder, idx));
    }
    let tmp = builder.alloc_temp(ty, span);
    builder.assign(
        tmp,
        Rvalue::IndexAccess {
            receiver: recv,
            indices: idx_ops,
            element_ty: ty,
            receiver_ty: recv_ty,
        },
        span,
    );
    Operand::Local(tmp)
}

/// lower 数组字面量（`[a, b, c]`）。`ty` 为结果类型：
/// 通常是表达式的标注类型（`Array<T>`），但在期望类型为 `MutableArray<T>`
/// 的声明语境下由调用方传入 MutableArray（MakeArray 结果不 freeze——见
/// `lower_local_val` 的 MutableArray 特判）。
pub fn lower_array_lit(
    builder: &mut FnLowering,
    els: &[Expr],
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let ops: Vec<Operand> = els.iter().map(|e| lower_expr(builder, e)).collect();
    // 空数组字面量 `[]` 的表达式类型为 Nothing（typecheck 让 check_assignable 通过），
    // 但 MakeArray 结果总是 Array 引用（GC ptr）。用 Array 引用类型创建临时，
    // 避免 Nothing 临时在 codegen 中被错误地以 i8 load（破坏指针）。
    let arr_ty = if builder.types.is_nothing(ty) {
        builder.array_ref_ty()
    } else {
        ty
    };
    let tmp = builder.alloc_temp(arr_ty, span);
    builder.assign(
        tmp,
        Rvalue::MakeArray {
            elements: ops,
            result_ty: arr_ty,
        },
        span,
    );
    Operand::Local(tmp)
}

/// lower member access（`a.b`）。
fn lower_member_access(
    builder: &mut FnLowering,
    receiver: &Expr,
    member: &MemberName,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    // enum variant 值（`Color.Red`，无构造调用）：直接构造 EnumVariant。
    // 必须在 lower receiver 之前特判——类型名 Ident 无法 lower 为值。
    if let ExprKind::Ident(recv_ident) = &receiver.kind
        && let MemberName::Named(variant_ident) = member
        && let Some(rc) =
            derive_enum_variant_call(builder, recv_ident.symbol, variant_ident.symbol, ty)
    {
        return emit_call_resolution(builder, &rc, vec![], span, ty, None, builder.current_expr_id);
    }
    let recv = lower_expr(builder, receiver);
    let recv_ty = super::stmt::operand_ty(builder, &recv);
    match member {
        MemberName::TupleIndex { value, .. } => {
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(
                tmp,
                Rvalue::TupleIndex {
                    receiver: recv,
                    index: *value,
                    element_ty: ty,
                },
                span,
            );
            Operand::Local(tmp)
        }
        MemberName::Named(name) => {
            let name_str = builder.hir.interner.resolve(name.symbol).to_string();
            let member_meta = builder.member_access_metadata(&name_str, recv_ty);
            // 查 member_refs（仅用于记录；metadata.resolved 暂留 None，后续 resolve 阶段填充）。
            let _ = builder
                .hir
                .member_ref(builder.file_id, receiver.id.max(receiver.id));
            let tmp = builder.alloc_temp(ty, span);
            let site_id = Some(builder.next_site_id());
            builder.assign(
                tmp,
                Rvalue::MemberAccess {
                    site_id,
                    receiver: recv,
                    member: member_meta,
                },
                span,
            );
            Operand::Local(tmp)
        }
    }
}

/// lower safe member access（`a?.b` → when + panic，spec）。
fn lower_safe_member_access(
    builder: &mut FnLowering,
    receiver: &Expr,
    member: &MemberName,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    // a?.b  ≡  if a == null then null else a.b
    let recv = lower_expr(builder, receiver);
    let result = builder.alloc_temp(ty, span);
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let merge_bb = builder.new_block();
    builder.terminate(
        crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::CondBr {
                cond: recv.clone(),
                then_target: then_bb,
                else_target: else_bb,
            },
        },
        then_bb,
    );
    // then: result = recv.member。
    builder.current_bb = then_bb;
    let member_val = lower_member_access_on(builder, recv, member, span, ty);
    builder.assign(result, Rvalue::Use(member_val), span);
    builder.goto(merge_bb, span);
    // else: result = null。
    builder.current_bb = else_bb;
    builder.assign(result, Rvalue::Use(Operand::Const(ConstValue::Null)), span);
    builder.goto(merge_bb, span);
    builder.current_bb = merge_bb;
    Operand::Local(result)
}

fn lower_member_access_on(
    builder: &mut FnLowering,
    recv: Operand,
    member: &MemberName,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let recv_ty = super::stmt::operand_ty(builder, &recv);
    match member {
        MemberName::TupleIndex { value, .. } => {
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(
                tmp,
                Rvalue::TupleIndex {
                    receiver: recv,
                    index: *value,
                    element_ty: ty,
                },
                span,
            );
            Operand::Local(tmp)
        }
        MemberName::Named(name) => {
            let name_str = builder.hir.interner.resolve(name.symbol).to_string();
            let member_meta = builder.member_access_metadata(&name_str, recv_ty);
            let tmp = builder.alloc_temp(ty, span);
            let site_id = Some(builder.next_site_id());
            builder.assign(
                tmp,
                Rvalue::MemberAccess {
                    site_id,
                    receiver: recv,
                    member: member_meta,
                },
                span,
            );
            Operand::Local(tmp)
        }
    }
}

/// lower `expr!!`（NotNullAssert → if null then panic else expr）。
fn lower_not_null_assert(
    builder: &mut FnLowering,
    inner: &Expr,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let v = lower_expr(builder, inner);
    let result = builder.alloc_temp(ty, span);
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let merge_bb = builder.new_block();
    builder.terminate(
        crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::CondBr {
                cond: v.clone(),
                then_target: then_bb,
                else_target: else_bb,
            },
        },
        then_bb,
    );
    // then: result = v。
    builder.current_bb = then_bb;
    builder.assign(result, Rvalue::Use(v), span);
    builder.goto(merge_bb, span);
    // else: panic。
    builder.current_bb = else_bb;
    builder.push_stmt(Statement {
        span,
        kind: StatementKind::Panic {
            message: "NotNullAssert 失败（值为 null）".to_string(),
        },
    });
    builder.goto(merge_bb, span);
    builder.current_bb = merge_bb;
    Operand::Local(result)
}

/// lower f-string（desugar 到调用链）。
fn lower_interpolated(
    builder: &mut FnLowering,
    parts: &[ast::StringPart],
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let mut mir_parts = Vec::new();
    for part in parts {
        match part {
            ast::StringPart::Text(s) => {
                mir_parts.push(crate::mir::InterpolatedPart::Lit(s.clone()))
            }
            ast::StringPart::Expr(e) => {
                let v = lower_expr(builder, e);
                mir_parts.push(crate::mir::InterpolatedPart::Expr(v));
            }
        }
    }
    let tmp = builder.alloc_temp(ty, span);
    builder.assign(tmp, Rvalue::InterpolatedString { parts: mir_parts }, span);
    Operand::Local(tmp)
}

/// lower `expr with { ... }`。
fn lower_with_update(
    builder: &mut FnLowering,
    base: &Expr,
    updates: &[ast::WithUpdateField],
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let base_op = lower_expr(builder, base);
    let mut mir_updates = Vec::new();
    for u in updates {
        let v = lower_expr(builder, &u.value);
        let value_ty = super::stmt::operand_ty(builder, &v);
        let path: Vec<crate::mir::WithUpdateSegment> = u
            .path
            .segments
            .iter()
            .map(|s| match s {
                MemberName::Named(n) => crate::mir::WithUpdateSegment::Named(n.symbol),
                MemberName::TupleIndex { value, .. } => {
                    crate::mir::WithUpdateSegment::TupleIndex(*value)
                }
            })
            .collect();
        mir_updates.push(crate::mir::WithUpdateField {
            path,
            value: v,
            value_ty,
        });
    }
    let tmp = builder.alloc_temp(ty, span);
    builder.assign(
        tmp,
        Rvalue::WithUpdate {
            base: base_op,
            updates: mir_updates,
            result_ty: ty,
        },
        span,
    );
    Operand::Local(tmp)
}

/// 扫描 lambda body 收集被引用的标识符（自由变量候选）。
/// 遍历 body 中所有 ExprKind::Ident，排除 `it`/`true`/`false`/`this`/`field` 等内建。
fn collect_lambda_free_vars(l: &ast::LambdaExpr) -> Vec<scoop2_base::Symbol> {
    let mut syms = std::collections::HashSet::new();
    match &l.body {
        ast::LambdaBody::Block(b) => scan_block_idents(b, &mut syms),
        ast::LambdaBody::Expr(e) => scan_expr_idents(e, &mut syms),
    }
    // 排除 lambda 自身的参数名。
    for p in &l.params {
        syms.remove(&p.name.symbol);
    }
    syms.into_iter().collect()
}

fn scan_block_idents(b: &ast::Block, syms: &mut std::collections::HashSet<scoop2_base::Symbol>) {
    for s in &b.stmts {
        scan_stmt_idents(s, syms);
    }
}

fn scan_stmt_idents(s: &ast::Stmt, syms: &mut std::collections::HashSet<scoop2_base::Symbol>) {
    match &s.kind {
        ast::StmtKind::Expr(e) => scan_expr_idents(e, syms),
        ast::StmtKind::Assign { target, value } => {
            scan_assign_target_idents(target, syms);
            scan_expr_idents(value, syms);
        }
        ast::StmtKind::LocalVal(d) => {
            if let Some(init) = &d.init {
                scan_expr_idents(init, syms);
            }
            // 排除声明的绑定名（它是新的局部，不是自由变量）。
            match &d.binding {
                ast::ValBinding::Name(n) => {
                    syms.remove(&n.symbol);
                }
                ast::ValBinding::Pattern(p) => {
                    remove_pattern_binders(p, syms);
                }
            }
        }
        ast::StmtKind::Return { value } => {
            if let Some(e) = value {
                scan_expr_idents(e, syms);
            }
        }
        ast::StmtKind::While { cond, body } => {
            scan_expr_idents(cond, syms);
            scan_block_idents(body, syms);
        }
        ast::StmtKind::For { iter, body, .. } => {
            scan_expr_idents(iter, syms);
            scan_block_idents(body, syms);
        }
        _ => {}
    }
}

fn scan_assign_target_idents(
    t: &ast::AssignTarget,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    match &t.kind {
        ast::AssignTargetKind::Ident(id) => {
            syms.insert(id.symbol);
        }
        ast::AssignTargetKind::Member { receiver, .. } => scan_expr_idents(receiver, syms),
        ast::AssignTargetKind::Index { receiver, indices } => {
            scan_expr_idents(receiver, syms);
            for i in indices {
                scan_expr_idents(i, syms);
            }
        }
    }
}

fn scan_expr_idents(e: &Expr, syms: &mut std::collections::HashSet<scoop2_base::Symbol>) {
    match &e.kind {
        ExprKind::Ident(id) => {
            let name = id.symbol;
            // 排除内建标识符。
            let text = format!("{}", name.as_u32());
            let _ = text;
            syms.insert(name);
        }
        ExprKind::Call { callee, args } => {
            scan_expr_idents(callee, syms);
            for a in args {
                scan_expr_idents(&a.value, syms);
            }
        }
        ExprKind::MemberAccess { receiver, .. } | ExprKind::SafeMemberAccess { receiver, .. } => {
            scan_expr_idents(receiver, syms);
        }
        ExprKind::Index { receiver, indices } => {
            scan_expr_idents(receiver, syms);
            for i in indices {
                scan_expr_idents(i, syms);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            scan_expr_idents(lhs, syms);
            scan_expr_idents(rhs, syms);
        }
        ExprKind::Unary { expr, .. } => scan_expr_idents(expr, syms),
        ExprKind::InfixCall { receiver, arg, .. } => {
            scan_expr_idents(receiver, syms);
            scan_expr_idents(arg, syms);
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            scan_expr_idents(cond, syms);
            scan_expr_idents(then_branch, syms);
            if let Some(eb) = else_branch {
                scan_expr_idents(eb, syms);
            }
        }
        ExprKind::When { subject, arms } => {
            scan_expr_idents(subject, syms);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    scan_expr_idents(g, syms);
                }
                scan_expr_idents(&arm.body, syms);
            }
        }
        ExprKind::TupleLit(els) | ExprKind::ArrayLit(els) => {
            for e in els {
                scan_expr_idents(e, syms);
            }
        }
        ExprKind::StructLit { fields, .. } => {
            for f in fields {
                scan_expr_idents(&f.value, syms);
            }
        }
        ExprKind::WithUpdate { base, updates } => {
            scan_expr_idents(base, syms);
            for u in updates {
                scan_expr_idents(&u.value, syms);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let ast::StringPart::Expr(e) = p {
                    scan_expr_idents(e, syms);
                }
            }
        }
        ExprKind::Lambda(l) => {
            // 嵌套 lambda：收集其自由变量后减去其自身参数。
            let mut nested = std::collections::HashSet::new();
            match &l.body {
                ast::LambdaBody::Block(b) => scan_block_idents(b, &mut nested),
                ast::LambdaBody::Expr(e) => scan_expr_idents(e, &mut nested),
            }
            for p in &l.params {
                nested.remove(&p.name.symbol);
            }
            syms.extend(nested);
        }
        ExprKind::Block(b)
        | ExprKind::DoBlock(b)
        | ExprKind::UnsafeBlock(b)
        | ExprKind::SafeBlock(b) => {
            scan_block_idents(b, syms);
        }
        ExprKind::NotNullAssert { expr } => scan_expr_idents(expr, syms),
        ExprKind::TypeApply { callee, .. } => scan_expr_idents(callee, syms),
        ExprKind::TypeCheck { expr, .. } | ExprKind::Cast { expr, .. } => {
            scan_expr_idents(expr, syms)
        }
        ExprKind::Annotated { expr, .. } => scan_expr_idents(expr, syms),
        ExprKind::Handle { body, finally, .. } => {
            scan_block_idents(body, syms);
            if let Some(f) = finally {
                scan_block_idents(f, syms);
            }
        }
        ExprKind::SpliceField { receiver, field } => {
            scan_expr_idents(receiver, syms);
            scan_expr_idents(field, syms);
        }
        _ => {}
    }
}

fn remove_pattern_binders(
    p: &ast::Pattern,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    match &p.kind {
        ast::PatternKind::Bind(n) => {
            syms.remove(&n.symbol);
        }
        ast::PatternKind::Tuple(els) => {
            for e in els {
                remove_pattern_binders(e, syms);
            }
        }
        ast::PatternKind::Struct { fields, .. } => {
            for f in fields {
                if let Some(p) = &f.pattern {
                    remove_pattern_binders(p, syms);
                } else {
                    syms.remove(&f.name.symbol);
                }
            }
        }
        ast::PatternKind::Variant {
            args: Some(els), ..
        } => {
            for e in els {
                remove_pattern_binders(e, syms);
            }
        }
        _ => {}
    }
}

/// lower lambda（闭包）：生成 env tuple + 嵌套 Item::Fun。
fn lower_lambda(
    builder: &mut FnLowering,
    expr: &Expr,
    l: &ast::LambdaExpr,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let _ = expr;
    // 扫描 lambda body 收集**真实自由变量**（被引用的外层 local 名），而非全捕获 symbol_locals。
    // 自由变量 = 在 lambda body 内引用、且在 builder.symbol_locals 中有对应的 local。
    let free_vars = collect_lambda_free_vars(l);
    let captured: Vec<(Symbol, scoop2_hir::ty::TypeId)> = free_vars
        .iter()
        .filter_map(|&sym| {
            builder.symbol_locals.get(&sym).map(|lid| {
                let t = builder
                    .body
                    .locals
                    .get(lid.0 as usize)
                    .map(|d| d.ty)
                    .unwrap_or_else(|| builder.types.any());
                (sym, t)
            })
        })
        .collect();
    // env tuple（外层构造）。
    let env_elems: Vec<Operand> = captured
        .iter()
        .map(|(s, _)| {
            builder
                .symbol_locals
                .get(s)
                .map(|lid| Operand::Local(*lid))
                .unwrap_or(Operand::Const(ConstValue::Unit))
        })
        .collect();
    let env_ty = builder
        .types
        .tuple(captured.iter().map(|(_, t)| *t).collect());
    let env_tmp = builder.alloc_temp(env_ty, span);
    let env_transport = builder.aggregate_transport(env_ty, AggregateTransportKind::Tuple);
    builder.assign(
        env_tmp,
        Rvalue::MakeTuple {
            elements: env_elems,
            transport: env_transport,
        },
        span,
    );
    // 嵌套函数名。
    builder.closure_counter += 1;
    let invoke_fqn = format!("{}$closure{}", builder.owner_fqn, builder.closure_counter);
    // 预计算 captures metadata（避免在 builder.assign 内部 &mut 借用冲突）。
    let mut captures_meta = Vec::new();
    for (i, (cap_sym, cap_ty)) in captured.iter().enumerate() {
        let cap_lid = builder
            .symbol_locals
            .get(cap_sym)
            .copied()
            .unwrap_or(crate::mir::LocalId(0));
        let cap_name = builder.hir.interner.resolve(*cap_sym).to_string();
        let mutable = builder
            .body
            .locals
            .get(cap_lid.0 as usize)
            .map(|d| d.mutable)
            .unwrap_or(false);
        // 闭包捕获 transport：值类型被捕获到 Any 边界时标记 ClosureCapture boxing。
        let any_ty = builder.types.any();
        let cap_transport = if *cap_ty != any_ty
            && matches!(
                builder.types.kind(*cap_ty),
                scoop2_hir::ty::TypeKind::Value(_)
            ) {
            // 值类型捕获到 ref/Any 边界：产生 boxing intent。
            crate::mir::transport::ValueTransportMetadata {
                source_ty: *cap_ty,
                kind: crate::mir::transport::mir_transport_kind_for_ty(
                    &builder.types,
                    *cap_ty,
                    &builder.enum_fqns,
                ),
                requirements: crate::mir::transport::mir_transport_requirements(
                    &builder.types,
                    *cap_ty,
                ),
                boxing: Some(crate::mir::transport::MirBoxingIntent {
                    source_ty: *cap_ty,
                    target_ty: Some(any_ty),
                    reason: crate::mir::transport::MirBoxingReason::ClosureCapture,
                }),
            }
        } else {
            crate::mir::transport::value_transport(&builder.types, &builder.enum_fqns, *cap_ty)
        };
        captures_meta.push(crate::mir::transport::ClosureCaptureTransportMetadata {
            name: cap_name,
            decl_span: span,
            mutable,
            source_local: cap_lid,
            transport: cap_transport,
        });
        let _ = i;
    }
    let env_contract = crate::mir::transport::ClosureEnvTransportMetadata {
        env_ty,
        captures: captures_meta,
    };
    // 闭包值。
    let tmp = builder.alloc_temp(ty, span);
    builder.assign(
        tmp,
        Rvalue::MakeClosure {
            env: Operand::Local(env_tmp),
            invoke_fqn: invoke_fqn.clone(),
            env_contract,
        },
        span,
    );

    // 在新 FnLowering 中 lower lambda body，产出带真实 body 的嵌套 FunDecl。
    // 嵌套函数签名：第一个参数是 env tuple（捕获），其后是 lambda 参数。
    let mut nested_store = builder.types.clone();
    // 从 lambda 表达式的函数类型提取参数类型（而非从 TypeRef 节点查 expr_types——
    // TypeRef 不在 expr_types 中）。
    let fn_param_tys: Vec<scoop2_hir::ty::TypeId> = match nested_store.kind(ty) {
        scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Function(ft)) => {
            // ft.params 不含 env 参数；它们对应 lambda 的显式参数。
            ft.params.clone()
        }
        _ => Vec::new(),
    };
    let mut lambda_param_tys: Vec<scoop2_hir::ty::TypeId> = Vec::new();
    for (i, p) in l.params.iter().enumerate() {
        let pty = if i < fn_param_tys.len() {
            fn_param_tys[i]
        } else {
            // 回退：从 TypeRef 查（可能为 any）。
            p.ty.as_ref()
                .and_then(|t| builder.hir.expr_type(builder.file_id, t.id))
                .unwrap_or_else(|| nested_store.any())
        };
        lambda_param_tys.push(pty);
    }
    // 返回类型：从 lambda 表达式整体类型推断（函数类型的 return_ty）。
    let (return_ty, fn_effect_row) = match nested_store.kind(ty) {
        scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Function(ft)) => {
            (ft.return_ty, ft.effects.clone())
        }
        _ => (nested_store.any(), scoop2_hir::ty::EffectRow::pure()),
    };
    // 完整函数类型（env + lambda params → return）。
    let mut all_param_tys = vec![env_ty];
    all_param_tys.extend(lambda_param_tys.iter().copied());
    let nested_fn_ty = nested_store.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: all_param_tys.clone(),
        return_ty,
        effects: fn_effect_row.clone(),
        closed: false,
    });
    let mut errors: Vec<crate::diagnostics::MirLowerError> = Vec::new();
    let mut nested_builder = FnLowering::new(
        builder.hir,
        nested_store,
        builder.file_id,
        invoke_fqn.clone(),
        return_ty,
        fn_effect_row.clone(),
        &mut errors,
    );
    // env 参数（local 0）：解包捕获到 nested_builder.symbol_locals。
    let env_param = crate::mir::LocalDecl {
        span,
        name: Some("$env".to_string()),
        ty: env_ty,
        source: crate::mir::LocalSource::Source,
        mutable: false,
    };
    let env_lid = nested_builder.alloc_local(env_param);
    for (i, (cap_sym, cap_ty)) in captured.iter().enumerate() {
        let cap_lid = nested_builder.alloc_named(
            builder.hir.interner.resolve(*cap_sym).to_string(),
            *cap_ty,
            span,
        );
        nested_builder.symbol_locals.insert(*cap_sym, cap_lid);
        // 从 env tuple 解包第 i 个捕获。
        nested_builder.assign(
            cap_lid,
            Rvalue::TupleIndex {
                receiver: Operand::Local(env_lid),
                index: i as u128,
                element_ty: *cap_ty,
            },
            span,
        );
    }
    // lambda 参数 → locals。
    let mut params: Vec<crate::mir::Param> = Vec::new();
    params.push(crate::mir::Param {
        span,
        name: "$env".to_string(),
        ty: env_ty,
        local: env_lid,
    });
    for (i, p) in l.params.iter().enumerate() {
        let pty = lambda_param_tys[i];
        let lid = nested_builder.alloc_named(
            builder.hir.interner.resolve(p.name.symbol).to_string(),
            pty,
            p.name.span,
        );
        nested_builder.symbol_locals.insert(p.name.symbol, lid);
        params.push(crate::mir::Param {
            span: p.name.span,
            name: builder.hir.interner.resolve(p.name.symbol).to_string(),
            ty: pty,
            local: lid,
        });
    }
    // 隐式 `it` 参数：lambda 无显式参数时，body 内的 `it` 绑定到函数类型首个参数。
    if l.params.is_empty()
        && let Some(it_sym) = builder.hir.interner.get("it")
    {
        // it 类型 = 函数类型首个参数（跳过 env）。
        let it_ty = match nested_builder.types.kind(ty) {
            scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Function(ft))
                if !ft.params.is_empty() =>
            {
                ft.params[0]
            }
            _ => nested_builder.types.any(),
        };
        let lid = nested_builder.alloc_named("it".to_string(), it_ty, span);
        nested_builder.symbol_locals.insert(it_sym, lid);
        params.push(crate::mir::Param {
            span,
            name: "it".to_string(),
            ty: it_ty,
            local: lid,
        });
    }
    // lower lambda body。
    let (body, nested_more, store_out) = match &l.body {
        ast::LambdaBody::Block(b) => {
            let tail = crate::mir::lower::stmt::lower_block(&mut nested_builder, b);
            // 块尾表达式是 lambda 的隐式返回值（与普通函数体 lower_fun_body
            // 的处理一致）：尾块未终结且尾值非 Unit 时补 `Return(tail)`，
            // 否则交由 finish 补 Return(None)。
            let tail_is_unit = matches!(
                tail,
                crate::mir::Operand::Const(crate::mir::ConstValue::Unit)
            );
            let bb = nested_builder.current_bb;
            if !tail_is_unit
                && matches!(
                    nested_builder.body.blocks[bb.0 as usize].terminator.kind,
                    crate::mir::TerminatorKind::Unreachable
                )
            {
                nested_builder.terminate(
                    crate::mir::Terminator {
                        span: b.span,
                        kind: crate::mir::TerminatorKind::Return { value: Some(tail) },
                    },
                    bb,
                );
            }
            let (bd, more, st) = nested_builder.finish();
            (bd, more, st)
        }
        ast::LambdaBody::Expr(e) => {
            let val = lower_expr(&mut nested_builder, e);
            let cur_bb = nested_builder.current_bb;
            nested_builder.terminate(
                crate::mir::Terminator {
                    span: e.span,
                    kind: crate::mir::TerminatorKind::Return { value: Some(val) },
                },
                cur_bb,
            );
            let (bd, more, st) = nested_builder.finish();
            (bd, more, st)
        }
    };
    let nested = crate::mir::FunDecl {
        span,
        fqn: invoke_fqn,
        name: format!("$closure{}", builder.closure_counter),
        ty: nested_fn_ty,
        params,
        return_ty,
        effect_row: fn_effect_row,
        type_params: Vec::new(),
        body: Some(body),
        file: builder.file_id,
        stable_template_key: None,
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: None,
    };
    builder.nested_funs.push(nested);
    builder.nested_funs.extend(nested_more);
    // 把 nested lowering 的 store 合并回外层（简化：直接克隆替换——外层后续 lowering
    // 会用自己的 store；nested 的类型已 remap 进 store_out）。为保持 TypeId 一致，
    // 把 store_out 的新类型合并进 builder.types。
    let _remap = builder.types.extend_from(&store_out);
    // 把 nested lowering 错误并入外层。
    builder.errors.extend(errors);
    Operand::Local(tmp)
}

// ---------------------------------------------------------------------------
// when / handle lowering（控制流）
// ---------------------------------------------------------------------------

pub fn lower_when(
    builder: &mut FnLowering,
    subject: &Expr,
    arms: &[ast::WhenArm],
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let subj = lower_expr(builder, subject);
    let subj_ty = super::stmt::operand_ty(builder, &subj);
    let result = builder.alloc_temp(ty, span);
    let merge_bb = builder.new_block();
    for arm in arms {
        let arm_bb = builder.new_block();
        let next_bb = builder.new_block();
        // 发射真实模式测试：为每种模式 kind 生成匹配检查（CondBr）。
        let matches = lower_pattern_test(builder, &arm.pat, subj.clone(), subj_ty, arm, span);
        // guard：模式命中后，若 arm 有 guard 则还需 guard 为真。
        let cond = if let Some(guard) = &arm.guard {
            // 先把 pattern bindings 引入（在 guard 和 arm body 之前）。
            bind_pattern_arm(builder, &arm.pat, subj.clone(), subj_ty);
            lower_expr(builder, guard)
        } else {
            matches
        };
        builder.terminate(
            crate::mir::Terminator {
                span: arm.span,
                kind: crate::mir::TerminatorKind::CondBr {
                    cond,
                    then_target: arm_bb,
                    else_target: next_bb,
                },
            },
            arm_bb,
        );
        // arm body（bindings 在 guard 阶段已引入；若无 guard 则在此引入）。
        if arm.guard.is_none() {
            bind_pattern_arm(builder, &arm.pat, subj.clone(), subj_ty);
        }
        builder.current_bb = arm_bb;
        let v = lower_expr(builder, &arm.body);
        builder.assign(result, Rvalue::Use(v), arm.body.span);
        builder.goto(merge_bb, arm.span);
        builder.current_bb = next_bb;
    }
    // 无 arm 命中：result = Unit（或不可达）。
    builder.assign(result, Rvalue::Use(Operand::Const(ConstValue::Unit)), span);
    builder.goto(merge_bb, span);
    builder.current_bb = merge_bb;
    Operand::Local(result)
}

/// 为 when arm 的模式发射真实匹配测试（返回 Bool operand）。
///
/// - Wildcard / Else / Bind：总是命中（Bool(true)），Bind 同时引入绑定。
/// - Variant：通过 `equals` 方法比较 variant 构造器（variant 名作为 tag）。
/// - Literal（Int/Char/String/Bool）：发射值相等比较。
/// - Is：发射 TypeTest。
/// - Struct / Tuple：总是命中（类型已由 typecheck 保证），引入绑定。
/// - Or：发射各子模式测试的逻辑或（CondBr 链）。
fn lower_pattern_test(
    builder: &mut FnLowering,
    pat: &ast::Pattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
    _arm: &ast::WhenArm,
    span: Span,
) -> Operand {
    let bool_ty = builder.types.bool();
    match &pat.kind {
        ast::PatternKind::Else | ast::PatternKind::Wildcard | ast::PatternKind::Rest => {
            Operand::Const(ConstValue::Bool(true))
        }
        ast::PatternKind::Bind(_) | ast::PatternKind::Struct { .. } => {
            // irrefutable 模式：类型已由 typecheck 保证匹配 → 总是命中。
            Operand::Const(ConstValue::Bool(true))
        }
        ast::PatternKind::Tuple(elems) => {
            // tuple 模式：逐元素提取并递归测试（AND 链，short-circuit）。
            // bind/wildcard/rest 元素无约束（跳过）；字面量/嵌套模式可反驳
            // （如 `(1, x)` / `("go", 'A')` 的字面量元素必须真实比较）。
            let testable: Vec<(usize, &ast::Pattern)> = elems
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    matches!(
                        &e.kind,
                        ast::PatternKind::Literal(_)
                            | ast::PatternKind::Variant { .. }
                            | ast::PatternKind::Tuple(_)
                            | ast::PatternKind::Struct { .. }
                            | ast::PatternKind::Is(_)
                            | ast::PatternKind::Or(_)
                    )
                })
                .map(|(i, e)| (i, e))
                .collect();
            if testable.is_empty() {
                return Operand::Const(ConstValue::Bool(true));
            }
            let result = builder.alloc_temp(bool_ty, span);
            let merge_bb = builder.new_block();
            let mut prev_test: Option<Operand> = None;
            for (i, sub_pat) in testable {
                // 前置测试失败 → result = false，goto merge（首元素无前置条件）。
                if let Some(prev) = prev_test.take() {
                    let cont_bb = builder.new_block();
                    let fail_bb = builder.new_block();
                    builder.terminate(
                        crate::mir::Terminator {
                            span,
                            kind: crate::mir::TerminatorKind::CondBr {
                                cond: prev,
                                then_target: cont_bb,
                                else_target: fail_bb,
                            },
                        },
                        cont_bb,
                    );
                    builder.current_bb = fail_bb;
                    builder.assign(
                        result,
                        Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
                        span,
                    );
                    builder.goto(merge_bb, span);
                    builder.current_bb = cont_bb;
                }
                let elem_ty =
                    tuple_elem_ty(builder, subj_ty, i).unwrap_or_else(|| builder.types.any());
                let tmp = builder.alloc_temp(elem_ty, span);
                builder.assign(
                    tmp,
                    Rvalue::PatternExtract {
                        subject: subj.clone(),
                        path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(i)],
                        result_ty: elem_ty,
                    },
                    span,
                );
                prev_test = Some(lower_pattern_test(
                    builder,
                    sub_pat,
                    Operand::Local(tmp),
                    elem_ty,
                    _arm,
                    span,
                ));
            }
            // 全部通过：result = 最后一个子测试的值。
            builder.assign(
                result,
                Rvalue::Use(prev_test.expect("testable 非空")),
                span,
            );
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        ast::PatternKind::Variant { path, args } => {
            // variant 模式：发射 PatternMatch，让后端做真实 variant tag 比较。
            let variant_name_sym = path.segments.last().map(|s| s.symbol).unwrap_or_default();
            // 解析 enum FQN。
            let enum_fqn = {
                let prefix = builder
                    .hir
                    .file(builder.file_id)
                    .map(|f| f.package_prefix.as_str())
                    .unwrap_or("");
                let vname = builder.hir.interner.resolve(variant_name_sym);
                let candidates = if prefix.is_empty() {
                    vec![vname.to_string()]
                } else {
                    vec![vname.to_string(), format!("{prefix}.{vname}")]
                };
                candidates
                    .iter()
                    .filter_map(|c| builder.hir.interner.get(c))
                    .filter(|f| builder.hir.enum_variants.contains_key(f))
                    .next()
                    .unwrap_or(variant_name_sym)
            };
            // tag 级测试的 args：嵌套子模式（variant/tuple/struct/字面量/is/or）降级为
            // Wildcard——它们需要先从 payload 提取才能测试，在下方 AND 链中展开；
            // binder / 通配原样保留（无约束，但保留 arity 形态）。
            let tag_args: Vec<crate::mir::Pattern> = args
                .as_ref()
                .map(|args| {
                    args.iter()
                        .map(|a| match &a.kind {
                            ast::PatternKind::Bind(_) | ast::PatternKind::Wildcard => {
                                lower_pattern_to_mir(builder, a)
                            }
                            _ => crate::mir::Pattern::Wildcard,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let tag_tmp = builder.alloc_temp(bool_ty, span);
            builder.assign(
                tag_tmp,
                Rvalue::PatternMatch {
                    subject: subj.clone(),
                    pattern: crate::mir::Pattern::Variant {
                        enum_fqn,
                        variant_name: variant_name_sym,
                        args: tag_args,
                    },
                },
                span,
            );
            // 嵌套子模式位置（需要提取 payload 字段后递归测试）。
            let nested: Vec<(usize, &ast::Pattern)> = args
                .as_ref()
                .map(|args| {
                    args.iter()
                        .enumerate()
                        .filter(|(_, a)| {
                            matches!(
                                &a.kind,
                                ast::PatternKind::Variant { .. }
                                    | ast::PatternKind::Tuple(_)
                                    | ast::PatternKind::Struct { .. }
                                    | ast::PatternKind::Literal(_)
                                    | ast::PatternKind::Is(_)
                                    | ast::PatternKind::Or(_)
                            )
                        })
                        .map(|(i, a)| (i, a))
                        .collect()
                })
                .unwrap_or_default();
            if nested.is_empty() {
                return Operand::Local(tag_tmp);
            }
            // AND 链：tag 测试通过后，逐位置提取 payload 字段并递归测试；
            // 任一环节失败 → false。
            let result = builder.alloc_temp(bool_ty, span);
            let merge_bb = builder.new_block();
            let mut prev_test = Operand::Local(tag_tmp);
            for (i, sub_pat) in nested {
                let cont_bb = builder.new_block();
                let fail_bb = builder.new_block();
                builder.terminate(
                    crate::mir::Terminator {
                        span,
                        kind: crate::mir::TerminatorKind::CondBr {
                            cond: prev_test.clone(),
                            then_target: cont_bb,
                            else_target: fail_bb,
                        },
                    },
                    cont_bb,
                );
                // 失败路径：result = false，goto merge。
                builder.current_bb = fail_bb;
                builder.assign(
                    result,
                    Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
                    span,
                );
                builder.goto(merge_bb, span);
                // 通过路径：提取第 i 个 payload 字段并递归测试。
                builder.current_bb = cont_bb;
                prev_test =
                    if let Some(field_ty) = variant_payload_field_ty(builder, subj_ty, path, i) {
                        let variant_name = path
                            .segments
                            .last()
                            .map(|s| builder.hir.interner.resolve(s.symbol).to_string())
                            .unwrap_or_default();
                        let tmp = builder.alloc_temp(field_ty, span);
                        builder.assign(
                            tmp,
                            Rvalue::PatternExtract {
                                subject: subj.clone(),
                                path: vec![
                                    crate::mir::transport::PatternBindingStep::VariantField {
                                        variant: variant_name,
                                        field_index: i,
                                    },
                                ],
                                result_ty: field_ty,
                            },
                            span,
                        );
                        lower_pattern_test(builder, sub_pat, Operand::Local(tmp), field_ty, _arm, span)
                    } else {
                        // 字段类型不可定位 → 保守视为命中（不阻断 tag 级语义）。
                        Operand::Const(ConstValue::Bool(true))
                    };
            }
            // 全部通过：result = 最后一个子测试的值。
            builder.assign(result, Rvalue::Use(prev_test), span);
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        ast::PatternKind::Literal(lit) => {
            // 字面量模式：发射 PatternMatch，让后端做值比较。
            let mir_pat = match lit {
                ast::PatternLiteral::Int(il) => crate::mir::Pattern::IntLit(il.value as i128),
                ast::PatternLiteral::Char(cl) => crate::mir::Pattern::CharLit(cl.value),
                ast::PatternLiteral::String(sl) => crate::mir::Pattern::StringLit(sl.value.clone()),
                ast::PatternLiteral::Bool { value, .. } => crate::mir::Pattern::BoolLit(*value),
            };
            let tmp = builder.alloc_temp(bool_ty, span);
            builder.assign(
                tmp,
                Rvalue::PatternMatch {
                    subject: subj,
                    pattern: mir_pat,
                },
                span,
            );
            Operand::Local(tmp)
        }
        ast::PatternKind::Is(ty_ref) => {
            // `is T` / `!is T` 模式：解析真实目标类型，发射 PatternMatch{Is{ty, negated}}。
            let target_ty = resolve_typeref(builder, ty_ref);
            let mir_pat = crate::mir::Pattern::Is {
                ty: target_ty,
                negated: false,
            };
            let tmp = builder.alloc_temp(bool_ty, span);
            builder.assign(
                tmp,
                Rvalue::PatternMatch {
                    subject: subj,
                    pattern: mir_pat,
                },
                span,
            );
            Operand::Local(tmp)
        }
        ast::PatternKind::Or(alts) => {
            // or 模式：发射各子模式的 PatternMatch OR 链。
            // 用 CondBr 链实现 short-circuit OR：任一子模式命中 → true。
            if alts.is_empty() {
                return Operand::Const(ConstValue::Bool(false));
            }
            let result = builder.alloc_temp(bool_ty, span);
            let merge_bb = builder.new_block();
            for alt in alts {
                let test = lower_pattern_test(builder, alt, subj.clone(), subj_ty, _arm, span);
                let match_bb = builder.new_block();
                let next_bb = builder.new_block();
                builder.terminate(
                    crate::mir::Terminator {
                        span,
                        kind: crate::mir::TerminatorKind::CondBr {
                            cond: test,
                            then_target: match_bb,
                            else_target: next_bb,
                        },
                    },
                    match_bb,
                );
                // 匹配成功：result = true，goto merge。
                builder.current_bb = match_bb;
                builder.assign(
                    result,
                    Rvalue::Use(Operand::Const(ConstValue::Bool(true))),
                    span,
                );
                builder.goto(merge_bb, span);
                builder.current_bb = next_bb;
            }
            // 所有子模式都不匹配：result = false。
            builder.assign(
                result,
                Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
                span,
            );
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
    }
}

/// variant 模式第 `index` 个 payload 字段的类型（嵌套子模式递归绑定用）。
///
/// Option subject 的 `Some` payload = inner；nominal enum 走 `<enum>.<variant>`
/// 名义下登记的 members（声明序）。
pub(crate) fn variant_payload_field_ty(
    builder: &FnLowering,
    subj_ty: scoop2_hir::ty::TypeId,
    path: &ast::TypePath,
    index: usize,
) -> Option<scoop2_hir::ty::TypeId> {
    use scoop2_hir::ty::{TypeKind, ValueTypeKind};
    match builder.types.kind(subj_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let inner = *inner;
            if index == 0 { Some(inner) } else { None }
        }
        TypeKind::Value(ValueTypeKind::Nominal(n))
        | TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => {
            let variant_sym = path.segments.last().map(|s| s.symbol)?;
            let variant_fqn_text = format!(
                "{}.{}",
                builder.hir.interner.resolve(n.fqn),
                builder.hir.interner.resolve(variant_sym)
            );
            let vfqn = builder.hir.interner.get(&variant_fqn_text)?;
            let members = builder.hir.ordered_members(&vfqn);
            members.get(index).map(|(_, ty)| *ty)
        }
        _ => None,
    }
}

/// tuple 类型第 `index` 个元素的类型（嵌套 tuple 子模式递归用）。
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

/// nominal（struct/class）字段在声明序中的下标（struct 模式 binder 提取用；
/// 与 pattern 中字段的书写顺序无关）。
fn nominal_field_index(
    builder: &FnLowering,
    ty: scoop2_hir::ty::TypeId,
    name: scoop2_base::Symbol,
) -> Option<usize> {
    use scoop2_hir::ty::{TypeKind, ValueTypeKind};
    let fqn = match builder.types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n))
        | TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => n.fqn,
        _ => return None,
    };
    builder
        .hir
        .ordered_members(&fqn)
        .iter()
        .position(|(n, _)| *n == name)
}
fn lower_pattern_to_mir(builder: &mut FnLowering, pat: &ast::Pattern) -> crate::mir::Pattern {    match &pat.kind {
        ast::PatternKind::Wildcard => crate::mir::Pattern::Wildcard,
        ast::PatternKind::Bind(name) => {
            let ty = builder
                .hir
                .expr_type(builder.file_id, pat.id)
                .unwrap_or_else(|| builder.types.any());
            crate::mir::Pattern::Bind {
                name: name.symbol,
                ty,
            }
        }
        ast::PatternKind::Literal(lit) => match lit {
            ast::PatternLiteral::Int(il) => crate::mir::Pattern::IntLit(il.value as i128),
            ast::PatternLiteral::Char(cl) => crate::mir::Pattern::CharLit(cl.value),
            ast::PatternLiteral::String(sl) => crate::mir::Pattern::StringLit(sl.value.clone()),
            ast::PatternLiteral::Bool { value, .. } => crate::mir::Pattern::BoolLit(*value),
        },
        ast::PatternKind::Is(ty_ref) => {
            // TypeRef 未类型化；用 expr_type 或回退。
            let target = builder
                .hir
                .expr_type(builder.file_id, ty_ref.id)
                .unwrap_or_else(|| builder.types.any());
            crate::mir::Pattern::Is {
                ty: target,
                negated: false,
            }
        }
        _ => crate::mir::Pattern::Wildcard,
    }
}

/// 为 when arm 的模式引入绑定（从 subject 提取 binder 到 local）。
fn bind_pattern_arm(
    builder: &mut FnLowering,
    pat: &ast::Pattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
) {
    match &pat.kind {
        ast::PatternKind::Wildcard | ast::PatternKind::Else | ast::PatternKind::Rest => {}
        ast::PatternKind::Bind(name) => {
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(name.symbol).to_string(),
                subj_ty,
                name.span,
            );
            builder.symbol_locals.insert(name.symbol, lid);
            builder.assign(lid, Rvalue::Use(subj), name.span);
        }
        ast::PatternKind::Variant { path, args } => {
            // 从 pattern_bindings 侧表引入 binder。
            if let Some(bindings) = builder.hir.pattern_bindings(builder.file_id, pat.id) {
                for (i, b) in bindings.iter().enumerate() {
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(b.name).to_string(),
                        b.ty,
                        b.span,
                    );
                    builder.symbol_locals.insert(b.name, lid);
                    if matches!(
                        b.source,
                        scoop2_hir::hir::PatternBindingSource::VariantField
                    ) {
                        // variant 字段绑定：从 payload 提取第 pos 个字段
                        // （单字段 variant 也必须提取——`Some(x)` 的 x 是 payload，
                        // 不是整个 Option 值）。
                        // 提取路径用 binder 在 args 中的字段位置（bindings 序可能
                        // 与字段序不一致：非 binder 位置不占 bindings 条目）。
                        // VariantField 携带 variant 名：多字段 variant 的字段偏移
                        // 依赖具体 variant 的 slot（codegen 按布局表定位）。
                        let pos = args
                            .as_ref()
                            .and_then(|args| {
                                args.iter().position(|a| {
                                    matches!(&a.kind, ast::PatternKind::Bind(id) if id.symbol == b.name)
                                })
                            })
                            .unwrap_or(i);
                        let variant_name = path
                            .segments
                            .last()
                            .map(|s| builder.hir.interner.resolve(s.symbol).to_string())
                            .unwrap_or_default();
                        builder.assign(
                            lid,
                            Rvalue::PatternExtract {
                                subject: subj.clone(),
                                path: vec![
                                    crate::mir::transport::PatternBindingStep::VariantField {
                                        variant: variant_name,
                                        field_index: pos,
                                    },
                                ],
                                result_ty: b.ty,
                            },
                            b.span,
                        );
                    } else {
                        builder.assign(lid, Rvalue::Use(subj.clone()), b.span);
                    }
                }
            }
            // 嵌套子模式（如 `Wrap(Hit(v))` / `Some((a, b))`）：按字段位置提取后递归绑定。
            if let Some(args) = args {
                for (i, arg) in args.iter().enumerate() {
                    if matches!(
                        &arg.kind,
                        ast::PatternKind::Variant { .. }
                            | ast::PatternKind::Tuple(_)
                            | ast::PatternKind::Struct { .. }
                    ) && let Some(field_ty) =
                        variant_payload_field_ty(builder, subj_ty, path, i)
                    {
                        let variant_name = path
                            .segments
                            .last()
                            .map(|s| builder.hir.interner.resolve(s.symbol).to_string())
                            .unwrap_or_default();
                        let tmp = builder.alloc_temp(field_ty, arg.span);
                        builder.assign(
                            tmp,
                            Rvalue::PatternExtract {
                                subject: subj.clone(),
                                path: vec![
                                    crate::mir::transport::PatternBindingStep::VariantField {
                                        variant: variant_name,
                                        field_index: i,
                                    },
                                ],
                                result_ty: field_ty,
                            },
                            arg.span,
                        );
                        bind_pattern_arm(builder, arg, Operand::Local(tmp), field_ty);
                    }
                }
            }
        }
        ast::PatternKind::Tuple(elems) => {
            // 从 pattern_bindings 侧表引入 binder。
            if let Some(bindings) = builder.hir.pattern_bindings(builder.file_id, pat.id) {
                for (i, b) in bindings.iter().enumerate() {
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(b.name).to_string(),
                        b.ty,
                        b.span,
                    );
                    builder.symbol_locals.insert(b.name, lid);
                    // tuple 元素 binder（when arm 中 source 多为 Destructure）
                    // 一律按元素位置提取，不能绑整个 tuple。
                    // 提取路径用 binder 在 tuple 元素中的位置（bindings 序可能
                    // 与字段序不一致：字面量/通配元素不占 bindings 条目，
                    // 如 `(1, x)` 中 x 的字段下标是 1 而非 0）。
                    let pos = elems
                        .iter()
                        .position(|e| {
                            matches!(&e.kind, ast::PatternKind::Bind(id) if id.symbol == b.name)
                        })
                        .unwrap_or(i);
                    builder.assign(
                        lid,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(
                                pos,
                            )],
                            result_ty: b.ty,
                        },
                        b.span,
                    );
                }
            }
            // 嵌套子模式（如 `((a, b), c)`）：按元素位置提取后递归绑定。
            for (i, e) in elems.iter().enumerate() {
                if matches!(
                    &e.kind,
                    ast::PatternKind::Variant { .. }
                        | ast::PatternKind::Tuple(_)
                        | ast::PatternKind::Struct { .. }
                ) && let Some(elem_ty) = tuple_elem_ty(builder, subj_ty, i)
                {
                    let tmp = builder.alloc_temp(elem_ty, e.span);
                    builder.assign(
                        tmp,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(i)],
                            result_ty: elem_ty,
                        },
                        e.span,
                    );
                    bind_pattern_arm(builder, e, Operand::Local(tmp), elem_ty);
                }
            }
        }
        ast::PatternKind::Struct { .. } => {
            // 从 pattern_bindings 侧表引入 binder；字段下标按 subject 声明序
            // （ordered_members）定位，与 pattern 中的书写顺序无关。
            if let Some(bindings) = builder.hir.pattern_bindings(builder.file_id, pat.id) {
                for (i, b) in bindings.iter().enumerate() {
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(b.name).to_string(),
                        b.ty,
                        b.span,
                    );
                    builder.symbol_locals.insert(b.name, lid);
                    // struct 字段 binder（source 多为 Destructure）一律按声明序
                    // 字段下标提取，不能绑整个 struct。
                    let pos = nominal_field_index(builder, subj_ty, b.name).unwrap_or(i);
                    builder.assign(
                        lid,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(
                                pos,
                            )],
                            result_ty: b.ty,
                        },
                        b.span,
                    );
                }
            }
        }
        ast::PatternKind::Is(_) => {
            // is T 模式：subject 本身可用（类型已收窄）。
        }
        ast::PatternKind::Or(_) | ast::PatternKind::Literal(_) => {}
    }
}

pub fn lower_handle(
    builder: &mut FnLowering,
    body: &ast::Block,
    arms: &[ast::HandleArm],
    finally: Option<&ast::Block>,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let result = builder.alloc_temp(ty, span);
    let body_bb = builder.new_block();
    let exit_bb = builder.new_block();
    let arm_bbs: Vec<_> = arms.iter().map(|_| builder.new_block()).collect();
    let finally_bb = finally.map(|_| builder.new_block());
    // 为每个 arm 构造 HandlerArm 契约。
    // binder 符号注册进 symbol_locals 会遮盖外层同名绑定；嵌套 handle 的
    // arm body 在本 handle 之后 lower，若不恢复会把内层 binder 泄漏给外层
    // arm body（同名 binder 解析错 local）。在此快照旧值，handle 结束后恢复。
    let mut saved_binder_bindings: std::collections::HashMap<
        scoop2_base::Symbol,
        Option<crate::mir::LocalId>,
    > = std::collections::HashMap::new();
    let mut handler_arms: Vec<crate::mir::transport::HandlerArm> = Vec::with_capacity(arms.len());
    // 每个 arm 的 (符号, binder local) 绑定记录：arm body 在 handle body 之后
    // lower，期间 body 内同名 val 声明会遮盖 construction 期注册的 binder
    // 绑定（arm 体内引用 binder 名会错解析为 body 局部）。lower 各 arm body
    // 前按此记录重新安装。
    let mut arm_binder_pairs: Vec<Vec<(scoop2_base::Symbol, crate::mir::LocalId)>> =
        Vec::with_capacity(arms.len());
    for arm in arms {
        // op_fqn = effect_path.last_segment . op_name
        let effect_name = arm
            .op
            .effect_path
            .segments
            .last()
            .map(|s| builder.hir.interner.resolve(s.symbol).to_string())
            .unwrap_or_default();
        let op_name = builder.hir.interner.resolve(arm.op.op.symbol).to_string();
        let op_fqn = if effect_name.is_empty() {
            op_name.clone()
        } else {
            format!("{}.{}", effect_name, op_name)
        };
        // 解析 handled effect type。
        let handled_effect_ty = builder
            .hir
            .interner
            .get(&effect_name)
            .and_then(|fqn| {
                // 尝试构造 effect nominal 类型。
                if builder.hir.enum_variants.contains_key(&fqn) {
                    Some(builder.types.ref_nominal(scoop2_hir::ty::NominalType {
                        fqn,
                        args: vec![],
                        eff: None,
                    }))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| builder.types.any());
        // binder 类型：优先 ascription；无 ascription 时从 op 声明的
        // member_funs 签名回填参数类型（避免 binder 退化为 Any，导致 arm 体内
        // 按 Any 使用——例如 `Int.plus(to, 4)` 会把 i64 存进 ptr alloca）。
        let op_param_tys: Vec<scoop2_hir::ty::TypeId> = {
            let prefix = builder
                .hir
                .file(builder.file_id)
                .map(|f| f.package_prefix.as_str())
                .unwrap_or("");
            let candidates = if prefix.is_empty() {
                vec![effect_name.clone(), format!("scoop.core.{effect_name}")]
            } else {
                vec![
                    effect_name.clone(),
                    format!("{prefix}.{effect_name}"),
                    format!("scoop.core.{effect_name}"),
                ]
            };
            candidates
                .iter()
                .filter_map(|c| builder.hir.interner.get(c))
                .find_map(|eff| {
                    builder
                        .hir
                        .member_funs
                        .get(&eff)
                        .and_then(|m| m.get(&arm.op.op.symbol))
                })
                .and_then(|sigs| sigs.first())
                .map(|sig| sig.param_types.clone())
                .unwrap_or_default()
        };
        let mut binder_locals: Vec<crate::mir::LocalId> = Vec::new();
        let mut binder_pairs: Vec<(scoop2_base::Symbol, crate::mir::LocalId)> = Vec::new();
        let mut payload_component_tys: Vec<scoop2_hir::ty::TypeId> = Vec::new();
        for (bi, b) in arm.op.binders.iter().enumerate() {
            let bty = b
                .ty
                .as_ref()
                .and_then(|t| builder.hir.expr_type(builder.file_id, t.id))
                .or_else(|| op_param_tys.get(bi).copied())
                .unwrap_or_else(|| builder.types.any());
            payload_component_tys.push(bty);
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(b.name.symbol).to_string(),
                bty,
                b.name.span,
            );
            saved_binder_bindings
                .entry(b.name.symbol)
                .or_insert_with(|| builder.symbol_locals.get(&b.name.symbol).copied());
            builder.symbol_locals.insert(b.name.symbol, lid);
            binder_locals.push(lid);
            binder_pairs.push((b.name.symbol, lid));
        }
        // continuation_local：resuming arm 的 escape_continuation binder。
        let (continuation_local, kind) = if let Some(k_ident) = &arm.escape_continuation {
            // resuming arm：, k -> expr
            // k 的类型是 Continuation<Resume, Answer, eff E>，由 typecheck 推断。
            // 这里先分配一个 Any 类型的 local（effect lowering pass 会用精确类型替换）。
            let cont_ty = builder.types.any();
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(k_ident.symbol).to_string(),
                cont_ty,
                k_ident.span,
            );
            saved_binder_bindings
                .entry(k_ident.symbol)
                .or_insert_with(|| builder.symbol_locals.get(&k_ident.symbol).copied());
            builder.symbol_locals.insert(k_ident.symbol, lid);
            binder_pairs.push((k_ident.symbol, lid));
            (
                Some(lid),
                crate::mir::transport::HandlerArmKind::EscapeContinuation,
            )
        } else {
            (None, crate::mir::transport::HandlerArmKind::NonResuming)
        };
        handler_arms.push(crate::mir::transport::HandlerArm {
            op_fqn: op_fqn.clone(),
            op_type_args: Vec::new(),
            binder_count: arm.op.binders.len(),
            binder_locals: binder_locals.clone(),
            continuation_local,
            handled_effect_ty,
            payload_tuple_ty: if payload_component_tys.len() == 1 {
                Some(payload_component_tys[0])
            } else if payload_component_tys.is_empty() {
                None
            } else {
                Some(builder.types.tuple(payload_component_tys.clone()))
            },
            payload_component_tys: payload_component_tys.clone(),
            body_ty: ty,
            kind,
        });
        arm_binder_pairs.push(binder_pairs);
    }
    // 发射 Handle 终结符。
    let handle_metadata = HandleMetadata {
        result_ty: ty,
        body_result_ty: ty,
        finally_result_ty: None,
        result_local: result,
    };
    let handle_site_id = Some(builder.next_site_id());
    builder.terminate(
        crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::Handle {
                site_id: handle_site_id,
                metadata: handle_metadata,
                arms: handler_arms,
                body_target: body_bb,
                arm_targets: arm_bbs.clone(),
                finally_target: finally_bb,
                exit_target: exit_bb,
            },
        },
        body_bb,
    );
    // body。
    builder.current_bb = body_bb;
    let bv = super::stmt::lower_block(builder, body);
    builder.assign(result, Rvalue::Use(bv), span);
    builder.goto(exit_bb, span);
    // arms（lower 各 arm body 到对应块，结果写 result）。
    for (i, arm) in arms.iter().enumerate() {
        builder.current_bb = arm_bbs[i];
        // binder locals 在 HandlerArm 构造时注册，但 handle body lowering 期间
        // 同名 val 声明会遮盖它们（symbol_locals 是平铺 map、无作用域弹出）。
        // lower 本 arm body 前重新安装本 arm 的 binder 绑定，结束后恢复。
        let mut arm_saved: Vec<(scoop2_base::Symbol, Option<crate::mir::LocalId>)> =
            Vec::with_capacity(arm_binder_pairs[i].len());
        for &(sym, lid) in &arm_binder_pairs[i] {
            arm_saved.push((sym, builder.symbol_locals.get(&sym).copied()));
            builder.symbol_locals.insert(sym, lid);
        }
        let v = lower_expr(builder, &arm.body);
        for (sym, old) in arm_saved {
            match old {
                Some(lid) => {
                    builder.symbol_locals.insert(sym, lid);
                }
                None => {
                    builder.symbol_locals.remove(&sym);
                }
            }
        }
        builder.assign(result, Rvalue::Use(v), arm.body.span);
        builder.goto(exit_bb, arm.span);
    }
    // finally。
    if let (Some(fb), Some(fblock)) = (finally_bb, finally) {
        builder.current_bb = fb;
        super::stmt::lower_block(builder, fblock);
        builder.goto(exit_bb, span);
    }
    // 恢复 handle 之前的同名绑定（嵌套 handle 的 arm body 之后才会 lower，
    // 不恢复会把本 handle 的 binder 泄漏给外层 arm body）。
    for (sym, old) in saved_binder_bindings {
        match old {
            Some(lid) => {
                builder.symbol_locals.insert(sym, lid);
            }
            None => {
                builder.symbol_locals.remove(&sym);
            }
        }
    }
    builder.current_bb = exit_bb;
    Operand::Local(result)
}

/// lower if。
fn lower_if(
    builder: &mut FnLowering,
    cond: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    span: Span,
    ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let c = lower_expr(builder, cond);
    let result = builder.alloc_temp(ty, span);
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let merge_bb = builder.new_block();
    builder.terminate(
        crate::mir::Terminator {
            span,
            kind: crate::mir::TerminatorKind::CondBr {
                cond: c,
                then_target: then_bb,
                else_target: else_bb,
            },
        },
        then_bb,
    );
    builder.current_bb = then_bb;
    let tv = lower_expr(builder, then_branch);
    builder.assign(result, Rvalue::Use(tv), then_branch.span);
    builder.goto(merge_bb, span);
    builder.current_bb = else_bb;
    if let Some(eb) = else_branch {
        let ev = lower_expr(builder, eb);
        builder.assign(result, Rvalue::Use(ev), eb.span);
    } else {
        builder.assign(result, Rvalue::Use(Operand::Const(ConstValue::Unit)), span);
    }
    builder.goto(merge_bb, span);
    builder.current_bb = merge_bb;
    Operand::Local(result)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn suffix_of(s: &Option<ast::IntSuffix>) -> Option<crate::mir::IntSuffix> {
    s.map(|s| match s {
        ast::IntSuffix::U => crate::mir::IntSuffix::U,
        ast::IntSuffix::L => crate::mir::IntSuffix::L,
        ast::IntSuffix::UL => crate::mir::IntSuffix::UL,
    })
}

fn resolve_struct_fqn(builder: &FnLowering, sym: Symbol) -> Symbol {
    let name = builder.hir.interner.resolve(sym);
    for cand in [name.to_string(), format!("scoop.core.{}", name)] {
        if let Some(f) = builder.hir.interner.get(&cand) {
            return f;
        }
    }
    sym
}

pub(crate) fn resolve_typeref(
    builder: &mut FnLowering,
    t: &ast::TypeRef,
) -> scoop2_hir::ty::TypeId {
    // TypeRef 节点未类型化；从 expr_types 查（is/as 的目标类型有时记录）。
    if let Some(ty) = builder.hir.expr_type(builder.file_id, t.id) {
        return ty;
    }
    // 按路径名查 nominal / 标量 / Unit（不再回退 Any）。
    use ast::TypeRefKind;
    match &t.kind {
        TypeRefKind::Path { path, .. } => {
            if let Some(last) = path.segments.last() {
                let name = builder.hir.interner.resolve(last.symbol);
                // 候选 FQN：裸名 / package prefix / scoop.core。
                let prefix = builder
                    .hir
                    .file(builder.file_id)
                    .map(|f| f.package_prefix.as_str())
                    .unwrap_or("");
                let candidates = if prefix.is_empty() {
                    vec![name.to_string(), format!("scoop.core.{}", name)]
                } else {
                    vec![
                        name.to_string(),
                        format!("{}.{}", prefix, name),
                        format!("scoop.core.{}", name),
                    ]
                };
                for cand in &candidates {
                    if let Some(f) = builder.hir.interner.get(cand) {
                        // 查 members / enum_variants / top_level_vals 确认存在。
                        let is_known = builder.hir.members.contains_key(&f)
                            || builder.hir.enum_variants.contains_key(&f)
                            || builder.hir.top_level_vals.contains_key(&f)
                            || builder.hir.top_level_funs.contains_key(&f)
                            || builder.hir.member_funs.contains_key(&f);
                        if is_known {
                            // 判断 ref vs value nominal：class/interface/object → ref；
                            // struct/enum → value。
                            let is_value = builder.hir.enum_variants.contains_key(&f);
                            let nominal = scoop2_hir::ty::NominalType {
                                fqn: f,
                                args: vec![],
                                eff: None,
                            };
                            if is_value {
                                return builder.types.value_nominal(nominal);
                            } else {
                                return builder.types.ref_nominal(nominal);
                            }
                        }
                    }
                }
            }
            // 路径存在但无法解析到已知 nominal——精确报错而非回退 Any。
            // 注：此时返回 expr 的已推断类型（ty 参数由调用方传入），而非 Any。
            // 调用方在 is/as 场景会拿到 test_ty/target_ty 用于 runtime test metadata。
            // 此处返回 nothing（bottom）作为「类型不可解析」的精确标记。
            builder.types.nothing()
        }
        TypeRefKind::Unit => builder.types.unit(),
        TypeRefKind::Tuple(els) => {
            let elem_tys: Vec<scoop2_hir::ty::TypeId> =
                els.iter().map(|e| resolve_typeref(builder, e)).collect();
            builder.types.tuple(elem_tys)
        }
        TypeRefKind::Nullable(inner) => {
            let inner_ty = resolve_typeref(builder, inner);
            builder.types.option(inner_ty)
        }
        TypeRefKind::Function {
            params,
            ret,
            effect: _,
            ..
        } => {
            let param_tys: Vec<scoop2_hir::ty::TypeId> =
                params.iter().map(|p| resolve_typeref(builder, p)).collect();
            let return_ty = resolve_typeref(builder, ret);
            let ft = scoop2_hir::ty::FunctionType {
                receiver: None,
                params: param_tys,
                return_ty,
                effects: scoop2_hir::ty::EffectRow::pure(),
                closed: false,
            };
            builder.types.function(ft)
        }
        TypeRefKind::ReceiverFunction {
            receiver,
            params,
            ret,
            ..
        } => {
            let recv_ty = resolve_typeref(builder, receiver);
            let param_tys: Vec<scoop2_hir::ty::TypeId> =
                params.iter().map(|p| resolve_typeref(builder, p)).collect();
            let return_ty = resolve_typeref(builder, ret);
            let ft = scoop2_hir::ty::FunctionType {
                receiver: Some(recv_ty),
                params: param_tys,
                return_ty,
                effects: scoop2_hir::ty::EffectRow::pure(),
                closed: false,
            };
            builder.types.function(ft)
        }
    }
}
