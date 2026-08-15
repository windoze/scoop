//! HIR body 树 → MIR lowering（PLAN.md M2-5：MIR 输入翻转的树消费路径）。
//!
//! 与 AST 路径（`lower/`）**双轨并行**：本模块从 `FnTree`（决议内联的封闭
//! 词汇表）构造 MIR，复用 [`FnLowering`] 的全部机器（temp 分配 / transport /
//! CallKind 构造 / 块切分）——只有「遍历」不同。按函数迁移：[`unsupported_construct`]
//! 判定词汇覆盖，双路径 oracle 逐函数比对 dump 字节，全语料一致后切换默认并
//! 删除 AST 路径（C1/C9）。
//!
//! MIR dump 的 local/block 标签是索引派生 hash（`hash("l{i}")`）——字节一致
//! 只需分配序一致，因此本模块严格镜像 AST 路径的遍历/分配顺序。
//!
//! **当前支持集（第一切片：直线代码）**：字面量 / LocalRef / TopLevelValRef /
//! Call（TopLevel、Method-direct、LocalValue、FunValue）/ Member（字段、元组
//! 下标）/ Block / Tuple / LogicalAnd/Or / LocalVal(Name) / Assign(Local、
//! MemberField) / Return。Virtual/Interface 分派、Ctor/Variant、When/If/While/
//! Handle 等的控制流与元数据复刻在后续切片（见 `unsupported_construct` 清单）。

use scoop2_base::Span;
use scoop2_hir::hir::tree::{
    BlockId, ExprId, FnTree, HandleTreeArm, TreeBody, TreeCallee, TreeExprKind, TreeMember,
    TreePattern, TreeStmt, WhenTreeArm,
};

use crate::mir::lower::builder::FnLowering;
use crate::mir::transport::AggregateTransportKind;
use crate::mir::{Body, ConstValue, Operand, Rvalue, Terminator, TerminatorKind};

/// 树是否完全被本模块支持（不支持返回构造名——用于迁移统计）。
pub fn unsupported_construct(tree: &FnTree) -> Option<&'static str> {
    for e in &tree.body.exprs {
        match &e.kind {
            TreeExprKind::Lit(_)
            | TreeExprKind::LocalRef(_)
            | TreeExprKind::TopLevelValRef { .. }
            | TreeExprKind::Call { .. }
            | TreeExprKind::Member { .. }
            | TreeExprKind::SafeMember { .. }
            | TreeExprKind::Block(_)
            | TreeExprKind::Tuple(_)
            | TreeExprKind::LogicalAnd { .. }
            | TreeExprKind::LogicalOr { .. }
            | TreeExprKind::NotNullAssert { .. }
            | TreeExprKind::If { .. }
            | TreeExprKind::While { .. }
            | TreeExprKind::StructLit { .. }
            | TreeExprKind::InterpolatedString { .. }
            | TreeExprKind::ArrayLit(_)
            | TreeExprKind::Cast { .. }
            | TreeExprKind::TypeCheck { .. }
            | TreeExprKind::WithUpdate { .. }
            | TreeExprKind::When { .. }
            | TreeExprKind::Lambda { .. }
            | TreeExprKind::Handle { .. }
            | TreeExprKind::UnresolvedName { .. }
            | TreeExprKind::UnresolvedCall { .. }
            | TreeExprKind::BoolNot { .. } => {}
        }
    }
    for e in &tree.body.exprs {
        if let TreeExprKind::Call { callee, .. } = &e.kind {
            match callee {
                TreeCallee::TopLevel { .. }
                | TreeCallee::LocalValue { .. }
                | TreeCallee::FunValue { .. } => {}
                TreeCallee::Method { .. } => {}

                TreeCallee::EffectOp { .. } => {}

                TreeCallee::InitCall { .. }
                | TreeCallee::Ctor { .. }
                | TreeCallee::Variant { .. } => {}
            }
        }
    }
    for s in &tree.body.stmts {
        match s {
            TreeStmt::Expr(_)
            | TreeStmt::LocalVal { .. }
            | TreeStmt::Assign { .. }
            | TreeStmt::Return(_) => {}
            TreeStmt::Destructure { .. } => {}

            TreeStmt::Break | TreeStmt::Continue => {}
        }
    }
    None
}

/// 从树 lower 函数体（调用方已完成参数 local 分配并注册符号表，随后 `finish()`）。
/// 语义镜像 `stmt::lower_block` + `lower_fun_body` 的尾值处理。
pub fn lower_tree_fn_body(builder: &mut FnLowering, tree: &FnTree) {
    let root = tree.body.root.expect("树必有根块");
    let root_span = tree.body.blocks[root.0 as usize].span;
    let tail = lower_tree_block(builder, &tree.body, root);
    let tail_is_unit = matches!(tail, Operand::Const(ConstValue::Unit));
    let bb = builder.current_bb;
    if !tail_is_unit
        && matches!(
            builder.body.blocks[bb.0 as usize].terminator.kind,
            TerminatorKind::Unreachable
        )
    {
        builder.terminate(
            Terminator {
                span: root_span,
                kind: TerminatorKind::Return { value: Some(tail) },
            },
            bb,
        );
    }
}

/// lower 树块：语句序列 + 尾值（镜像 stmt::lower_block 的顺序）。
fn lower_tree_block(builder: &mut FnLowering, body: &TreeBody, block: BlockId) -> Operand {
    let blk = &body.blocks[block.0 as usize];
    let mut last_val = Operand::Const(ConstValue::Unit);
    for &sid in &blk.stmts {
        lower_tree_stmt(builder, body, sid);
    }
    let _ = &mut last_val;
    if let Some(tail) = blk.tail {
        last_val = lower_tree_expr(builder, body, tail);
    }
    last_val
}

fn lower_tree_stmt(builder: &mut FnLowering, body: &TreeBody, sid: scoop2_hir::hir::tree::StmtId) {
    use scoop2_hir::hir::tree::TreePlace;
    let stmt = &body.stmts[sid.0 as usize];
    match stmt {
        TreeStmt::Expr(e) => {
            let _ = lower_tree_expr(builder, body, *e);
        }
        TreeStmt::LocalVal { local, init } => {
            let v = lower_tree_expr(builder, body, *init);
            let decl = &body.locals[local.0 as usize];
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(decl.name).to_string(),
                decl.ty,
                decl.span,
            );
            builder.symbol_locals.insert(decl.name, lid);
            builder.assign(lid, Rvalue::Use(v), decl.span);
        }
        TreeStmt::Assign { place, value } => {
            let v = lower_tree_expr(builder, body, *value);
            match place {
                TreePlace::Local(local) => {
                    let decl = &body.locals[local.0 as usize];
                    if let Some(&lid) = builder.symbol_locals.get(&decl.name) {
                        builder.assign(lid, Rvalue::Use(v), decl.span);
                    }
                }
                TreePlace::MemberField {
                    recv,
                    owner_fqn,
                    name,
                } => {
                    let recv_op = lower_tree_expr(builder, body, *recv);
                    let val_ty = operand_ty_of(builder, &v);
                    let owner_text = builder.hir.interner.resolve(*owner_fqn).to_string();
                    let name_text = builder.hir.interner.resolve(*name).to_string();
                    let _ = owner_text;
                    let recv_ty_v = recv_ty_of(builder, &recv_op);
                    builder.push_stmt(crate::mir::Statement {
                        span: decl_span_of(body, *value),
                        kind: crate::mir::StatementKind::StoreMember {
                            receiver: recv_op,
                            member: crate::mir::transport::MemberAccessMetadata {
                                name: name_text,
                                receiver_ty: recv_ty_v,
                                resolved: None,
                                hidden_effects: scoop2_hir::ty::EffectRow::pure(),
                            },
                            value: v,
                            value_ty: val_ty,
                            continuation_route:
                                crate::mir::transport::StoredContinuationRoutePublication::None,
                        },
                    });
                }
                TreePlace::TopLevelVar { fqn } => {
                    // 镜像 lower_assign 的 TopLevelVar 分支：StoreTopLevelVar。
                    let val_ty = operand_ty_of(builder, &v);
                    builder.push_stmt(crate::mir::Statement {
                        span: decl_span_of(body, *value),
                        kind: crate::mir::StatementKind::StoreTopLevelVar {
                            fqn: *fqn,
                            value: v,
                            value_ty: val_ty,
                        },
                    });
                }
            }
        }
        TreeStmt::Return(value) => {
            let v = value
                .map(|e| lower_tree_expr(builder, body, e))
                .unwrap_or_else(|| Operand::Const(ConstValue::Unit));
            builder.terminate(
                Terminator {
                    span: Span::default(),
                    kind: TerminatorKind::Return { value: Some(v) },
                },
                builder.current_bb,
            );
            let dead = builder.new_block();
            builder.current_bb = dead;
        }
        TreeStmt::Break => {
            if let Some(loop_ctx) = builder.loop_stack.last().copied() {
                builder.goto(loop_ctx.break_target, Span::default());
                let dead = builder.new_block();
                builder.current_bb = dead;
            }
        }
        TreeStmt::Continue => {
            if let Some(loop_ctx) = builder.loop_stack.last().copied() {
                builder.goto(loop_ctx.continue_target, Span::default());
                let dead = builder.new_block();
                builder.current_bb = dead;
            }
        }
        TreeStmt::Destructure { pat, init, mutable } => {
            // 镜像 bind_pattern：模式解构绑定（解构语义）
            let v = lower_tree_expr(builder, body, *init);
            let init_ty = operand_ty_of(builder, &v);
            bind_tree_pattern(builder, body, pat, v, init_ty, *mutable);
        }
    }
}

