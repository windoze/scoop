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
    BlockId, ExprId, FnTree, TreeBody, TreeCallee, TreeExprKind, TreeMember, TreeStmt,
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
            | TreeExprKind::While { .. } => {}

            TreeExprKind::When { .. } => return Some("When"),
            TreeExprKind::Handle { .. } => return Some("Handle"),
            TreeExprKind::WithUpdate { .. } => return Some("WithUpdate"),
            TreeExprKind::InterpolatedString { .. } => return Some("InterpolatedString"),
            TreeExprKind::ArrayLit(_) => return Some("ArrayLit"),
            TreeExprKind::StructLit { .. } => return Some("StructLit"),
            TreeExprKind::Lambda { .. } => return Some("Lambda"),
            TreeExprKind::Cast { .. } => return Some("Cast"),
            TreeExprKind::TypeCheck { .. } => return Some("TypeCheck"),
        }
    }
    for e in &tree.body.exprs {
        if let TreeExprKind::Call { callee, .. } = &e.kind {
            match callee {
                TreeCallee::TopLevel { .. }
                | TreeCallee::LocalValue { .. }
                | TreeCallee::FunValue { .. } => {}
                // Method 仅支持 direct（非虚非接口）——分派元数据复刻在后续切片。
                TreeCallee::Method {
                    is_virtual,
                    is_interface,
                    ..
                } if !is_virtual && !is_interface => {}
                TreeCallee::Method { .. } => return Some("Method-dispatch"),

                TreeCallee::EffectOp { .. } => return Some("EffectOp"),
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
            TreeStmt::Destructure { .. } => return Some("Destructure"),
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
        TreeStmt::Destructure { .. } => {
            unsupported!("Destructure 语句在支持集外")
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
            owner_fqn,
            method,
            type_args,
            param_types,
            ..
        } => {
            // direct 方法调用：receiver 前置（镜像 AST 路径的 final_args 构造）。
            let recv_op = recv_op.expect("Method 分支已预 lower receiver");
            let recv_ty = operand_ty_of(builder, &recv_op);
            let owner_str = builder.hir.interner.resolve(*owner_fqn).to_string();
            let method_str = builder.hir.interner.resolve(*method).to_string();
            let mut final_args = Vec::with_capacity(mir_args.len() + 1);
            final_args.push(crate::mir::CallArg {
                name: None,
                is_spread: false,
                value: recv_op,
                value_ty: recv_ty,
            });
            final_args.extend(mir_args);
            Rvalue::Call {
                site_id: call_site_id,
                kind: builder.make_direct_call_kind_with_params(
                    format!("{owner_str}.{method_str}"),
                    type_args.clone(),
                    false,
                    Some(param_types),
                ),
                args: final_args,
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
        TreeCallee::EffectOp { .. } | TreeCallee::InitCall { .. } => {
            unsupported!("effect/init 调用在支持集外")
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
