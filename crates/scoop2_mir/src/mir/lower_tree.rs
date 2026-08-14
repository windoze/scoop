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
    BlockId, ExprId, FnTree, TreeBody, TreeCallee, TreeExprKind, TreeMember, TreePattern,
    TreeStmt, WhenTreeArm,
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
            | TreeExprKind::When { .. } => {}

            TreeExprKind::Handle { .. } => return Some("Handle"),

            TreeExprKind::Lambda { .. } => return Some("Lambda"),
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

                TreeCallee::InitCall { .. } => return Some("InitCall"),
                TreeCallee::Ctor { .. } | TreeCallee::Variant { .. } => {}
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
                TreePlace::TopLevelVar { .. } => {
                    unsupported!("TopLevelVar place 在本切片支持集外")
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
            // 镜像 bind_pattern：模式解构绑定
            let v = lower_tree_expr(builder, body, *init);
            let init_ty = operand_ty_of(builder, &v);
            bind_tree_pattern(builder, body, pat, v, init_ty);
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
            let text = builder.hir.interner.resolve(*fqn).to_string();
            let tl = crate::mir::TopLevelRef {
                fqn: text,
                hidden_effects: scoop2_hir::ty::EffectRow::pure(),
                stable_template_key: None,
                stable_instance_key: None,
                generic_type_args: vec![],
                generic_eff_args: vec![],
            };
            let tmp = builder.alloc_temp(ty, span);
            builder.assign(tmp, Rvalue::TopLevelRef(tl), span);
            Operand::Local(tmp)
        }
        TreeExprKind::Call { callee, args } => {
            lower_tree_call(builder, body, callee, args, ty, span)
        }
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
    let call_site_id = Some(builder.next_site_id());
    let call_transport = builder.call_transport(ty);
    let mir_args: Vec<crate::mir::CallArg> = arg_ops
        .iter()
        .zip(args.iter())
        .map(|(op, &e)| crate::mir::CallArg {
            name: None,
            is_spread: false,
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
                // struct：StructLit（值语义）；字段名按 member_order 位置对应。
                let ordered_names: Vec<scoop2_base::Symbol> = builder
                    .hir
                    .member_order
                    .get(type_fqn)
                    .cloned()
                    .unwrap_or_default();
                let mut mir_fields: Vec<crate::mir::StructLitField> =
                    Vec::with_capacity(mir_args.len());
                for (i, arg) in mir_args.iter().enumerate() {
                    let name = ordered_names.get(i).copied().unwrap_or_default();
                    mir_fields.push(crate::mir::StructLitField {
                        name,
                        value: arg.value.clone(),
                        value_ty: arg.value_ty,
                    });
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
            let payload_component_tys: Vec<scoop2_hir::ty::TypeId> =
                arg_ops.iter().zip(args.iter()).map(|(op, e)| {
                    body.exprs[e.0 as usize].ty
                }).collect();
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

/// 从树构造完整 `FunDecl`（顶层函数 / 方法 / `$init`；val 初始化器树暂不支持——
/// 其签名语义由 Initializer item 承载，后续切片接入）。
///
/// 返回 `(FunDecl, 私有 store)`；嵌套闭包在直线子集下不产生。
pub fn lower_tree_fun_decl(
    hir: &scoop2_hir::hir::TypedHir,
    file_id: scoop2_base::FileId,
    tree: &FnTree,
    base_types: &scoop2_hir::ty::TypeStore,
) -> Option<(crate::mir::FunDecl, scoop2_hir::ty::TypeStore)> {
    let mut errors: Vec<crate::diagnostics::MirLowerError> = Vec::new();
    let mut types = base_types.clone();

    // 签名数据（return/effect/参数类型）：$init 合成 → unit/pure；其余按 FQN 查表。
    let (return_ty, effect_row): (scoop2_hir::ty::TypeId, scoop2_hir::ty::EffectRow) =
        if tree.fqn.ends_with(".$init") {
            (types.unit(), scoop2_hir::ty::EffectRow::pure())
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
            (sig.return_ty, sig.effect_row)
        };

    // fn_ty 的参数不含隐式 <this>（镜像 AST：fd.params 事后追加 this）。
    let param_tys: Vec<scoop2_hir::ty::TypeId> = tree
        .params
        .iter()
        .filter(|&p| builder_this_check(hir, tree.body.locals[p.0 as usize].name))
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
    let (body, _nested, types_out) = builder.finish();
    fd.body = Some(body);
    Some((fd, types_out))
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
            bind_tree_pattern(builder, body, &arm.pat, subj.clone(), subj_ty);
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
            bind_tree_pattern(builder, body, &arm.pat, subj.clone(), subj_ty);
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
fn lower_tree_pattern_test(
    builder: &mut FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
) -> Operand {
    let bool_ty = builder.types.bool();
    match pat {
        TreePattern::Wildcard | TreePattern::Else => Operand::Const(ConstValue::Bool(true)),
        TreePattern::Binder(local) => {
            // irrefutable 模式：类型已由 typecheck 保证匹配 → 总是命中
            let _ = local;
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

                let elem_ty = tree_tuple_elem_ty(builder, subj_ty, i).unwrap_or_else(|| builder.types.any());
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
            builder.assign(result, Rvalue::Use(prev_test.expect("testable 非空")), subj_ty_of(builder, &subj));
            builder.goto(merge_bb, subj_ty_of(builder, &subj));
            builder.current_bb = merge_bb;
            Operand::Local(result)
        }
        TreePattern::Variant { enum_fqn, variant, args } => {
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
                    TreePattern::Binder(_) | TreePattern::Wildcard => {
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
                let field_ty = tree_variant_payload_field_ty(builder, subj_ty, enum_fqn, i);
                prev_test = if let Some(field_ty) = field_ty {
                    let tmp = builder.alloc_temp(field_ty, subj_ty_of(builder, &subj));
                    let variant_str = builder.hir.interner.resolve(*variant).to_string();
                    builder.assign(
                        tmp,
                        Rvalue::PatternExtract {
                            subject: subj.clone(),
                            path: vec![
                                crate::mir::transport::PatternBindingStep::VariantField {
                                    variant: variant_str,
                                    field_index: i,
                                },
                            ],
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

/// 为树模式引入绑定（树版本）。
fn bind_tree_pattern(
    builder: &mut FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
    subj: Operand,
    subj_ty: scoop2_hir::ty::TypeId,
) {
    match pat {
        TreePattern::Wildcard | TreePattern::Else => {}
        TreePattern::Binder(local) => {
            let decl = &body.locals[local.0 as usize];
            let lid = builder.alloc_named(
                builder.hir.interner.resolve(decl.name).to_string(),
                decl.ty,
                decl.span,
            );
            builder.symbol_locals.insert(decl.name, lid);
            builder.assign(lid, Rvalue::Use(subj), decl.span);
        }
        TreePattern::Tuple(elems) => {
            // tuple 元素绑定：按元素位置提取
            for (i, elem) in elems.iter().enumerate() {
                if let TreePattern::Binder(local) = elem {
                    let decl = &body.locals[local.0 as usize];
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(decl.name).to_string(),
                        decl.ty,
                        decl.span,
                    );
                    builder.symbol_locals.insert(decl.name, lid);

                    let elem_ty = tree_tuple_elem_ty(builder, subj_ty, i).unwrap_or_else(|| builder.types.any());
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
                                path: vec![crate::mir::transport::PatternBindingStep::TupleIndex(i)],
                                result_ty: elem_ty,
                            },
                            subj_ty_of(builder, &subj),
                        );
                        bind_tree_pattern(builder, body, elem, Operand::Local(tmp), elem_ty);
                    }
                }
            }
        }
        TreePattern::Variant { enum_fqn, variant, args } => {
            // variant 字段绑定：按字段位置提取
            let variant_str = builder.hir.interner.resolve(*variant).to_string();

            for (i, arg) in args.iter().enumerate() {
                if let TreePattern::Binder(local) = arg {
                    let decl = &body.locals[local.0 as usize];
                    let lid = builder.alloc_named(
                        builder.hir.interner.resolve(decl.name).to_string(),
                        decl.ty,
                        decl.span,
                    );
                    builder.symbol_locals.insert(decl.name, lid);

                    if let Some(field_ty) = tree_variant_payload_field_ty(builder, subj_ty, enum_fqn, i) {
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
                    } else {
                        builder.assign(lid, Rvalue::Use(subj.clone()), decl.span);
                    }
                }
            }

            // 嵌套子模式：按字段位置提取后递归绑定
            for (i, arg) in args.iter().enumerate() {
                if matches!(
                    arg,
                    TreePattern::Variant { .. } | TreePattern::Tuple(_) | TreePattern::Or(_)
                ) {
                    if let Some(field_ty) = tree_variant_payload_field_ty(builder, subj_ty, enum_fqn, i) {
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
                        bind_tree_pattern(builder, body, arg, Operand::Local(tmp), field_ty);
                    }
                }
            }
        }
        TreePattern::Literal(_) | TreePattern::Is { .. } => {}
        TreePattern::Or(alts) => {
            // or 模式的绑定只在第一个 alt 中引入（AST 路径的行为）
            if let Some(first) = alts.first() {
                bind_tree_pattern(builder, body, first, subj, subj_ty);
            }
        }
    }
}

/// 树模式 → MIR 模式（用于 PatternMatch）。
fn lower_tree_pattern_to_mir(
    builder: &FnLowering,
    body: &TreeBody,
    pat: &TreePattern,
) -> crate::mir::Pattern {
    match pat {
        TreePattern::Wildcard => crate::mir::Pattern::Wildcard,
        TreePattern::Binder(local) => {
            let decl = &body.locals[local.0 as usize];
            crate::mir::Pattern::Bind {
                name: decl.name,
                ty: decl.ty,
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
    enum_fqn: &str,
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
            let vfqn = builder.hir.interner.get(enum_fqn)?;
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