/// 树表达式 → Operand（镜像 expr::lower_expr 的分配序）。
fn lower_tree_expr(builder: &mut FnLowering, body: &TreeBody, eid: ExprId) -> Operand {
    let node = &body.exprs[eid.0 as usize];
    let ty = node.ty;
    let span = node.span;
    match &node.kind {
        TreeExprKind::Lit(lit) => lit_operand(lit, ty, builder),
        TreeExprKind::LocalRef(local) => {
            let decl = &body.locals[local.0 as usize];
            match builder.symbol_locals.get(&decl.name).copied() {
                Some(lid) => Operand::Local(lid),
                None => {
                    let name = builder.hir.interner.resolve(decl.name).to_string();
                    let lid = builder.alloc_named(name, decl.ty, decl.span);
                    builder.symbol_locals.insert(decl.name, lid);
                    Operand::Local(lid)
                }
            }
        }
        TreeExprKind::TopLevelValRef { fqn } => {
            // 镜像 AST lower_ident 的 top_level_vals 回退分支：fqn 文本用**简名**
            //（AST 的 value_ref 查询是 u32::MAX 哨兵节点，恒走回退——简名 quirk
            // 字节一致保留；C1 清理时与 AST 路径一并统一）。表 miss（顶层 val
            // 模式绑定名等未登记形态）→ UnresolvedName 回退（AST 同）。
            let simple = builder
                .hir
                .interner
                .resolve(*fqn)
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_string();
            let in_top_level_vals = builder
                .hir
                .interner
                .get(&simple)
                .is_some_and(|sym| builder.hir.top_level_vals.contains_key(&sym));
            let tmp = builder.alloc_temp(ty, span);
            if in_top_level_vals {
                let tl = crate::mir::TopLevelRef {
                    fqn: simple.clone(),
                    hidden_effects: scoop2_hir::ty::EffectRow::pure(),
                    stable_template_key: Some(crate::mir::stable_id::make_stable_template_key(
                        crate::mir::stable_id::StableHashScope::Dump,
                        &simple,
                        &[],
                        "",
                    )),
                    stable_instance_key: None,
                    generic_type_args: vec![],
                    generic_eff_args: vec![],
                };
                builder.assign(tmp, Rvalue::TopLevelRef(tl), span);
            } else {
                builder.assign(tmp, Rvalue::UnresolvedName { name: simple }, span);
            }
            Operand::Local(tmp)
        }
        TreeExprKind::Call {
            callee,
            args,
            arg_names,
            arg_spread,
        } => lower_tree_call(builder, body, callee, args, arg_names, arg_spread, ty, span),
        TreeExprKind::Member { recv, member } => {
            let recv_op = lower_tree_expr(builder, body, *recv);
            lower_tree_member(builder, recv_op, member, ty, span)
        }
        TreeExprKind::SafeMember { recv, member } => {
            // 镜像 lower_safe_member_access：if null then null else recv.member。
            let recv_op = lower_tree_expr(builder, body, *recv);
            let result = builder.alloc_temp(ty, span);
            let then_bb = builder.new_block();
            let else_bb = builder.new_block();
            let merge_bb = builder.new_block();
            builder.terminate(
                Terminator {
                    span,
                    kind: TerminatorKind::CondBr {
                        cond: recv_op.clone(),
                        then_target: then_bb,
                        else_target: else_bb,
                    },
                },
                then_bb,
            );
            builder.current_bb = then_bb;
            let member_val = lower_tree_member(builder, recv_op, member, ty, span);
            builder.assign(result, Rvalue::Use(member_val), span);
            builder.goto(merge_bb, span);
            builder.current_bb = else_bb;
            builder.assign(result, Rvalue::Use(Operand::Const(ConstValue::Null)), span);
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        TreeExprKind::StructLit { fqn, fields } => {
            // 镜像 StructLit lower：字段按源序 lower（值 + 类型），temp 后分配。
            let mut mir_fields = Vec::with_capacity(fields.len());
            for &(name, e) in fields {
                let v = lower_tree_expr(builder, body, e);
                let vty = operand_ty_of(builder, &v);
                mir_fields.push(crate::mir::StructLitField {
                    name,
                    value: v,
                    value_ty: vty,
                });
            }
            let type_fqn = resolve_tree_struct_fqn(builder, fqn);
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
        TreeExprKind::InterpolatedString { parts } => {
            // 镜像 lower_interpolated：parts lower 后 temp 分配。
            let mut mir_parts = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    scoop2_hir::hir::tree::InterpPart::Lit(s) => {
                        mir_parts.push(crate::mir::InterpolatedPart::Lit(s.clone()))
                    }
                    scoop2_hir::hir::tree::InterpPart::Expr(e) => {
                        let v = lower_tree_expr(builder, body, *e);
                        mir_parts.push(crate::mir::InterpolatedPart::Expr(v));
                    }
                }
            }
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(tmp, Rvalue::InterpolatedString { parts: mir_parts }, span);
            Operand::Local(tmp)
        }
        TreeExprKind::ArrayLit(els) => {
            // 镜像 lower_array_lit：Nothing 类型时用 Array 引用 temp。
            let ops: Vec<Operand> = els
                .iter()
                .map(|&e| lower_tree_expr(builder, body, e))
                .collect();
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
        TreeExprKind::Cast {
            expr: inner,
            target,
            nullable,
        } => {
            // 镜像 Cast lower：As 结果 = 表达式类型；AsSafe = 目标类型。
            let v = lower_tree_expr(builder, body, *inner);
            let operand_ty_id = operand_ty_of(builder, &v);
            let result = if *nullable { *target } else { ty };
            let tmp = builder.alloc_temp(result, span);
            let type_fqn_str = tree_nominal_fqn_of(builder, *target);
            let metadata = crate::mir::transport::RuntimeCastMetadata {
                test: crate::mir::transport::RuntimeTypeTestMetadata {
                    source_ty: operand_ty_id,
                    target_ty: *target,
                    descriptor: crate::mir::transport::RuntimeTypeDescriptorKey {
                        ty: *target,
                        kind: crate::mir::transport::RuntimeTypeDescriptorKind::Nominal {
                            fqn: type_fqn_str,
                            kind: None,
                        },
                    },
                    static_fold: crate::mir::transport::RuntimeTypeStaticFold::Dynamic,
                    parameterized: crate::mir::transport::RuntimeTypeParameterizedMatch::None,
                },
                failure: crate::mir::transport::RuntimeCastFailure::ReturnNone,
                result: crate::mir::transport::RuntimeCastResult::Target { ty: *target },
            };
            let site_id = Some(builder.next_site_id());
            let op = if *nullable {
                crate::mir::CastOp::AsSafe
            } else {
                crate::mir::CastOp::As
            };
            builder.assign(
                tmp,
                Rvalue::Cast {
                    site_id,
                    value: v,
                    op,
                    metadata,
                },
                span,
            );
            Operand::Local(tmp)
        }
        TreeExprKind::TypeCheck {
            expr: inner,
            target,
            ..
        } => {
            // 镜像 TypeCheck lower（`is`/`!is` 的差异由 verify/codegen 处理）。
            let v = lower_tree_expr(builder, body, *inner);
            let operand_ty_id = operand_ty_of(builder, &v);
            let bool_ty = builder.types.bool();
            let tmp = builder.alloc_temp(bool_ty, span);
            let type_fqn_str = tree_nominal_fqn_of(builder, *target);
            let metadata = crate::mir::transport::RuntimeTypeTestMetadata {
                source_ty: operand_ty_id,
                target_ty: *target,
                descriptor: crate::mir::transport::RuntimeTypeDescriptorKey {
                    ty: *target,
                    kind: crate::mir::transport::RuntimeTypeDescriptorKind::Nominal {
                        fqn: type_fqn_str,
                        kind: None,
                    },
                },
                static_fold: crate::mir::transport::RuntimeTypeStaticFold::Dynamic,
                parameterized: crate::mir::transport::RuntimeTypeParameterizedMatch::None,
            };
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
        TreeExprKind::WithUpdate { base, updates } => {
            // 镜像 lower_with_update：base → 各 update 值 → temp。
            let base_op = lower_tree_expr(builder, body, *base);
            let mut mir_updates = Vec::with_capacity(updates.len());
            for (path, e) in updates {
                let v = lower_tree_expr(builder, body, *e);
                let value_ty = operand_ty_of(builder, &v);
                let segs: Vec<crate::mir::WithUpdateSegment> = path
                    .segments
                    .iter()
                    .map(|s| match s {
                        scoop2_hir::hir::tree::TreeFieldSeg::Named(n) => {
                            crate::mir::WithUpdateSegment::Named(*n)
                        }
                        scoop2_hir::hir::tree::TreeFieldSeg::TupleIndex(i) => {
                            crate::mir::WithUpdateSegment::TupleIndex(*i as u128)
                        }
                    })
                    .collect();
                mir_updates.push(crate::mir::WithUpdateField {
                    path: segs,
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
        TreeExprKind::Block(b) => lower_tree_block(builder, body, *b),
        TreeExprKind::Tuple(els) => {
            let ops: Vec<Operand> = els
                .iter()
                .map(|&e| lower_tree_expr(builder, body, e))
                .collect();
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
        TreeExprKind::If { cond, then, else_ } => {
            // 镜像 lower_if：CondBr（target=then_bb 形态）+ 双臂赋值 + merge。
            let c = lower_tree_expr(builder, body, *cond);
            let result = builder.alloc_temp(ty, span);
            let then_bb = builder.new_block();
            let else_bb = builder.new_block();
            let merge_bb = builder.new_block();
            builder.terminate(
                Terminator {
                    span,
                    kind: TerminatorKind::CondBr {
                        cond: c,
                        then_target: then_bb,
                        else_target: else_bb,
                    },
                },
                then_bb,
            );
            builder.current_bb = then_bb;
            let tv = lower_tree_expr(builder, body, *then);
            let then_span = body.exprs[then.0 as usize].span;
            builder.assign(result, Rvalue::Use(tv), then_span);
            builder.goto(merge_bb, span);
            builder.current_bb = else_bb;
            match else_ {
                Some(eb) => {
                    let ev = lower_tree_expr(builder, body, *eb);
                    let else_span = body.exprs[eb.0 as usize].span;
                    builder.assign(result, Rvalue::Use(ev), else_span);
                }
                None => {
                    builder.assign(result, Rvalue::Use(Operand::Const(ConstValue::Unit)), span);
                }
            }
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        TreeExprKind::LogicalAnd { lhs, rhs } => {
            lower_logical(builder, body, *lhs, *rhs, ty, span, true)
        }
        TreeExprKind::LogicalOr { lhs, rhs } => {
            lower_logical(builder, body, *lhs, *rhs, ty, span, false)
        }
        TreeExprKind::While {
            cond,
            body: loop_body,
        } => {
            // 镜像 lower_while：cond 块 → body（loop_stack 注册）→ 回 cond；exit。
            let cond_bb = builder.new_block();
            let body_bb = builder.new_block();
            let exit_bb = builder.new_block();
            builder.goto(cond_bb, span);
            builder.current_bb = cond_bb;
            let c = lower_tree_expr(builder, body, *cond);
            let cond_span = body.exprs[cond.0 as usize].span;
            builder.terminate(
                Terminator {
                    span: cond_span,
                    kind: TerminatorKind::CondBr {
                        cond: c,
                        then_target: body_bb,
                        else_target: exit_bb,
                    },
                },
                body_bb,
            );
            builder
                .loop_stack
                .push(crate::mir::lower::builder::LoopContext {
                    break_target: exit_bb,
                    continue_target: body_bb,
                });
            lower_tree_block(builder, body, *loop_body);
            builder.loop_stack.pop();
            builder.goto(cond_bb, span);
            builder.current_bb = exit_bb;
            Operand::Const(ConstValue::Unit)
        }
        TreeExprKind::UnresolvedCall { args } => {
            // 镜像 lower_via_call_resolution 的无决议回退：lower 实参（副作用）
            // 后返回 Unit temp。
            for &a in args {
                let _ = lower_tree_expr(builder, body, a);
            }
            let unit = builder.types.unit();
            Operand::Local(builder.alloc_temp(unit, span))
        }
        TreeExprKind::UnresolvedName { name } => {
            // 镜像 lower_ident 的未解析回退：Unit-temp + UnresolvedName 赋值。
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(tmp, Rvalue::UnresolvedName { name: name.clone() }, span);
            Operand::Local(tmp)
        }
        TreeExprKind::BoolNot { expr: inner } => {
            // 镜像 lower_unary 的 Not 分支：`v == false` 分派调用形态
            //（owner 文本来自默认符号——AST quirk 字节一致保留）。
            let v = lower_tree_expr(builder, body, *inner);
            let inner_ty = operand_ty_of(builder, &v);
            let bool_ty = builder.types.bool();
            let equals_sym = builder.hir.interner.get("equals").unwrap_or_default();
            let false_op = Operand::Const(ConstValue::Bool(false));
            let tmp = builder.alloc_temp(bool_ty, span);
            let owner_str = builder
                .hir
                .interner
                .resolve(scoop2_base::Symbol::default())
                .to_string();
            let method_str = builder.hir.interner.resolve(equals_sym).to_string();
            let member_fqn = format!("{}.{}", owner_str, method_str);
            let overload_sig = String::new();
            let stk = crate::mir::stable_id::make_stable_template_key(
                crate::mir::stable_id::StableHashScope::Dump,
                &member_fqn,
                &[],
                &overload_sig,
            );
            let dispatch = crate::mir::transport::DispatchMetadata {
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
                crate::mir::lower::stmt::resolve_owner_fqn_from_operand(builder, &v),
                v.clone(),
                dispatch,
            );
            builder.assign(
                tmp,
                Rvalue::Call {
                    site_id,
                    kind,
                    args: vec![crate::mir::CallArg {
                        name: None,
                        is_spread: false,
                        value: false_op,
                        value_ty: bool_ty,
                    }],
                    transport,
                },
                span,
            );
            Operand::Local(tmp)
        }
        TreeExprKind::NotNullAssert { expr: inner } => {
            // 镜像 lower_not_null_assert：CondBr + else panic 路径。
            let v = lower_tree_expr(builder, body, *inner);
            let result = builder.alloc_temp(ty, span);
            let then_bb = builder.new_block();
            let else_bb = builder.new_block();
            let merge_bb = builder.new_block();
            builder.terminate(
                Terminator {
                    span,
                    kind: TerminatorKind::CondBr {
                        cond: v.clone(),
                        then_target: then_bb,
                        else_target: else_bb,
                    },
                },
                then_bb,
            );
            builder.current_bb = then_bb;
            builder.assign(result, Rvalue::Use(v), span);
            builder.goto(merge_bb, span);
            builder.current_bb = else_bb;
            builder.push_stmt(crate::mir::Statement {
                span,
                kind: crate::mir::StatementKind::Panic {
                    message: "NotNullAssert 失败（值为 null）".to_string(),
                },
            });
            builder.goto(merge_bb, span);
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        TreeExprKind::When { subject, arms } => {
            // 镜像 lower_when：逐 arm 测试模式 + guard，命中则赋值 result 并 goto merge。
            lower_tree_when(builder, body, *subject, arms, ty, span)
        }
        TreeExprKind::Lambda {
            params,
            body: lambda_body,
            implicit_it,
        } => {
            // 镜像 lower_lambda：生成 env tuple + 嵌套 Item::Fun
            lower_tree_lambda(builder, body, params, lambda_body, *implicit_it, ty, span)
        }
        TreeExprKind::Handle {
            body: hbody,
            arms,
            finally_,
        } => {
            // 镜像 lower_handle：Handle 终结符 + body/arm/finally 块 + binder 作用域管理。
            lower_tree_handle(builder, body, *hbody, arms, *finally_, ty, span)
        }
        _ => unsupported!("本切片支持集外的表达式构造"),
    }
}

/// 字面量 → Const operand（镜像 expr.rs 字面量臂，含 Float32 期望物化）。
fn lit_operand(
    lit: &scoop2_hir::hir::tree::Lit,
    ty: scoop2_hir::ty::TypeId,
    builder: &FnLowering,
) -> Operand {
    use scoop2_hir::hir::tree::Lit;
    match lit {
        Lit::Unit => Operand::Const(ConstValue::Unit),
        Lit::Bool(b) => Operand::Const(ConstValue::Bool(*b)),
        Lit::Int(v, suffix) => Operand::Const(ConstValue::Int(
            *v,
            crate::mir::lower::expr::suffix_of(suffix),
        )),
        Lit::Float(v) => {
            let f32_expected = matches!(
                builder.types.kind(ty),
                scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Float32)
            );
            let suffix = if f32_expected {
                Some(crate::mir::FloatSuffix::F32)
            } else {
                None
            };
            Operand::Const(ConstValue::Float(*v, suffix))
        }
        Lit::Char(c) => Operand::Const(ConstValue::Char(*c)),
        Lit::Str(s) => Operand::Const(ConstValue::String(s.clone())),
    }
}

/// 成员读取（镜像 MemberAccess lower：字段 / 元组下标）。
fn lower_tree_member(
    builder: &mut FnLowering,
    recv: Operand,
    member: &TreeMember,
    ty: scoop2_hir::ty::TypeId,
    span: Span,
) -> Operand {
    let recv_ty = operand_ty_of(builder, &recv);
    match member {
        TreeMember::Field { owner_fqn, name } => {
            let metadata = crate::mir::transport::MemberAccessMetadata {
                name: builder.hir.interner.resolve(*name).to_string(),
                receiver_ty: recv_ty,
                resolved: None,
                hidden_effects: scoop2_hir::ty::EffectRow::pure(),
            };
            let site = builder.next_site_id();
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(
                tmp,
                Rvalue::MemberAccess {
                    site_id: Some(site),
                    receiver: recv,
                    member: metadata,
                },
                span,
            );
            Operand::Local(tmp)
        }
        TreeMember::TupleIndex { index } => {
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(
                tmp,
                Rvalue::TupleIndex {
                    receiver: recv,
                    index: *index as u128,
                    element_ty: ty,
                },
                span,
            );
            Operand::Local(tmp)
        }
    }
}

/// 调用（镜像 emit_call_resolution 的 CallKind 构造与 transport；支持集见模块注释）。
fn lower_tree_call(
    builder: &mut FnLowering,
    body: &TreeBody,
    callee: &TreeCallee,
    args: &[ExprId],
    arg_names: &[Option<scoop2_base::Symbol>],
    arg_spread: &[bool],
    ty: scoop2_hir::ty::TypeId,
    span: Span,
) -> Operand {
    // 分配序镜像 AST 路径：先 receiver（Method）、再实参、最后结果 temp
    //（emit_call_resolution 的 tmp 在实参 lower 完成后分配）。
    let recv_op = match callee {
        TreeCallee::Method { recv, .. } => Some(lower_tree_expr(builder, body, *recv)),
        _ => None,
    };
    // 限定名 variant 的 callee 死语句镜像（AST 路径先 lower `Color.Red` 的
    // callee，产生 Unit temp + UnresolvedName(receiver 名)——字节一致保留；
    // C1 清理时与 AST 路径一并删除）。
    if let TreeCallee::Variant {
        enum_fqn,
        qualified: true,
        ..
    } = callee
    {
        let unit = builder.types.unit();
        let dead = builder.alloc_temp(unit, span);
        let recv_name = builder
            .hir
            .interner
            .resolve(*enum_fqn)
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_string();
        builder.assign(dead, Rvalue::UnresolvedName { name: recv_name }, span);
    }
    let arg_ops: Vec<Operand> = args
        .iter()
        .map(|&e| lower_tree_expr(builder, body, e))
        .collect();
    let tmp = builder.alloc_temp(ty, span);
    // super `$init` 委托调用无 site_id（镜像 emit_super_init_call）。
    let call_site_id = if matches!(callee, TreeCallee::InitCall { .. }) {
        None
    } else {
        Some(builder.next_site_id())
    };
    let call_transport = builder.call_transport(ty);
    let mir_args: Vec<crate::mir::CallArg> = arg_ops
        .iter()
        .zip(args.iter())
        .zip(arg_names.iter().chain(std::iter::repeat(&None)))
        .zip(arg_spread.iter().chain(std::iter::repeat(&false)))
        .map(|(((op, &e), name), spread)| crate::mir::CallArg {
            name: *name,
            is_spread: *spread,
            value: op.clone(),
            value_ty: body.exprs[e.0 as usize].ty,
        })
        .collect();
    let _ = &arg_ops;
    let rv = match callee {
        TreeCallee::TopLevel {
            fqn,
            type_args,
            param_types,
        } => {
            let callee_fqn = builder.hir.interner.resolve(*fqn).to_string();
            Rvalue::Call {
                site_id: call_site_id,
                kind: builder.make_direct_call_kind_with_params(
                    callee_fqn,
                    type_args.clone(),
                    false,
                    Some(param_types),
                ),
                args: mir_args,
                transport: call_transport,
            }
        }
        TreeCallee::Method {
            recv,
            owner_fqn,
            method,
            is_virtual,
            is_interface,
            type_args,
            param_types,
        } => {
            let recv_op = recv_op.expect("Method 分支已预 lower receiver");
            let recv_ty = operand_ty_of(builder, &recv_op);
            let owner_str = builder.hir.interner.resolve(*owner_fqn).to_string();
            let method_str = builder.hir.interner.resolve(*method).to_string();
            let member_fqn = format!("{owner_str}.{method_str}");
            // args 组装（先于 kind——direct 前置 receiver；虚分派不前置）。
            let final_args = if *is_virtual {
                mir_args
            } else {
                let mut fa = Vec::with_capacity(mir_args.len() + 1);
                fa.push(crate::mir::CallArg {
                    name: None,
                    is_spread: false,
                    value: recv_op.clone(),
                    value_ty: recv_ty,
                });
                fa.extend(mir_args);
                fa
            };
            let overload_sig = crate::mir::stable_id::build_overload_sig(
                &builder.types,
                &builder.hir.interner,
                param_types,
            );
            let stk = crate::mir::stable_id::make_stable_template_key(
                crate::mir::stable_id::StableHashScope::Dump,
                &member_fqn,
                &[],
                &overload_sig,
            );
            // 特判：Continuation.resume → CallKind::Resume（镜像 AST 路径——
            // resume 是 continuation 对象原语，不走 itable 分发）。
            let kind = if method_str == "resume" && owner_str.ends_with("Continuation") {
                let resume_value = final_args
                    .iter()
                    .next()
                    .map(|a| a.value.clone())
                    .unwrap_or(Operand::Const(ConstValue::Unit));
                crate::mir::CallKind::Resume {
                    continuation: recv_op.clone(),
                    resume_value,
                }
            } else if *is_virtual {
                let dispatch = crate::mir::transport::DispatchMetadata {
                    owner_fqn: owner_str,
                    member_name: method_str,
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
                    generic_type_args: type_args.clone(),
                    generic_eff_args: vec![],
                };
                if *is_interface {
                    crate::mir::CallKind::Interface {
                        receiver: recv_op,
                        dispatch,
                    }
                } else {
                    crate::mir::CallKind::Virtual {
                        receiver: recv_op,
                        dispatch,
                    }
                }
            } else {
                builder.make_direct_call_kind_with_params(
                    member_fqn,
                    type_args.clone(),
                    false,
                    Some(param_types),
                )
            };
            let args_out = if matches!(kind, crate::mir::CallKind::Resume { .. }) {
                Vec::new()
            } else {
                final_args
            };
            Rvalue::Call {
                site_id: call_site_id,
                kind,
                args: args_out,
                transport: call_transport,
            }
        }
        TreeCallee::LocalValue { local } => {
            let decl = &body.locals[local.0 as usize];
            let callee_op = match builder.symbol_locals.get(&decl.name).copied() {
                Some(lid) => Operand::Local(lid),
                None => Operand::Const(ConstValue::Unit),
            };
            Rvalue::Call {
                site_id: call_site_id,
                kind: crate::mir::CallKind::FunValue { callee: callee_op },
                args: mir_args,
                transport: call_transport,
            }
        }
        TreeCallee::FunValue { callee } => {
            let callee_op = lower_tree_expr(builder, body, *callee);
            Rvalue::Call {
                site_id: call_site_id,
                kind: crate::mir::CallKind::FunValue { callee: callee_op },
                args: mir_args,
                transport: call_transport,
            }
        }
        TreeCallee::Ctor { type_fqn, .. } => {
            if !builder.hir.class_fqns.contains(type_fqn) {
                // struct：StructLit（值语义）。命名实参按 member_order 声明序
                // 重排（镜像 AST Constructor 分支）；位置实参按位对应。
                let ordered_names: Vec<scoop2_base::Symbol> = builder
                    .hir
                    .member_order
                    .get(type_fqn)
                    .cloned()
                    .unwrap_or_default();
                let any_named = mir_args.iter().any(|a| a.name.is_some());
                let mut mir_fields: Vec<crate::mir::StructLitField> =
                    Vec::with_capacity(mir_args.len());
                if any_named {
                    for &mname in &ordered_names {
                        if let Some(arg) = mir_args.iter().find(|a| a.name == Some(mname)) {
                            mir_fields.push(crate::mir::StructLitField {
                                name: mname,
                                value: arg.value.clone(),
                                value_ty: arg.value_ty,
                            });
                        }
                    }
                    // member_order 未覆盖的命名实参：按原顺序追加。
                    for arg in &mir_args {
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
                    for (i, arg) in mir_args.iter().enumerate() {
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
                // 树的 args 已含默认填充（resolved_call_args 消费）——跳过
                // expand_super_ctor_chain（与 resolved.is_some() 分支一致）。
                Rvalue::ClassCtor {
                    site_id: call_site_id,
                    type_fqn: *type_fqn,
                    ctor: crate::mir::transport::ClassCtorCallMetadata {
                        target_init_class_fqn: type_fqn_str,
                        selected_ctor_span: None,
                        ordered_param_count: mir_args.len(),
                        stable_template_key: None,
                    },
                    args: mir_args,
                    hidden_effects: scoop2_hir::ty::EffectRow::pure(),
                }
            }
        }
        TreeCallee::Variant {
            enum_fqn, variant, ..
        } => {
            let payload = builder.aggregate_transport(ty, AggregateTransportKind::EnumPayload);
            Rvalue::EnumVariant {
                enum_ty: ty,
                enum_fqn: *enum_fqn,
                variant_name: *variant,
                args: mir_args,
                payload,
                stable_key: None,
            }
        }
        TreeCallee::InitCall { target_class } => {
            // super 委托：`[this, ...实参]` 直调 `<Super>.$init`（镜像
            // emit_super_init_call——plain_no_outward transport、无 site_id）。
            let callee_fqn = format!("{}.$init", target_class);
            let unit = builder.types.unit();
            Rvalue::Call {
                site_id: None,
                kind: crate::mir::CallKind::Direct {
                    callee_fqn,
                    type_args: vec![],
                    is_intrinsic: false,
                    stable_template_key: None,
                    stable_instance_key: None,
                    generic_type_args: vec![],
                    generic_eff_args: vec![],
                    intrinsic_name: None,
                },
                args: mir_args,
                transport: crate::mir::transport::CallTransportMetadata::plain_no_outward(
                    unit,
                    crate::mir::transport::MirTransportKind::Scalar,
                ),
            }
        }
        TreeCallee::EffectOp { effect, op } => {
            // 镜像 lower_effect_op：effect-op 调用 → Perform 终结符
            let op_fqn = format!(
                "{}.{}",
                builder.hir.interner.resolve(*effect),
                builder.hir.interner.resolve(*op)
            );
            let resume_local = builder.alloc_temp(ty, span);
            let resume_target = builder.new_block();

            // 从 args 构造 payload metadata
            let payload_component_tys: Vec<scoop2_hir::ty::TypeId> = arg_ops
                .iter()
                .zip(args.iter())
                .map(|(op, e)| body.exprs[e.0 as usize].ty)
                .collect();
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
                Some(payload_component_tys[0])
            } else if payload_component_tys.is_empty() {
                None
            } else {
                Some(builder.types.tuple(payload_component_tys.clone()))
            };
            let arg_mapping: Vec<usize> = (0..args.len()).collect();

            // 解析 effect 类型
            let eff_name = builder.hir.interner.resolve(*effect);
            let effect_ty = {
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
            };

            let metadata = crate::mir::PerformMetadata {
                effect_ty,
                op_type_args: vec![],
                result_ty: ty,
                payload_tuple_ty,
                payload_component_tys,
                payload_transport,
                arg_mapping,
            };
            let site_id = Some(builder.next_site_id());

            // EffectOp 不返回普通值，而是发射 Perform 终结符并返回 resume_local
            builder.terminate(
                crate::mir::Terminator {
                    span,
                    kind: crate::mir::TerminatorKind::Perform {
                        site_id,
                        op_fqn,
                        metadata,
                        args: mir_args,
                        resume_local,
                        resume_target,
                    },
                },
                resume_target,
            );
            // resume_target 块：把 resume_local 作为结果
            return Operand::Local(resume_local);
        }
        TreeCallee::InitCall { .. } => {
            unsupported!("init 调用在支持集外")
        }
        TreeCallee::Method {
            is_virtual: true, ..
        }
        | TreeCallee::Method {
            is_interface: true, ..
        } => {
            unsupported!("虚/接口分派在支持集外")
        }
    };
    builder.assign(tmp, rv, span);
    Operand::Local(tmp)
}

/// 短路逻辑：**逐语句镜像** lower_binary 的 LogAnd/LogOr。
/// - `&&`：then = rhs，else = false；
/// - `||`：then = true，else = rhs。
/// （含 terminate 目标为 then_bb 的历史形态——字节一致优先。）
fn lower_logical(
    builder: &mut FnLowering,
    body: &TreeBody,
    lhs: ExprId,
    rhs: ExprId,
    _ty: scoop2_hir::ty::TypeId,
    span: Span,
    is_and: bool,
) -> Operand {
    let lv = lower_tree_expr(builder, body, lhs);
    let bool_ty = builder.types.bool();
    let result = builder.alloc_temp(bool_ty, span);
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let merge_bb = builder.new_block();
    builder.terminate(
        Terminator {
            span,
            kind: TerminatorKind::CondBr {
                cond: lv,
                then_target: then_bb,
                else_target: else_bb,
            },
        },
        then_bb,
    );
    builder.current_bb = then_bb;
    if is_and {
        // then: rhs → result。
        let bv = lower_tree_expr(builder, body, rhs);
        builder.assign(result, Rvalue::Use(bv), span);
    } else {
        // then: result = true。
        builder.assign(
            result,
            Rvalue::Use(Operand::Const(ConstValue::Bool(true))),
            span,
        );
    }
    builder.goto(merge_bb, span);
    builder.current_bb = else_bb;
    if is_and {
        // else: result = false。
        builder.assign(
            result,
            Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
            span,
        );
    } else {
        // else: rhs → result。
        let bv = lower_tree_expr(builder, body, rhs);
        builder.assign(result, Rvalue::Use(bv), span);
    }
    builder.goto(merge_bb, span);
    builder.current_bb = merge_bb;
    Operand::Local(result)
}

// ---- 小工具（避免对 builder 内部布局的额外假设）----

fn operand_ty_of(builder: &mut FnLowering, op: &Operand) -> scoop2_hir::ty::TypeId {
    crate::mir::lower::stmt::operand_ty_public(builder, op)
}

fn recv_ty_of(builder: &mut FnLowering, op: &Operand) -> scoop2_hir::ty::TypeId {
    crate::mir::lower::stmt::operand_ty_public(builder, op)
}

fn decl_span_of(body: &TreeBody, eid: ExprId) -> Span {
    body.exprs[eid.0 as usize].span
}

/// 标记不可达（支持集过滤后不应发生——C9：不写防御分支，直接 ICE）。
macro_rules! unsupported {
    ($msg:expr) => {{ unreachable!($msg) }};
}
use unsupported;

// ---------------------------------------------------------------------------
// 函数级脚手架：FnTree → FunDecl（签名数据从 hir 表取，不经 AST）
// ---------------------------------------------------------------------------

/// 从树构造完整 `FunDecl`（顶层函数 / 方法 / `$init`；val 初始化器树走
/// [`lower_tree_initializer`]）。
///
/// 返回 `(FunDecl, 嵌套闭包 sibling 列表, 私有 store)`；嵌套闭包与主函数共用
/// 同一 store（模块合并时统一 remap——镜像 AST）。
pub fn lower_tree_fun_decl(
    hir: &scoop2_hir::hir::TypedHir,
    file_id: scoop2_base::FileId,
    tree: &FnTree,
    base_types: &scoop2_hir::ty::TypeStore,
    sig_hint: Option<(scoop2_hir::ty::TypeId, scoop2_hir::ty::EffectRow)>,
) -> Option<(
    crate::mir::FunDecl,
    Vec<crate::mir::FunDecl>,
    scoop2_hir::ty::TypeStore,
)> {
    let mut errors: Vec<crate::diagnostics::MirLowerError> = Vec::new();
    let mut types = base_types.clone();

    // 签名数据（return/effect/参数类型）：$init 合成 → unit/pure；其余按 FQN 查表
    //（扩展函数无 top_level_funs 表项——用骨架携带的 sig_hint）。
    let (return_ty, effect_row): (scoop2_hir::ty::TypeId, scoop2_hir::ty::EffectRow) =
        if tree.fqn.ends_with(".$init") {
            (types.unit(), scoop2_hir::ty::EffectRow::pure())
        } else if let Some((ret, eff)) = sig_hint {
            (ret, eff)
        } else {
            let fqn_sym = hir.interner.get(&tree.fqn)?;
            let sig = hir
                .top_level_funs
                .get(&fqn_sym)
                .and_then(|s| s.first())
                .cloned()
                .or_else(|| {
                    // 方法：owner.method → member_funs[owner][method]
                    let dot = tree.fqn.rfind('.')?;
                    let (owner, method) = tree.fqn.split_at(dot);
                    let owner_sym = hir.interner.get(owner)?;
                    let method_sym = hir.interner.get(&method[1..])?;
                    hir.member_funs
                        .get(&owner_sym)?
                        .get(&method_sym)?
                        .first()
                        .cloned()
                })?;
            // effect 行镜像 AST `lookup_effect_row` quirk：按「包前缀.简名」查
            // top_level_funs 的 `.first()`（方法/扩展通常 miss → pure——即使
            // 签名本身带 effect 参数）。
            let eff = {
                let name = tree.fqn.rsplit('.').next().unwrap_or_default();
                let prefix = hir
                    .file(file_id)
                    .map(|f| f.package_prefix.as_str())
                    .unwrap_or("");
                let fqn_text = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{prefix}.{name}")
                };
                hir.interner
                    .get(&fqn_text)
                    .and_then(|s| hir.top_level_funs.get(&s))
                    .and_then(|sigs| sigs.first())
                    .map(|s| s.effect_row.clone())
                    .unwrap_or_else(scoop2_hir::ty::EffectRow::pure)
            };
            (sig.return_ty, eff)
        };

    // fn_ty 的参数不含隐式 <this>（镜像 AST：fd.params 事后追加 this）。
    // 例外：`$init` 合成的 fn_ty **含** this（镜像 lower_class_init_callable：
    // [this, ctor_params...]）。
    let is_init = tree.fqn.ends_with(".$init");
    let param_tys: Vec<scoop2_hir::ty::TypeId> = tree
        .params
        .iter()
        .filter(|&p| is_init || builder_this_check(hir, tree.body.locals[p.0 as usize].name))
        .map(|&p| tree.body.locals[p.0 as usize].ty)
        .collect();
    let fn_ty = types.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: param_tys.clone(),
        return_ty,
        effects: effect_row.clone(),
        closed: false,
    });
    let name = tree.fqn.rsplit('.').next().unwrap_or(&tree.fqn).to_string();
    let mut fd = crate::mir::FunDecl {
        span: scoop2_base::Span::default(),
        fqn: tree.fqn.clone(),
        name,
        ty: fn_ty,
        params: Vec::new(),
        return_ty,
        effect_row: effect_row.clone(),
        type_params: Vec::new(),
        body: None,
        file: file_id,
        stable_template_key: None,
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: None,
    };

    let mut builder = FnLowering::new(
        hir,
        types,
        file_id,
        tree.fqn.clone(),
        return_ty,
        effect_row,
        &mut errors,
    );
    for &p in &tree.params {
        let decl = &tree.body.locals[p.0 as usize];
        let sym_text = builder.hir.interner.resolve(decl.name);
        // 隐式接收者：MIR 参数名固定 `<this>`（符号表仍按 `this` 注册）。
        let mir_name = if sym_text == "this" {
            "<this>"
        } else {
            sym_text
        };
        let lid = builder.alloc_named(mir_name.to_string(), decl.ty, decl.span);
        builder.symbol_locals.insert(decl.name, lid);
        fd.params.push(crate::mir::Param {
            span: decl.span,
            name: mir_name.to_string(),
            ty: decl.ty,
            local: lid,
        });
    }
    lower_tree_fn_body(&mut builder, tree);
    let (body, nested, types_out) = builder.finish();
    fd.body = Some(body);
    Some((fd, nested, types_out))
}

/// 顶层 `val`/`var` 初始化器树 → `InitializerRoot`（镜像 builder.rs 的
/// `lower_top_level_val`：owner `#init`、lower init、Return 终结）。
pub fn lower_tree_initializer(
    hir: &scoop2_hir::hir::TypedHir,
    file_id: scoop2_base::FileId,
    tree: &FnTree,
    base_types: &scoop2_hir::ty::TypeStore,
) -> Option<(crate::mir::InitializerRoot, scoop2_hir::ty::TypeStore)> {
    let mut types = base_types.clone();
    let nothing_ty = types.nothing();
    // 类型从 top_level_vals（按简名符号——与 AST 路径一致）。
    let name_sym = tree
        .fqn
        .rsplit('.')
        .next()
        .and_then(|n| hir.interner.get(n))?;
    let ty = hir
        .top_level_vals
        .get(&name_sym)
        .copied()
        .unwrap_or(nothing_ty);
    let effect_row = scoop2_hir::ty::EffectRow::pure();
    let owner_fqn = format!("{}#init", tree.fqn);
    let mut errors: Vec<crate::diagnostics::MirLowerError> = Vec::new();
    let mut builder = FnLowering::new(hir, types, file_id, owner_fqn, ty, effect_row, &mut errors);
    // 根块为尾表达式（build_val_init_tree）——lower 后以 Return 终结。
    let root = tree.body.root?;
    let val = lower_tree_block(&mut builder, &tree.body, root);
    builder.terminate(
        Terminator {
            span: Span::default(),
            kind: TerminatorKind::Return { value: Some(val) },
        },
        builder.current_bb,
    );
    let (body, _, types_out) = builder.finish();
    Some((
        crate::mir::InitializerRoot {
            span: Span::default(),
            fqn: tree.fqn.clone(),
            ty,
            is_var: tree.val_init.unwrap_or(false),
            body,
            file: file_id,
        },
        types_out,
    ))
}

// ---------------------------------------------------------------------------
// 模块级树驱动 lowering（M2-5 翻转：MIR 只消费 HIR 产出——树 + 骨架，不读 AST）
// ---------------------------------------------------------------------------

/// 把整个 TypedHir（用户文件的树 + item 骨架）lower 为 MIR Module。
///
/// 镜像 `lower::lower_module` 的产出序与 store 合并序（逐 item：base 克隆 →
/// per-item store → extend_from + remap）；`$init` 树在成员方法合并**之后**
/// 以演化后的 module.types 为基（与 AST 的 lower_class_init_callable 调用位
/// 一致）。
pub fn lower_module_from_trees(
    hir: &scoop2_hir::hir::TypedHir,
    diags: &mut scoop2_base::diag::DiagnosticSink,
) -> crate::mir::lower::LowerResult {
    let mut module = crate::mir::Module {
        items: Vec::new(),
        types: hir.store.clone(),
    };
    let mut errors: Vec<crate::diagnostics::MirLowerError> = Vec::new();
    for tf in &hir.files {
        lower_file_from_skeleton(hir, tf, &mut module, &mut errors);
    }
    for e in &errors {
        diags.push(e.to_diagnostic());
    }
    crate::mir::lower::LowerResult { module, errors }
}

/// 单文件：按 item 骨架（源码序）产出模块 items。
fn lower_file_from_skeleton(
    hir: &scoop2_hir::hir::TypedHir,
    tf: &scoop2_hir::hir::TypedFile,
    module: &mut crate::mir::Module,
    _errors: &mut Vec<crate::diagnostics::MirLowerError>,
) {
    use scoop2_hir::hir::element::TypeCategory;
    use scoop2_hir::hir::tree::{FileItemKind, TreeBody};

    let file_id = tf.file_id;
    for entry in &tf.item_skeleton {
        let (start, end) = entry.tree_range;
        let trees = &tf.trees[start as usize..end as usize];
        match entry.kind {
            FileItemKind::Fun => {
                if trees.is_empty() {
                    // 无 body 声明（extern / abstract / intrinsic）：签名-only
                    // FunDecl（参数 local 0——镜像 AST 无 body 分支）。
                    let base = module.types.clone();
                    if let Some((item, st)) =
                        signature_only_fun_item(hir, file_id, &entry.fqn, &base)
                    {
                        let remap = module.types.extend_from(&st);
                        module
                            .items
                            .push(crate::mir::lower::remap_item(&remap, item));
                    }
                    continue;
                }
                let base = module.types.clone();
                for tree in trees {
                    if let Some((fd, nested, st)) =
                        lower_tree_fun_decl(hir, file_id, tree, &base, entry.fun_sig.clone())
                    {
                        let remap = module.types.extend_from(&st);
                        module.items.push(crate::mir::Item::Fun(
                            crate::mir::lower::remap_fun_decl(&remap, fd),
                        ));
                        for nf in nested {
                            module.items.push(crate::mir::Item::Fun(
                                crate::mir::lower::remap_fun_decl(&remap, nf),
                            ));
                        }
                    }
                }
            }
            FileItemKind::Val => {
                if trees.is_empty() {
                    // @Extern 顶层 var（无初始化器）→ ExternGlobal。
                    let nothing_ty = module.types.nothing();
                    let name_sym = entry
                        .fqn
                        .rsplit('.')
                        .next()
                        .and_then(|n| hir.interner.get(n));
                    let ty = name_sym
                        .and_then(|s| hir.top_level_vals.get(&s).copied())
                        .unwrap_or(nothing_ty);
                    module
                        .items
                        .push(crate::mir::Item::ExternGlobal(crate::mir::ExternGlobal {
                            span: Span::default(),
                            fqn: entry.fqn.clone(),
                            ty,
                            file: file_id,
                        }));
                    continue;
                }
                let base = module.types.clone();
                if let Some(tree) = trees.first()
                    && let Some((ir, st)) = lower_tree_initializer(hir, file_id, tree, &base)
                {
                    let remap = module.types.extend_from(&st);
                    module.items.push(crate::mir::lower::remap_item(
                        &remap,
                        crate::mir::Item::Initializer(ir),
                    ));
                }
            }
            FileItemKind::Type(category) => {
                module
                    .items
                    .push(crate::mir::Item::Metadata(crate::mir::MetadataRoot {
                        span: Span::default(),
                        fqn: entry.fqn.clone(),
                        kind: match category {
                            TypeCategory::Class => crate::mir::MetadataKind::Class,
                            TypeCategory::Interface => crate::mir::MetadataKind::Interface,
                            TypeCategory::Struct => crate::mir::MetadataKind::Struct,
                            TypeCategory::Enum => crate::mir::MetadataKind::Enum,
                            TypeCategory::Effect => crate::mir::MetadataKind::Effect,
                            TypeCategory::Object => crate::mir::MetadataKind::Object,
                        },
                        file: file_id,
                    }));
                // 成员方法树：同一 base（镜像 lower_type_member_funs_with_stores
                // 的共享 base）；`$init` 合成树在成员合并后以演化 store 为基；
                // 无 body 成员（接口/效应 op）发签名-only FunDecl。
                lower_type_members_from_slots(hir, file_id, entry, trees, module);
            }
            FileItemKind::Object => {
                module
                    .items
                    .push(crate::mir::Item::Metadata(crate::mir::MetadataRoot {
                        span: Span::default(),
                        fqn: entry.fqn.clone(),
                        kind: crate::mir::MetadataKind::Object,
                        file: file_id,
                    }));
                lower_type_members_from_slots(hir, file_id, entry, trees, module);
            }
        }
    }
}

/// Type/Object 的成员发射：按成员槽位（源码序）逐一产出——Tree 槽消费区间内
/// 的下一棵树（方法树共享 base；`$init` 树在成员之后以演化 store 为基），
/// Bodyless 槽发签名-only FunDecl。
fn lower_type_members_from_slots(
    hir: &scoop2_hir::hir::TypedHir,
    file_id: scoop2_base::FileId,
    entry: &scoop2_hir::hir::tree::FileItem,
    trees: &[FnTree],
    module: &mut crate::mir::Module,
) {
    use scoop2_hir::hir::tree::MemberSlot;
    let mut tree_iter = trees.iter();
    // $init 合成树固定在区间尾（无槽位）：先定位，成员槽位只消费方法树。
    let init_tree = trees
        .iter()
        .rev()
        .find(|t| t.fqn.ends_with(".$init"))
        .map(|t| t.fqn.clone());
    let base = module.types.clone();
    let mut emit_tree =
        |tree: &FnTree, module: &mut crate::mir::Module, base: &scoop2_hir::ty::TypeStore| {
            if let Some((fd, nested, st)) = lower_tree_fun_decl(hir, file_id, tree, base, None) {
                let remap = module.types.extend_from(&st);
                module
                    .items
                    .push(crate::mir::Item::Fun(crate::mir::lower::remap_fun_decl(
                        &remap, fd,
                    )));
                for nf in nested {
                    module
                        .items
                        .push(crate::mir::Item::Fun(crate::mir::lower::remap_fun_decl(
                            &remap, nf,
                        )));
                }
            }
        };
    for slot in &entry.members {
        match slot {
            MemberSlot::Tree => {
                if let Some(tree) = tree_iter.next() {
                    emit_tree(tree, module, &base);
                }
            }
            MemberSlot::Bodyless { fqn } => {
                if let Some((item, st)) = signature_only_fun_item(hir, file_id, fqn, &base) {
                    let remap = module.types.extend_from(&st);
                    module
                        .items
                        .push(crate::mir::lower::remap_item(&remap, item));
                }
            }
        }
    }
    // 成员合并后：$init 合成树以演化 store 为基（镜像 AST 调用位）。
    if let Some(init_fqn) = init_tree
        && let Some(tree) = trees.iter().rev().find(|t| t.fqn == init_fqn)
    {
        let evolved = module.types.clone();
        emit_tree(tree, module, &evolved);
    }
}

/// 无 body 函数声明的签名-only FunDecl（extern / abstract / intrinsic / 接口与
/// 效应 op）。参数类型按 FQN 查签名表（顶层 / 成员），缺失回退 Unit——镜像
/// AST 无 body 分支；`intrinsic_name` 的注解提取暂不镜像（语料内无该形态）。
/// 返回 (Item, 私有 store)——调用方合并（fn_ty 的 intern 需要进模块 store）。
fn signature_only_fun_item(
    hir: &scoop2_hir::hir::TypedHir,
    file_id: scoop2_base::FileId,
    fqn: &str,
    base_types: &scoop2_hir::ty::TypeStore,
) -> Option<(crate::mir::Item, scoop2_hir::ty::TypeStore)> {
    let fqn_sym = hir.interner.get(fqn)?;
    let sig = hir
        .top_level_funs
        .get(&fqn_sym)
        .and_then(|s| s.first())
        .cloned()
        .or_else(|| {
            let dot = fqn.rfind('.')?;
            let (owner, method) = fqn.split_at(dot);
            let owner_sym = hir.interner.get(owner)?;
            let method_sym = hir.interner.get(&method[1..])?;
            hir.member_funs
                .get(&owner_sym)?
                .get(&method_sym)?
                .first()
                .cloned()
        })?;
    let mut types = base_types.clone();
    let param_tys: Vec<scoop2_hir::ty::TypeId> = sig.param_types.clone();
    // effect 行镜像 AST `lookup_effect_row` quirk（包前缀.简名 → top_level_funs
    // `.first()`；接口/效应 op 通常 miss → pure）。
    let eff = {
        let name = fqn.rsplit('.').next().unwrap_or_default();
        let prefix = hir
            .file(file_id)
            .map(|f| f.package_prefix.as_str())
            .unwrap_or("");
        let fqn_text = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}.{name}")
        };
        hir.interner
            .get(&fqn_text)
            .and_then(|s| hir.top_level_funs.get(&s))
            .and_then(|sigs| sigs.first())
            .map(|s| s.effect_row.clone())
            .unwrap_or_else(scoop2_hir::ty::EffectRow::pure)
    };
    let fn_ty = types.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: param_tys.clone(),
        return_ty: sig.return_ty,
        effects: eff.clone(),
        closed: false,
    });
    let name = fqn.rsplit('.').next().unwrap_or(fqn).to_string();
    let mut fd = crate::mir::FunDecl {
        span: Span::default(),
        fqn: fqn.to_string(),
        name,
        ty: fn_ty,
        params: Vec::new(),
        return_ty: sig.return_ty,
        effect_row: eff,
        type_params: Vec::new(),
        body: None,
        file: file_id,
        stable_template_key: None,
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: None,
    };
    for (i, pname) in sig.param_names.iter().enumerate() {
        let pty = param_tys.get(i).copied().unwrap_or_else(|| types.unit());
        fd.params.push(crate::mir::Param {
            span: Span::default(),
            name: hir.interner.resolve(*pname).to_string(),
            ty: pty,
            local: crate::mir::LocalId(0),
        });
    }
    Some((crate::mir::Item::Fun(fd), types))
}

/// 局部是否是隐式 this（MIR fn_ty 排除）。
fn builder_this_check(hir: &scoop2_hir::hir::TypedHir, name: scoop2_base::Symbol) -> bool {
    hir.interner.resolve(name) != "this"
}

/// struct 字面量 FQN 解析（镜像 resolve_struct_fqn：裸名 → scoop.core 前缀）。
fn resolve_tree_struct_fqn(builder: &FnLowering, fqn_text: &str) -> scoop2_base::Symbol {
    for cand in [fqn_text.to_string(), format!("scoop.core.{fqn_text}")] {
        if let Some(f) = builder.hir.interner.get(&cand) {
            return f;
        }
    }
    scoop2_base::Symbol::default()
}

/// nominal FQN 文本（镜像 nominal_fqn_of：标量 → 内建 FQN）。
fn tree_nominal_fqn_of(builder: &FnLowering, ty: scoop2_hir::ty::TypeId) -> String {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    match builder.types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Int) => "scoop.core.Int".to_string(),
        TypeKind::Value(ValueTypeKind::UInt) => "scoop.core.UInt".to_string(),
        TypeKind::Value(ValueTypeKind::Bool) => "scoop.core.Bool".to_string(),
        TypeKind::Value(ValueTypeKind::Char) => "scoop.core.Char".to_string(),
        TypeKind::Value(ValueTypeKind::Float64) => "scoop.core.Float64".to_string(),
        TypeKind::Value(ValueTypeKind::Float32) => "scoop.core.Float32".to_string(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            builder.hir.interner.resolve(n.fqn).to_string()
        }
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// When 表达式 lowering（镜像 expr.rs 的 lower_when）
// ---------------------------------------------------------------------------

/// lower when 表达式（树版本）。
fn lower_tree_when(
    builder: &mut FnLowering,
    body: &TreeBody,
    subject: ExprId,
    arms: &[WhenTreeArm],
    ty: scoop2_hir::ty::TypeId,
    span: Span,
) -> Operand {
    let subj = lower_tree_expr(builder, body, subject);
    let subj_ty = operand_ty_of(builder, &subj);
    let result = builder.alloc_temp(ty, span);
    let merge_bb = builder.new_block();

    for arm in arms {
        let arm_bb = builder.new_block();
        let next_bb = builder.new_block();

        // 发射模式测试
        let matches = lower_tree_pattern_test(builder, body, &arm.pat, subj.clone(), subj_ty);

        // guard：模式命中后，若 arm 有 guard 则还需 guard 为真
        let cond = if let Some(guard) = arm.guard {
            // 先把 pattern bindings 引入（在 guard 和 arm body 之前）
            bind_tree_pattern_arm(builder, body, &arm.pat, subj.clone(), subj_ty);
            lower_tree_expr(builder, body, guard)
        } else {
            matches
        };

        builder.terminate(
            Terminator {
                span: body.exprs[arm.body.0 as usize].span,
                kind: TerminatorKind::CondBr {
                    cond,
                    then_target: arm_bb,
                    else_target: next_bb,
                },
            },
            arm_bb,
        );

        // arm body（bindings 在 guard 阶段已引入；若无 guard 则在此引入）
        if arm.guard.is_none() {
            bind_tree_pattern_arm(builder, body, &arm.pat, subj.clone(), subj_ty);
        }

        builder.current_bb = arm_bb;
        let v = lower_tree_expr(builder, body, arm.body);
        let body_span = body.exprs[arm.body.0 as usize].span;
        builder.assign(result, Rvalue::Use(v), body_span);
        builder.goto(merge_bb, body_span);

        builder.current_bb = next_bb;
    }

    // 无 arm 命中：result = Unit
    builder.assign(result, Rvalue::Use(Operand::Const(ConstValue::Unit)), span);
    builder.goto(merge_bb, span);
    builder.current_bb = merge_bb;
    Operand::Local(result)
}

/// 为树模式发射测试（树版本）。
/// handle 表达式 lowering（镜像 expr.rs 的 lower_handle）。
fn lower_tree_handle(
    builder: &mut FnLowering,
    body: &TreeBody,
    hbody: BlockId,
    arms: &[HandleTreeArm],
    finally_: Option<BlockId>,
    ty: scoop2_hir::ty::TypeId,
    span: Span,
) -> Operand {
    let result = builder.alloc_temp(ty, span);
    let body_bb = builder.new_block();
    let exit_bb = builder.new_block();
    let arm_bbs: Vec<_> = arms.iter().map(|_| builder.new_block()).collect();
    let finally_bb = finally_.map(|_| builder.new_block());
    // binder 符号注册会遮盖外层同名绑定；嵌套 handle 的 arm body 在本 handle
    // 之后 lower——快照旧值，结束时恢复（镜像 AST）。
    let mut saved_binder_bindings: std::collections::HashMap<
        scoop2_base::Symbol,
        Option<crate::mir::LocalId>,
    > = std::collections::HashMap::new();
    let mut handler_arms: Vec<crate::mir::transport::HandlerArm> = Vec::with_capacity(arms.len());
    let mut arm_binder_pairs: Vec<Vec<(scoop2_base::Symbol, crate::mir::LocalId)>> =
        Vec::with_capacity(arms.len());
    for arm in arms {
        // op_fqn = effect 简名 . op 名（镜像 AST 的 last_segment 规则）。
        let effect_name = arm
            .effect_path
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_string();
        let op_name = builder.hir.interner.resolve(arm.op).to_string();
        let op_fqn = if effect_name.is_empty() {
            op_name.clone()
        } else {
            format!("{}.{}", effect_name, op_name)
        };
        // 解析 handled effect 类型（enum 登记则为 effect nominal，否则 Any）。
        let handled_effect_ty = builder
            .hir
            .interner
            .get(&effect_name)
            .and_then(|fqn| {
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
        // binder 类型的回退源：op 声明签名参数类型（member_funs，包前缀候选）。
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
                        .and_then(|m| m.get(&arm.op))
                })
                .and_then(|sigs| sigs.first())
                .map(|sig| sig.param_types.clone())
                .unwrap_or_default()
        };
        let mut binder_locals: Vec<crate::mir::LocalId> = Vec::new();
        let mut binder_pairs: Vec<(scoop2_base::Symbol, crate::mir::LocalId)> = Vec::new();
        let mut payload_component_tys: Vec<scoop2_hir::ty::TypeId> = Vec::new();
        for (bi, bspec) in arm.binders.iter().enumerate() {
            // bty 链：ascription → op 签名参数类型 → Any（镜像 AST）。
            let bty = bspec
                .ascription_ty
                .or_else(|| op_param_tys.get(bi).copied())
                .unwrap_or_else(|| builder.types.any());
            payload_component_tys.push(bty);
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(bspec.name).to_string(),
                bty,
                bspec.span,
            );
            saved_binder_bindings
                .entry(bspec.name)
                .or_insert_with(|| builder.symbol_locals.get(&bspec.name).copied());
            builder.symbol_locals.insert(bspec.name, lid);
            binder_locals.push(lid);
            binder_pairs.push((bspec.name, lid));
        }
        // resuming arm 的 escape continuation binder（Any 类型——effect lowering
        // pass 用精确类型替换，镜像 AST）。
        let (continuation_local, kind) = if let Some(k_local) = arm.escape_cont {
            let k_decl = &body.locals[k_local.0 as usize];
            let cont_ty = builder.types.any();
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(k_decl.name).to_string(),
                cont_ty,
                k_decl.span,
            );
            saved_binder_bindings
                .entry(k_decl.name)
                .or_insert_with(|| builder.symbol_locals.get(&k_decl.name).copied());
            builder.symbol_locals.insert(k_decl.name, lid);
            binder_pairs.push((k_decl.name, lid));
            (
                Some(lid),
                crate::mir::transport::HandlerArmKind::EscapeContinuation,
            )
        } else {
            (None, crate::mir::transport::HandlerArmKind::NonResuming)
        };
        handler_arms.push(crate::mir::transport::HandlerArm {
            op_fqn,
            op_type_args: Vec::new(),
            binder_count: arm.binders.len(),
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
    let handle_metadata = crate::mir::transport::HandleMetadata {
        result_ty: ty,
        body_result_ty: ty,
        finally_result_ty: None,
        result_local: result,
    };
    let handle_site_id = Some(builder.next_site_id());
    builder.terminate(
        Terminator {
            span,
            kind: TerminatorKind::Handle {
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
    let bv = lower_tree_block(builder, body, hbody);
    builder.assign(result, Rvalue::Use(bv), span);
    builder.goto(exit_bb, span);
    // arms（lower 各 arm body 到对应块，结果写 result；binder 作用域先重装后恢复）。
    for (i, arm) in arms.iter().enumerate() {
        builder.current_bb = arm_bbs[i];
        let mut arm_saved: Vec<(scoop2_base::Symbol, Option<crate::mir::LocalId>)> =
            Vec::with_capacity(arm_binder_pairs[i].len());
        for &(sym, lid) in &arm_binder_pairs[i] {
            arm_saved.push((sym, builder.symbol_locals.get(&sym).copied()));
            builder.symbol_locals.insert(sym, lid);
        }
        let v = lower_tree_expr(builder, body, arm.body);
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
        let body_span = body.exprs[arm.body.0 as usize].span;
        builder.assign(result, Rvalue::Use(v), body_span);
        builder.goto(exit_bb, span);
    }
    // finally。
    if let (Some(fb), Some(fblock)) = (finally_bb, finally_) {
        builder.current_bb = fb;
        lower_tree_block(builder, body, fblock);
        builder.goto(exit_bb, span);
    }
    // 恢复 handle 之前的同名绑定（嵌套 handle 不泄漏 binder）。
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

fn lower_tree_pattern_test(
    builder: &mut FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let bool_ty = builder.types.bool();
    match pat {
        TreePattern::Wildcard | TreePattern::Else | TreePattern::Rest => {
            Operand::Const(ConstValue::Bool(true))
        }
        TreePattern::Binder { .. } | TreePattern::Struct { .. } => {
            // irrefutable 模式：类型已由 typecheck 保证匹配 → 总是命中
            // （AST 路径 Bind/Struct 同 arm）。
            Operand::Const(ConstValue::Bool(true))
        }
        TreePattern::Tuple(elems) => {
            // tuple 模式：逐元素提取并递归测试（AND 链，short-circuit）
            let testable: Vec<(usize, &TreePattern)> = elems
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    matches!(
                        e,
                        TreePattern::Literal(_)
                            | TreePattern::Variant { .. }
                            | TreePattern::Tuple(_)
                            | TreePattern::Struct { .. }
                            | TreePattern::Is { .. }
                            | TreePattern::Or(_)
                    )
                })
                .collect();

            if testable.is_empty() {
                return Operand::Const(ConstValue::Bool(true));
            }

            let result = builder.alloc_temp(bool_ty, subj_ty_of(builder, &subj));
            let merge_bb = builder.new_block();
            let mut prev_test: Option<Operand> = None;

            for (i, sub_pat) in testable {
                // 前置测试失败 → result = false，goto merge（首元素无前置条件）
                if let Some(prev) = prev_test.take() {
                    let cont_bb = builder.new_block();
                    let fail_bb = builder.new_block();
                    builder.terminate(
                        Terminator {
                            span: subj_ty_of(builder, &subj),
                            kind: TerminatorKind::CondBr {
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
                        subj_ty_of(builder, &subj),
                    );
                    builder.goto(merge_bb, subj_ty_of(builder, &subj));
                    builder.current_bb = cont_bb;
                }

                let elem_ty =
                    tree_tuple_elem_ty(builder, subj_ty, i).unwrap_or_else(|| builder.types.any());
                let tmp = builder.alloc_temp(elem_ty, subj_ty_of(builder, &subj));
                builder.assign(
                    tmp,
                    Rvalue::PatternExtract {
                        subject: subj.clone(),
                        path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(i)],
                        result_ty: elem_ty,
                    },
                    subj_ty_of(builder, &subj),
                );
                prev_test = Some(lower_tree_pattern_test(
                    builder,
                    body,
                    sub_pat,
                    Operand::Local(tmp),
                    elem_ty,
                ));
            }

            // 全部通过：result = 最后一个子测试的值
            builder.assign(
                result,
                Rvalue::Use(prev_test.expect("testable 非空")),
                subj_ty_of(builder, &subj),
            );
            builder.goto(merge_bb, subj_ty_of(builder, &subj));
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        TreePattern::Variant {
            enum_fqn,
            variant,
            args,
        } => {
            // variant 模式：发射 PatternMatch（镜像 AST 路径的 enum_fqn 解析逻辑）
            let variant_name = *variant;
            let variant_name_str = builder.hir.interner.resolve(variant_name).to_string();

            // 解析 enum FQN（镜像 expr.rs:2438-2456 的逻辑）
            let enum_fqn_sym = {
                let prefix = builder
                    .hir
                    .file(builder.file_id)
                    .map(|f| f.package_prefix.as_str())
                    .unwrap_or("");
                let candidates = if prefix.is_empty() {
                    vec![enum_fqn.clone(), variant_name_str.clone()]
                } else {
                    vec![
                        enum_fqn.clone(),
                        format!("{prefix}.{enum_fqn}"),
                        variant_name_str.clone(),
                        format!("{prefix}.{variant_name_str}"),
                    ]
                };
                // 解析 enum FQN（镜像 expr.rs:2438-2456 的逻辑）
                let enum_fqn_result = candidates
                    .iter()
                    .filter_map(|c| builder.hir.interner.get(c))
                    .filter(|f| builder.hir.enum_variants.contains_key(f))
                    .next();
                match enum_fqn_result {
                    Some(sym) => sym,
                    None => variant_name,
                }
            };

            // tag 级测试的 args：嵌套子模式降级为 Wildcard
            let tag_args: Vec<crate::mir::Pattern> = args
                .iter()
                .map(|a| match a {
                    TreePattern::Binder { .. } | TreePattern::Wildcard => {
                        lower_tree_pattern_to_mir(builder, body, a)
                    }
                    _ => crate::mir::Pattern::Wildcard,
                })
                .collect();

            let tag_tmp = builder.alloc_temp(bool_ty, subj_ty_of(builder, &subj));
            builder.assign(
                tag_tmp,
                Rvalue::PatternMatch {
                    subject: subj.clone(),
                    pattern: crate::mir::Pattern::Variant {
                        enum_fqn: enum_fqn_sym,
                        variant_name,
                        args: tag_args,
                    },
                },
                subj_ty_of(builder, &subj),
            );

            // 嵌套子模式位置（需要提取 payload 字段后递归测试）
            let nested: Vec<(usize, &TreePattern)> = args
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    matches!(
                        a,
                        TreePattern::Variant { .. }
                            | TreePattern::Tuple(_)
                            | TreePattern::Struct { .. }
                            | TreePattern::Literal(_)
                            | TreePattern::Is { .. }
                            | TreePattern::Or(_)
                    )
                })
                .collect();

            if nested.is_empty() {
                return Operand::Local(tag_tmp);
            }

            // AND 链：tag 测试通过后，逐位置提取 payload 字段并递归测试
            let result = builder.alloc_temp(bool_ty, subj_ty_of(builder, &subj));
            let merge_bb = builder.new_block();
            let mut prev_test = Operand::Local(tag_tmp);

            for (i, sub_pat) in nested {
                let cont_bb = builder.new_block();
                let fail_bb = builder.new_block();
                builder.terminate(
                    Terminator {
                        span: subj_ty_of(builder, &subj),
                        kind: TerminatorKind::CondBr {
                            cond: prev_test.clone(),
                            then_target: cont_bb,
                            else_target: fail_bb,
                        },
                    },
                    cont_bb,
                );

                // 失败路径：result = false，goto merge
                builder.current_bb = fail_bb;
                builder.assign(
                    result,
                    Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
                    subj_ty_of(builder, &subj),
                );
                builder.goto(merge_bb, subj_ty_of(builder, &subj));

                // 通过路径：提取第 i 个 payload 字段并递归测试
                builder.current_bb = cont_bb;
                let field_ty = tree_variant_payload_field_ty(builder, subj_ty, *variant, i);
                prev_test = if let Some(field_ty) = field_ty {
                    let tmp = builder.alloc_temp(field_ty, subj_ty_of(builder, &subj));
                    let variant_str = builder.hir.interner.resolve(*variant).to_string();
                    builder.assign(
                        tmp,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![crate::mir::transport::PatternBindingStep::VariantField {
                                variant: variant_str,
                                field_index: i,
                            }],
                            result_ty: field_ty,
                        },
                        subj_ty_of(builder, &subj),
                    );
                    lower_tree_pattern_test(builder, body, sub_pat, Operand::Local(tmp), field_ty)
                } else {
                    Operand::Const(ConstValue::Bool(true))
                };
            }

            // 全部通过：result = 最后一个子测试的值
            builder.assign(result, Rvalue::Use(prev_test), subj_ty_of(builder, &subj));
            builder.goto(merge_bb, subj_ty_of(builder, &subj));
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        TreePattern::Literal(lit) => {
            // 字面量模式：发射 PatternMatch
            let mir_pat = match lit {
                scoop2_hir::hir::tree::Lit::Unit => crate::mir::Pattern::Tuple { elements: vec![] },
                scoop2_hir::hir::tree::Lit::Bool(b) => crate::mir::Pattern::BoolLit(*b),
                scoop2_hir::hir::tree::Lit::Int(v, _) => crate::mir::Pattern::IntLit(*v as i128),
                scoop2_hir::hir::tree::Lit::Char(c) => crate::mir::Pattern::CharLit(*c),
                scoop2_hir::hir::tree::Lit::Str(s) => crate::mir::Pattern::StringLit(s.clone()),
                scoop2_hir::hir::tree::Lit::Float(_) => crate::mir::Pattern::Wildcard, // Float 模式暂不支持
            };
            let span = subj_ty_of(builder, &subj);
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
        TreePattern::Is { ty } => {
            // `is T` 模式：发射 PatternMatch{Is{ty, negated}}
            let mir_pat = crate::mir::Pattern::Is {
                ty: *ty,
                negated: false,
            };
            let span = subj_ty_of(builder, &subj);
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
        TreePattern::Or(alts) => {
            // or 模式：发射各子模式的 PatternMatch OR 链
            if alts.is_empty() {
                return Operand::Const(ConstValue::Bool(false));
            }

            let result = builder.alloc_temp(bool_ty, subj_ty_of(builder, &subj));
            let merge_bb = builder.new_block();

            for alt in alts {
                let test = lower_tree_pattern_test(builder, body, alt, subj.clone(), subj_ty);
                let match_bb = builder.new_block();
                let next_bb = builder.new_block();

                builder.terminate(
                    Terminator {
                        span: subj_ty_of(builder, &subj),
                        kind: TerminatorKind::CondBr {
                            cond: test,
                            then_target: match_bb,
                            else_target: next_bb,
                        },
                    },
                    match_bb,
                );

                // 匹配成功：result = true，goto merge
                builder.current_bb = match_bb;
                builder.assign(
                    result,
                    Rvalue::Use(Operand::Const(ConstValue::Bool(true))),
                    subj_ty_of(builder, &subj),
                );
                builder.goto(merge_bb, subj_ty_of(builder, &subj));
                builder.current_bb = next_bb;
            }

            // 所有子模式都不匹配：result = false
            builder.assign(
                result,
                Rvalue::Use(Operand::Const(ConstValue::Bool(false))),
                subj_ty_of(builder, &subj),
            );
            builder.goto(merge_bb, subj_ty_of(builder, &subj));
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
    }
}

/// 按树模式绑定（**解构语义**——镜像 stmt.rs `bind_pattern`：`val (a,b) = pair` /
/// `val Some(x) = opt` / `val Point { x, y } = p`）。
fn bind_tree_pattern(
    builder: &mut FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
    mutable: bool,
) {
    match pat {
        TreePattern::Wildcard | TreePattern::Else | TreePattern::Rest => {}
        TreePattern::Binder { local, .. } => {
            // 镜像 AST：binder 类型取 subject 类型（非 pattern_bindings）。
            let decl = &body.locals[local.0 as usize];
            let lid = builder.alloc_named_mutable(
                builder.hir.interner.resolve(decl.name).to_string(),
                subj_ty,
                decl.span,
                mutable,
            );
            builder.symbol_locals.insert(decl.name, lid);
            builder.assign(lid, Rvalue::Use(subj), decl.span);
        }
        TreePattern::Tuple(elems) => {
            // 逐元素：temp + TupleIndex + 递归绑定（分配序与 AST 交错一致；
            // 元素提取用 Rvalue::TupleIndex——与 AST bind_pattern 相同）。
            for (i, sub) in elems.iter().enumerate() {
                let elem_ty =
                    tree_tuple_elem_ty(builder, subj_ty, i).unwrap_or_else(|| builder.types.any());
                let tmp = builder.alloc_temp(elem_ty, Span::default());
                builder.assign(
                    tmp,
                    Rvalue::TupleIndex {
                        receiver: subj.clone(),
                        index: i as u128,
                        element_ty: elem_ty,
                    },
                    Span::default(),
                );
                bind_tree_pattern(builder, body, sub, Operand::Local(tmp), elem_ty, mutable);
            }
        }
        TreePattern::Variant {
            enum_fqn,
            variant,
            args,
        } => {
            // variant 解构：binder 按字段位置经 PatternExtract（VariantField path）
            // 从 payload 提取——binder 类型取 pattern_bindings（树 local ty）。
            let variant_str = builder.hir.interner.resolve(*variant).to_string();
            for (i, arg) in args.iter().enumerate() {
                match arg {
                    TreePattern::Binder { local, .. } => {
                        let decl = &body.locals[local.0 as usize];
                        let lid = builder.alloc_named_mutable(
                            builder.hir.interner.resolve(decl.name).to_string(),
                            decl.ty,
                            decl.span,
                            mutable,
                        );
                        builder.symbol_locals.insert(decl.name, lid);
                        builder.assign(
                            lid,
                            Rvalue::PatternExtract {
                                subject: subj.clone(),
                                path: vec![
                                    crate::mir::transport::PatternBindingStep::VariantField {
                                        variant: variant_str.clone(),
                                        field_index: i,
                                    },
                                ],
                                result_ty: decl.ty,
                            },
                            decl.span,
                        );
                    }
                    TreePattern::Variant { .. }
                    | TreePattern::Tuple(_)
                    | TreePattern::Struct { .. } => {
                        // 嵌套子模式：提取字段后递归绑定。
                        if let Some(field_ty) =
                            tree_variant_payload_field_ty(builder, subj_ty, *variant, i)
                        {
                            let tmp = builder.alloc_temp(field_ty, Span::default());
                            builder.assign(
                                tmp,
                                Rvalue::PatternExtract {
                                    subject: subj.clone(),
                                    path: vec![
                                        crate::mir::transport::PatternBindingStep::VariantField {
                                            variant: variant_str.clone(),
                                            field_index: i,
                                        },
                                    ],
                                    result_ty: field_ty,
                                },
                                Span::default(),
                            );
                            bind_tree_pattern(
                                builder,
                                body,
                                arg,
                                Operand::Local(tmp),
                                field_ty,
                                mutable,
                            );
                        }
                    }
                    // Wildcard/Literal/Is/Or：不绑定。
                    _ => {}
                }
            }
        }
        TreePattern::Struct { fields } => {
            // struct 解构：简写字段经 MemberAccess 绑定；显式子模式递归。
            for f in fields {
                if let Some(sub) = &f.sub {
                    bind_tree_pattern(builder, body, sub, subj.clone(), subj_ty, mutable);
                    continue;
                }
                let Some(local) = f.binder else { continue };
                let decl = &body.locals[local.0 as usize];
                let fty = decl.ty;
                let bname_str = builder.hir.interner.resolve(decl.name).to_string();
                let lid = builder.alloc_named_mutable(bname_str.clone(), fty, decl.span, mutable);
                builder.symbol_locals.insert(decl.name, lid);
                let site_id = builder.next_site_id();
                let member = builder.member_access_metadata(&bname_str, subj_ty);
                builder.assign(
                    lid,
                    Rvalue::MemberAccess {
                        site_id: Some(site_id),
                        receiver: subj.clone(),
                        member,
                    },
                    decl.span,
                );
            }
        }
        // refutable 模式在 val 解构上下文不合法（HIR 已拒绝）——无绑定。
        TreePattern::Literal(_) | TreePattern::Is { .. } | TreePattern::Or(_) => {}
    }
}

/// when arm 的模式绑定（**arm 语义**——镜像 expr.rs `bind_pattern_arm`：binder
/// 直接从 subject 按位置提取，分配序 = pattern_bindings 出现序 = 树字段序）。
fn bind_tree_pattern_arm(
    builder: &mut FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
) {
    match pat {
        TreePattern::Wildcard | TreePattern::Else | TreePattern::Rest => {}
        TreePattern::Binder { local, .. } => {
            // 镜像 AST arm：binder 类型取 subject 类型。
            let decl = &body.locals[local.0 as usize];
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(decl.name).to_string(),
                subj_ty,
                decl.span,
            );
            builder.symbol_locals.insert(decl.name, lid);
            builder.assign(lid, Rvalue::Use(subj), decl.span);
        }
        TreePattern::Tuple(elems) => {
            // tuple 元素绑定：按元素位置提取
            for (i, elem) in elems.iter().enumerate() {
                if let TreePattern::Binder { local, .. } = elem {
                    let decl = &body.locals[local.0 as usize];
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(decl.name).to_string(),
                        decl.ty,
                        decl.span,
                    );
                    builder.symbol_locals.insert(decl.name, lid);

                    builder.assign(
                        lid,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(i)],
                            result_ty: decl.ty,
                        },
                        decl.span,
                    );
                }
            }

            // 嵌套子模式：按元素位置提取后递归绑定
            for (i, elem) in elems.iter().enumerate() {
                if matches!(
                    elem,
                    TreePattern::Variant { .. } | TreePattern::Tuple(_) | TreePattern::Or(_)
                ) {
                    if let Some(elem_ty) = tree_tuple_elem_ty(builder, subj_ty, i) {
                        let tmp = builder.alloc_temp(elem_ty, subj_ty_of(builder, &subj));
                        builder.assign(
                            tmp,
                            Rvalue::PatternExtract {
                                subject: subj.clone(),
                                path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(
                                    i,
                                )],
                                result_ty: elem_ty,
                            },
                            subj_ty_of(builder, &subj),
                        );
                        bind_tree_pattern_arm(builder, body, elem, Operand::Local(tmp), elem_ty);
                    }
                }
            }
        }
        TreePattern::Variant {
            enum_fqn,
            variant,
            args,
        } => {
            // variant 字段绑定：按字段位置提取
            let variant_str = builder.hir.interner.resolve(*variant).to_string();

            for (i, arg) in args.iter().enumerate() {
                if let TreePattern::Binder { local, .. } = arg {
                    let decl = &body.locals[local.0 as usize];
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(decl.name).to_string(),
                        decl.ty,
                        decl.span,
                    );
                    builder.symbol_locals.insert(decl.name, lid);

                    builder.assign(
                        lid,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![crate::mir::transport::PatternBindingStep::VariantField {
                                variant: variant_str.clone(),
                                field_index: i,
                            }],
                            result_ty: decl.ty,
                        },
                        decl.span,
                    );
                }
            }

            // 嵌套子模式：按字段位置提取后递归绑定
            for (i, arg) in args.iter().enumerate() {
                if matches!(
                    arg,
                    TreePattern::Variant { .. } | TreePattern::Tuple(_) | TreePattern::Or(_)
                ) {
                    if let Some(field_ty) =
                        tree_variant_payload_field_ty(builder, subj_ty, *variant, i)
                    {
                        let tmp = builder.alloc_temp(field_ty, subj_ty_of(builder, &subj));
                        builder.assign(
                            tmp,
                            Rvalue::PatternExtract {
                                subject: subj.clone(),
                                path: vec![
                                    crate::mir::transport::PatternBindingStep::VariantField {
                                        variant: variant_str.clone(),
                                        field_index: i,
                                    },
                                ],
                                result_ty: field_ty,
                            },
                            subj_ty_of(builder, &subj),
                        );
                        bind_tree_pattern_arm(builder, body, arg, Operand::Local(tmp), field_ty);
                    }
                }
            }
        }
        TreePattern::Struct { fields } => {
            // struct 字段 binder：字段下标按 subject 声明序（ordered_members）
            // 定位，与 pattern 书写序无关。
            for (i, f) in fields.iter().enumerate() {
                let Some(local) = f.binder else { continue };
                let decl = &body.locals[local.0 as usize];
                let lid = builder.alloc_named(
                    builder.hir.interner.resolve(decl.name).to_string(),
                    decl.ty,
                    decl.span,
                );
                builder.symbol_locals.insert(decl.name, lid);
                let pos = tree_nominal_field_index(builder, subj_ty, decl.name).unwrap_or(i);
                builder.assign(
                    lid,
                    Rvalue::PatternExtract {
                        subject: subj.clone(),
                        path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(pos)],
                        result_ty: decl.ty,
                    },
                    decl.span,
                );
            }
        }
        // is：subject 本身可用；literal / or：不绑定。
        TreePattern::Is { .. } | TreePattern::Literal(_) | TreePattern::Or(_) => {}
    }
}

/// nominal 字段的声明序下标（`nominal_field_index` 的树版本）。
fn tree_nominal_field_index(
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

/// 树模式 → MIR 模式（用于 PatternMatch）。
fn lower_tree_pattern_to_mir(
    builder: &FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
) -> crate::mir::Pattern {
    match pat {
        TreePattern::Wildcard | TreePattern::Rest => crate::mir::Pattern::Wildcard,
        TreePattern::Binder { local, node_ty } => {
            let decl = &body.locals[local.0 as usize];
            crate::mir::Pattern::Bind {
                name: decl.name,
                ty: *node_ty,
            }
        }
        TreePattern::Literal(lit) => match lit {
            scoop2_hir::hir::tree::Lit::Unit => crate::mir::Pattern::Tuple { elements: vec![] },
            scoop2_hir::hir::tree::Lit::Bool(b) => crate::mir::Pattern::BoolLit(*b),
            scoop2_hir::hir::tree::Lit::Int(v, _) => crate::mir::Pattern::IntLit(*v as i128),
            scoop2_hir::hir::tree::Lit::Char(c) => crate::mir::Pattern::CharLit(*c),
            scoop2_hir::hir::tree::Lit::Str(s) => crate::mir::Pattern::StringLit(s.clone()),
            scoop2_hir::hir::tree::Lit::Float(_) => crate::mir::Pattern::Wildcard,
        },
        TreePattern::Is { ty } => crate::mir::Pattern::Is {
            ty: *ty,
            negated: false,
        },
        _ => crate::mir::Pattern::Wildcard,
    }
}

/// variant 模式第 `index` 个 payload 字段的类型（树版本）。
fn tree_variant_payload_field_ty(
    builder: &FnLowering,
    subj_ty: scoop2_hir::ty::TypeId,
    variant: scoop2_base::Symbol,
    index: usize,
) -> Option<scoop2_hir::ty::TypeId> {
    use scoop2_hir::ty::{TypeKind, ValueTypeKind};

    // Option<T>：Some 的 payload = inner（index 0）
    if let Some(inner) = builder
        .types
        .nominal_args_of_fqn(subj_ty, builder.types.option_fqn())
        .and_then(|args| args.first().copied())
    {
        return if index == 0 { Some(inner) } else { None };
    }

    match builder.types.kind(subj_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n))
        | TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => {
            // 镜像 variant_payload_field_ty：variant FQN 文本 = subject nominal
            // 的完整 FQN + variant 名（非模式路径文本——裸名/限定名统一）。
            let variant_text = builder.hir.interner.resolve(variant);
            let fqn_text = format!("{}.{}", builder.hir.interner.resolve(n.fqn), variant_text);
            let vfqn = builder.hir.interner.get(&fqn_text)?;
            let members = builder.hir.ordered_members(&vfqn);
            members.get(index).map(|(_, ty)| *ty)
        }
        _ => None,
    }
}

/// tuple 类型第 `index` 个元素的类型（树版本）。
fn tree_tuple_elem_ty(
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

/// operand 的类型（复用 stmt 模块的函数）。
fn subj_ty_of(builder: &FnLowering, op: &Operand) -> Span {
    // 对于模式测试，span 主要用于标记位置，使用默认值即可
    Span::default()
}

// ---------------------------------------------------------------------------
// Lambda 表达式 lowering（镜像 lower/expr.rs 的 lower_lambda）
// ---------------------------------------------------------------------------

/// lower lambda（闭包）：生成 env tuple + 嵌套 Item::Fun（树版本）。
fn lower_tree_lambda(
    builder: &mut FnLowering,
    body: &TreeBody,
    params: &[scoop2_hir::hir::tree::LocalId],
    lambda_body: &scoop2_hir::hir::tree::LambdaBodyTree,
    implicit_it: bool,
    ty: scoop2_hir::ty::TypeId,
    span: Span,
) -> Operand {
    use scoop2_hir::hir::tree::LambdaBodyTree;

    // 真实自由变量（lambda body 内引用且在外层 symbol_locals 有对应 local 的名字）。
    let free_vars = collect_tree_lambda_free_vars(body, params, lambda_body);
    let captured: Vec<(scoop2_base::Symbol, scoop2_hir::ty::TypeId)> = free_vars
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

    // captures metadata（闭包捕获 transport：值类型捕获到 Any 边界标记 boxing）。
    let mut captures_meta = Vec::new();
    for (cap_sym, cap_ty) in captured.iter() {
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
        let any_ty = builder.types.any();
        let cap_transport = if *cap_ty != any_ty
            && matches!(
                builder.types.kind(*cap_ty),
                scoop2_hir::ty::TypeKind::Value(_)
            ) {
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

    // 嵌套函数签名：第一个参数是 env tuple，其后是 lambda 参数（类型按位取自
    // 函数类型——树 local 的 ty 即由此而来）。
    let lambda_param_tys: Vec<scoop2_hir::ty::TypeId> = match builder.types.kind(ty) {
        scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Function(ft)) => {
            ft.params.clone()
        }
        _ => Vec::new(),
    };
    let (return_ty, fn_effect_row) = match builder.types.kind(ty) {
        scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Function(ft)) => {
            (ft.return_ty, ft.effects.clone())
        }
        _ => (builder.types.any(), scoop2_hir::ty::EffectRow::pure()),
    };
    // fn_ty = [env] + 显式参数类型。隐式 `it` 形态下 AST 路径的 fn_ty **不含**
    // it 参数（fn_ty 与 Param 列表不一致的历史 quirk——字节一致保留）。
    let mut all_param_tys = vec![env_ty];
    if !implicit_it {
        all_param_tys.extend(lambda_param_tys.iter().copied());
    }

    let mut nested_store = builder.types.clone();
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

    // lambda 参数 → locals（树的 params 已含隐式 `it`——AST 路径的无参补 it
    // 分支在此不适用）。
    let mut nested_params: Vec<crate::mir::Param> = Vec::new();
    nested_params.push(crate::mir::Param {
        span,
        name: "$env".to_string(),
        ty: env_ty,
        local: env_lid,
    });
    for (i, &param_id) in params.iter().enumerate() {
        let decl = &body.locals[param_id.0 as usize];
        let pty = if i < lambda_param_tys.len() {
            lambda_param_tys[i]
        } else {
            decl.ty
        };
        let lid = nested_builder.alloc_named(
            builder.hir.interner.resolve(decl.name).to_string(),
            pty,
            decl.span,
        );
        nested_builder.symbol_locals.insert(decl.name, lid);
        nested_params.push(crate::mir::Param {
            span: decl.span,
            name: builder.hir.interner.resolve(decl.name).to_string(),
            ty: pty,
            local: lid,
        });
    }

    // lower lambda body（Block 或 Expr；块尾隐式返回与 lower_fun_body 同构）。
    match lambda_body {
        LambdaBodyTree::Block(block_id) => {
            let tail = lower_tree_block(&mut nested_builder, body, *block_id);
            let tail_is_unit = matches!(tail, Operand::Const(ConstValue::Unit));
            let bb = nested_builder.current_bb;
            if !tail_is_unit
                && matches!(
                    nested_builder.body.blocks[bb.0 as usize].terminator.kind,
                    TerminatorKind::Unreachable
                )
            {
                nested_builder.terminate(
                    Terminator {
                        span: body.blocks[block_id.0 as usize].span,
                        kind: TerminatorKind::Return { value: Some(tail) },
                    },
                    bb,
                );
            }
        }
        LambdaBodyTree::Expr(expr_id) => {
            let val = lower_tree_expr(&mut nested_builder, body, *expr_id);
            let cur_bb = nested_builder.current_bb;
            nested_builder.terminate(
                Terminator {
                    span: body.exprs[expr_id.0 as usize].span,
                    kind: TerminatorKind::Return { value: Some(val) },
                },
                cur_bb,
            );
        }
    }

    let (nested_body, nested_more, store_out) = nested_builder.finish();

    builder.nested_funs.push(crate::mir::FunDecl {
        span,
        fqn: invoke_fqn.clone(),
        name: format!("$closure{}", builder.closure_counter),
        ty: nested_fn_ty,
        params: nested_params,
        return_ty,
        effect_row: fn_effect_row,
        type_params: vec![],
        body: Some(nested_body),
        file: builder.file_id,
        stable_template_key: None,
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: None,
    });
    builder.nested_funs.extend(nested_more);
    // nested 的类型 / 错误合并回外层（与 AST 路径一致——TypeId 一致性）。
    let _remap = builder.types.extend_from(&store_out);
    builder.errors.extend(errors);
    Operand::Local(tmp)
}

// ---------------------------------------------------------------------------
// lambda 自由变量扫描（镜像 lower/expr.rs 的 scan_*_idents 家族）
// ---------------------------------------------------------------------------

/// 收集树 lambda 的自由变量（按 Symbol 排序的确定序——与 AST 路径的排序一致）。
/// 树的 `params` 已含隐式 `it`，一并排除（AST 无参 lambda 不排除 `it`，仅在
/// 外层恰有同名 local 时才会被 symbol_locals 过滤捕获——语料内无此形态）。
fn collect_tree_lambda_free_vars(
    body: &TreeBody,
    params: &[scoop2_hir::hir::tree::LocalId],
    lambda_body: &scoop2_hir::hir::tree::LambdaBodyTree,
) -> Vec<scoop2_base::Symbol> {
    use scoop2_hir::hir::tree::LambdaBodyTree;
    let mut syms = std::collections::HashSet::new();
    match lambda_body {
        LambdaBodyTree::Block(b) => collect_tree_block_idents(body, *b, &mut syms),
        LambdaBodyTree::Expr(e) => collect_tree_expr_idents(body, *e, &mut syms),
    }
    for &p in params {
        syms.remove(&body.locals[p.0 as usize].name);
    }
    let mut ordered: Vec<scoop2_base::Symbol> = syms.into_iter().collect();
    ordered.sort();
    ordered
}

fn collect_tree_block_idents(
    body: &TreeBody,
    block: BlockId,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    let blk = &body.blocks[block.0 as usize];
    for &sid in &blk.stmts {
        collect_tree_stmt_idents(body, sid, syms);
    }
    if let Some(tail) = blk.tail {
        collect_tree_expr_idents(body, tail, syms);
    }
}

fn collect_tree_stmt_idents(
    body: &TreeBody,
    stmt_id: scoop2_hir::hir::tree::StmtId,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    match &body.stmts[stmt_id.0 as usize] {
        TreeStmt::Expr(e) => collect_tree_expr_idents(body, *e, syms),
        TreeStmt::LocalVal { local, init } => {
            // 扫描 init 后排除声明的绑定名（它是新局部，不是自由变量）。
            collect_tree_expr_idents(body, *init, syms);
            syms.remove(&body.locals[local.0 as usize].name);
        }
        TreeStmt::Destructure { pat, init, .. } => {
            collect_tree_expr_idents(body, *init, syms);
            remove_tree_pattern_binders(pat, body, syms);
        }
        TreeStmt::Assign { place, value } => {
            collect_tree_place_idents(body, place, syms);
            collect_tree_expr_idents(body, *value, syms);
        }
        TreeStmt::Return(v) => {
            if let Some(e) = v {
                collect_tree_expr_idents(body, *e, syms);
            }
        }
        TreeStmt::Break | TreeStmt::Continue => {}
    }
}

/// 赋值目标中的标识符（AST 路径 scan_assign_target_idents 的镜像：Ident 目标
/// 计入自由变量——对外层变量的赋值即捕获）。
fn collect_tree_place_idents(
    body: &TreeBody,
    place: &scoop2_hir::hir::tree::TreePlace,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    use scoop2_hir::hir::tree::TreePlace;
    match place {
        TreePlace::Local(local) => {
            syms.insert(body.locals[local.0 as usize].name);
        }
        TreePlace::TopLevelVar { fqn } => {
            syms.insert(*fqn);
        }
        TreePlace::MemberField { recv, .. } => collect_tree_expr_idents(body, *recv, syms),
    }
}

fn collect_tree_expr_idents(
    body: &TreeBody,
    expr: ExprId,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    match &body.exprs[expr.0 as usize].kind {
        TreeExprKind::Lit(_) => {}
        TreeExprKind::UnresolvedName { .. } => {}
        TreeExprKind::BoolNot { expr } => {
            collect_tree_expr_idents(body, *expr, syms);
        }
        TreeExprKind::UnresolvedCall { args } => {
            for &a in args {
                collect_tree_expr_idents(body, a, syms);
            }
        }
        TreeExprKind::LocalRef(local) => {
            syms.insert(body.locals[local.0 as usize].name);
        }
        // 顶层 val 引用在 AST 路径以 Ident 形态入集（随后被 symbol_locals 过滤）；
        // 插入 fqn 符号以镜像该集合语义（遮蔽形态的过捕获 quirk 一并保留）。
        TreeExprKind::TopLevelValRef { fqn } => {
            syms.insert(*fqn);
        }
        TreeExprKind::Call { callee, args, .. } => {
            collect_tree_callee_idents(body, callee, syms);
            for &arg in args {
                collect_tree_expr_idents(body, arg, syms);
            }
        }
        TreeExprKind::Member { recv, .. } | TreeExprKind::SafeMember { recv, .. } => {
            collect_tree_expr_idents(body, *recv, syms);
        }
        TreeExprKind::Block(b) => collect_tree_block_idents(body, *b, syms),
        TreeExprKind::If { cond, then, else_ } => {
            collect_tree_expr_idents(body, *cond, syms);
            collect_tree_expr_idents(body, *then, syms);
            if let Some(eb) = else_ {
                collect_tree_expr_idents(body, *eb, syms);
            }
        }
        TreeExprKind::While {
            cond,
            body: loop_body,
        } => {
            collect_tree_expr_idents(body, *cond, syms);
            collect_tree_block_idents(body, *loop_body, syms);
        }
        TreeExprKind::Tuple(els) | TreeExprKind::ArrayLit(els) => {
            for &e in els {
                collect_tree_expr_idents(body, e, syms);
            }
        }
        TreeExprKind::When { subject, arms } => {
            collect_tree_expr_idents(body, *subject, syms);
            for arm in arms {
                if let Some(guard) = arm.guard {
                    collect_tree_expr_idents(body, guard, syms);
                }
                collect_tree_expr_idents(body, arm.body, syms);
            }
        }
        TreeExprKind::Lambda {
            params, body: lb, ..
        } => {
            // 嵌套 lambda：收集其自由变量后减去其自身参数。
            for s in collect_tree_lambda_free_vars(body, params, lb) {
                syms.insert(s);
            }
        }
        TreeExprKind::Handle {
            body: hb, finally_, ..
        } => {
            collect_tree_block_idents(body, *hb, syms);
            if let Some(f) = finally_ {
                collect_tree_block_idents(body, *f, syms);
            }
        }
        TreeExprKind::WithUpdate { base, updates } => {
            collect_tree_expr_idents(body, *base, syms);
            for (_, v) in updates {
                collect_tree_expr_idents(body, *v, syms);
            }
        }
        TreeExprKind::LogicalAnd { lhs, rhs } | TreeExprKind::LogicalOr { lhs, rhs } => {
            collect_tree_expr_idents(body, *lhs, syms);
            collect_tree_expr_idents(body, *rhs, syms);
        }
        TreeExprKind::InterpolatedString { parts } => {
            for p in parts {
                if let scoop2_hir::hir::tree::InterpPart::Expr(e) = p {
                    collect_tree_expr_idents(body, *e, syms);
                }
            }
        }
        TreeExprKind::StructLit { fields, .. } => {
            for (_, v) in fields {
                collect_tree_expr_idents(body, *v, syms);
            }
        }
        TreeExprKind::Cast { expr, .. }
        | TreeExprKind::NotNullAssert { expr }
        | TreeExprKind::TypeCheck { expr, .. } => {
            collect_tree_expr_idents(body, *expr, syms);
        }
    }
}

/// callee 中的标识符（AST 路径扫描 callee 表达式的镜像：以 Ident/MemberAccess
/// 形态出现后经 symbol_locals 过滤——保留同样的集合语义）。
fn collect_tree_callee_idents(
    body: &TreeBody,
    callee: &TreeCallee,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    match callee {
        TreeCallee::TopLevel { fqn, .. } | TreeCallee::Ctor { type_fqn: fqn, .. } => {
            syms.insert(*fqn);
        }
        TreeCallee::Method { recv, .. } => collect_tree_expr_idents(body, *recv, syms),
        TreeCallee::Variant {
            enum_fqn,
            variant,
            qualified,
        } => {
            if *qualified {
                syms.insert(*enum_fqn);
            } else {
                syms.insert(*variant);
            }
        }
        TreeCallee::LocalValue { local } => {
            syms.insert(body.locals[local.0 as usize].name);
        }
        TreeCallee::FunValue { callee } => collect_tree_expr_idents(body, *callee, syms),
        TreeCallee::EffectOp { effect, .. } => {
            syms.insert(*effect);
        }
        TreeCallee::InitCall { .. } => {}
    }
}

/// 模式绑定名从自由变量集合中移除（AST 路径 remove_pattern_binders 的镜像）。
fn remove_tree_pattern_binders(
    pat: &TreePattern,
    body: &TreeBody,
    syms: &mut std::collections::HashSet<scoop2_base::Symbol>,
) {
    match pat {
        TreePattern::Wildcard
        | TreePattern::Else
        | TreePattern::Rest
        | TreePattern::Literal(_)
        | TreePattern::Is { .. } => {}
        TreePattern::Binder { local, .. } => {
            syms.remove(&body.locals[local.0 as usize].name);
        }
        TreePattern::Tuple(els) | TreePattern::Or(els) => {
            for e in els {
                remove_tree_pattern_binders(e, body, syms);
            }
        }
        TreePattern::Variant { args, .. } => {
            for a in args {
                remove_tree_pattern_binders(a, body, syms);
            }
        }
        TreePattern::Struct { fields } => {
            for f in fields {
                if let Some(local) = f.binder {
                    syms.remove(&body.locals[local.0 as usize].name);
                }
                if let Some(sub) = &f.sub {
                    remove_tree_pattern_binders(sub, body, syms);
                }
            }
        }
    }
}
