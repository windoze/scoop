//! scoop2_lir：MIR 和 codegen 之间的准备层。

pub mod abi;
pub mod dispatch;
pub mod dump;
pub mod effect;
pub mod gc;
pub mod global_init;
pub mod layout;
pub mod program;
pub mod verify;

pub use program::*;

use scoop2_base::Interner;
use scoop2_hir::hir::TypedHir;
use scoop2_mir::mir::materialize::MaterializedMir;

/// LIR lowering 错误（字段解析失败等不可恢复不一致）。
///
/// 约定：LIR lowering 对"类型已知但成员解析失败"这类编译器内部不一致
/// 必须返回错误，绝不静默回退（静默 0 偏移曾导致字段读写错位）。
#[derive(Clone, Debug)]
pub struct LirError {
    pub message: String,
}

impl LirError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LIR lowering 失败：{}", self.message)
    }
}

impl std::error::Error for LirError {}

/// 主入口：将 MaterializedMir + TypedHir 降级为 LirProgram。
pub fn lower_to_lir(
    mir: &MaterializedMir,
    hir: &TypedHir,
    interner: &Interner,
) -> Result<LirProgram, LirError> {
    let mut program = LirProgram::new();
    // 1. 类型布局计算
    layout::compute_type_layouts(&mut program, mir, hir, interner);
    // 2. 分发表生成
    dispatch::generate_dispatch_tables(&mut program, mir, hir, interner);
    // 3. ABI 决策
    abi::decide_abi(&mut program, mir, hir, interner);
    // 4. GC 类型描述符
    gc::generate_gc_info(&mut program, mir, hir, interner);
    // 5. Effect step 合成类型布局
    effect::prepare_effect_steps(&mut program, mir, hir, interner);
    // 6. 全局初始化规划
    global_init::plan_global_init(&mut program, mir, hir, interner);
    // 顶层 val/var 的 fqn → 真实类型映射（供 TopLevelRef.ty 查真实类型）。
    let global_types: std::collections::HashMap<String, scoop2_hir::ty::TypeId> = program
        .global_init
        .entries
        .iter()
        .map(|e| (e.fqn.clone(), e.ty))
        .collect();
    // 7. MIR→LIR body 映射（含 GC info + effect schema 挂载）
    let step_tags = build_step_tag_tables(mir, interner);
    map_bodies(&mut program, mir, hir, interner, &step_tags, &global_types)?;
    // 8. 回填分发表信息到调用点
    dispatch::backfill_call_sites(&mut program);
    // 9. 验证
    verify::verify_lir(&program);
    Ok(program)
}

// =========================================================================
// 合成 Step enum 的 tag 表
// =========================================================================

/// 合成 Step enum 的变体判别值表。
///
/// Step enum 是 effect lowering 的合成类型，不在 HIR `enum_variants` 注册表中，
/// 通用 tag 解析路径（HIR 查表）对它会静默回退 0/None。这里从各 EffectStep
/// 函数的 `EffectStepAbi.step_variants`（声明序 = tag）直接构建权威映射。
#[derive(Default)]
struct StepTagTables {
    /// (step_ty, variant_name_sym) → tag：用于 `Rvalue::EnumVariant` 构造点。
    rvalue_tags: std::collections::HashMap<(scoop2_hir::ty::TypeId, scoop2_base::Symbol), u64>,
    /// (enum_fqn_sym, variant_name_sym) → tag：用于 `Pattern::Variant` 匹配点
    ///（enum_fqn_sym = 合成 Step 所属函数的 FQN Symbol）。
    pattern_tags: std::collections::HashMap<(scoop2_base::Symbol, scoop2_base::Symbol), u64>,
}

/// 从 MaterializedMir 中所有 EffectStep 函数构建 tag 表。
fn build_step_tag_tables(mir: &MaterializedMir, interner: &Interner) -> StepTagTables {
    let mut tables = StepTagTables::default();
    for item in &mir.module.items {
        let scoop2_mir::mir::Item::Fun(fd) = item else {
            continue;
        };
        let Some(abi) = &fd.effect_abi else {
            continue;
        };
        let fn_sym = interner.get(&fd.fqn).unwrap_or_default();
        for (i, v) in abi.step_variants.iter().enumerate() {
            tables
                .rvalue_tags
                .insert((abi.step_ty, v.name_sym), i as u64);
            tables.pattern_tags.insert((fn_sym, v.name_sym), i as u64);
        }
        // call-chain 站点的 Step 变体表：间接调用站点（FunValue/Closure）使用
        // 站点级合成 Step 类型（step_fqn_sym 为 default Symbol），其 PatternMatch
        // 的 (enum_fqn_sym, variant_sym) 键不在任何函数的 step_variants 里，
        // 必须按站点变体表登记（声明序 = tag；Direct 站点与 callee 自身登记
        // 键值相同，重复 insert 幂等）。
        for site in &abi.call_chain_sites {
            for (i, v) in site.step_variants.iter().enumerate() {
                tables
                    .rvalue_tags
                    .insert((site.step_ty, v.name_sym), i as u64);
                tables
                    .pattern_tags
                    .insert((site.step_fqn_sym, v.name_sym), i as u64);
            }
        }
    }
    tables
}

// =========================================================================
// MIR→LIR body 映射
// =========================================================================

fn map_bodies(
    program: &mut LirProgram,
    mir: &MaterializedMir,
    hir: &TypedHir,
    interner: &Interner,
    step_tags: &StepTagTables,
    global_types: &std::collections::HashMap<String, scoop2_hir::ty::TypeId>,
) -> Result<(), LirError> {
    for item in &mir.module.items {
        match item {
            scoop2_mir::mir::Item::Fun(fd) => {
                if fd.body.is_some() {
                    // 有函数体：放入 callables。
                    program.callables.push(map_callable(
                        fd,
                        mir,
                        hir,
                        &program.type_layouts,
                        interner,
                        step_tags,
                        global_types,
                    )?);
                } else {
                    // 无函数体（extern / abstract / intrinsic）：放入 declarations。
                    let symbol_name = &fd.fqn;
                    // 区分 @Intrinsic（内联，不声明 extern 符号）与 @Extern（运行时符号）。
                    // 仅 @Extern 才 is_extern=true；@Intrinsic 走 codegen 内联路径。
                    let is_intrinsic = fd.intrinsic_name.is_some();
                    program.declarations.push(LirDeclaration {
                        fqn: fd.fqn.clone(),
                        symbol_name: symbol_name.clone(),
                        params: fd
                            .params
                            .iter()
                            .map(|p| LirParam {
                                name: p.name.clone(),
                                ty: p.ty,
                                abi: abi::param_abi_for_type(p.ty, &program.type_layouts),
                                local_id: p.local.0,
                            })
                            .collect(),
                        return_ty: fd.return_ty,
                        return_abi: abi::param_abi_for_type(fd.return_ty, &program.type_layouts),
                        is_extern: !is_intrinsic,
                        extern_symbol: if is_intrinsic {
                            None
                        } else {
                            Some(symbol_name.clone())
                        },
                        intrinsic_name: fd.intrinsic_name.clone(),
                    });
                }
            }
            scoop2_mir::mir::Item::Initializer(ir) => {
                program.callables.push(map_initializer(
                    ir,
                    &program.type_layouts,
                    &mir.module.types,
                    hir,
                    interner,
                    step_tags,
                    global_types,
                )?);
            }
            scoop2_mir::mir::Item::ExternGlobal(eg) => {
                program.declarations.push(LirDeclaration {
                    fqn: eg.fqn.clone(),
                    symbol_name: eg.fqn.clone(),
                    params: Vec::new(),
                    return_ty: eg.ty,
                    return_abi: ParamAbi::Direct,
                    is_extern: true,
                    extern_symbol: Some(eg.fqn.clone()),
                    intrinsic_name: None,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn map_callable(
    fd: &scoop2_mir::mir::FunDecl,
    mir: &MaterializedMir,
    hir: &TypedHir,
    layouts: &TypeLayoutTable,
    interner: &Interner,
    step_tags: &StepTagTables,
    global_types: &std::collections::HashMap<String, scoop2_hir::ty::TypeId>,
) -> Result<LirCallable, LirError> {
    let symbol_name = fd
        .instance_symbol
        .clone()
        .unwrap_or_else(|| abi::mangle_symbol(&fd.fqn, &fd.stable_template_key));
    let abi_kind = if fd.effect_abi.is_some() {
        LirCallableAbi::EffectStep
    } else {
        LirCallableAbi::Plain
    };
    let params: Vec<LirParam> = fd
        .params
        .iter()
        .map(|p| LirParam {
            name: p.name.clone(),
            ty: p.ty,
            abi: abi::param_abi_for_type(p.ty, layouts),
            local_id: p.local.0,
        })
        .collect();
    let return_abi = abi::param_abi_for_type(fd.return_ty, layouts);

    // 计算函数体的 GC info 和 effect schema。
    let (body, gc_info, frame_schema, step_layout, state_dispatch, continuation_layout, effect_info) =
        if let Some(mir_body) = &fd.body {
            let frame_local = fd.effect_abi.as_ref().map(|ea| ea.frame_local);
            let lir_body = map_body(
                mir_body,
                layouts,
                &mir.module.types,
                hir,
                interner,
                step_tags,
                frame_local,
                global_types,
            )?;
            let frame_for_gc = fd
                .effect_abi
                .as_ref()
                .map(|ea| (ea.frame_ty, ea.frame_local));
            let gc = gc::compute_gc_info_for_body(mir_body, &fd.fqn, layouts, frame_for_gc);
            let (fs, sl, sd, cl) = if let Some(ref eff_abi) = fd.effect_abi {
                effect::prepare_effect_abi(eff_abi, &fd.fqn, layouts, hir, interner)
            } else {
                (None, None, None, None)
            };
            let ei = fd.effect_abi.as_ref().map(|ea| LirEffectInfo {
                frame_local: ea.frame_local.0,
                frame_ty: ea.frame_ty,
                param_slots: fd
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.local.0, (i + 1) as u64))
                    .collect(),
                resume_points: ea
                    .resume_points
                    .iter()
                    .map(|rp| LirResumePoint {
                        state: rp.state as u64,
                        block_id: rp.block.0,
                        resume_local: rp.resume_local.0,
                        resume_ty: rp.resume_ty,
                    })
                    .collect(),
                step_ty: ea.step_ty,
            });
            (Some(lir_body), Some(gc), fs, sl, sd, cl, ei)
        } else {
            (None, None, None, None, None, None, None)
        };

    Ok(LirCallable {
        fqn: fd.fqn.clone(),
        symbol_name,
        abi: abi_kind,
        params,
        return_ty: fd.return_ty,
        return_abi,
        body,
        gc_info,
        frame_schema,
        step_layout,
        state_dispatch,
        continuation_layout,
        effect_info,
    })
}

fn map_initializer(
    ir: &scoop2_mir::mir::InitializerRoot,
    layouts: &TypeLayoutTable,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    step_tags: &StepTagTables,
    global_types: &std::collections::HashMap<String, scoop2_hir::ty::TypeId>,
) -> Result<LirCallable, LirError> {
    let symbol_name = abi::mangle_symbol(&ir.fqn, &None);
    let body = map_body(&ir.body, layouts, types, hir, interner, step_tags, None, global_types)?;
    Ok(LirCallable {
        fqn: ir.fqn.clone(),
        symbol_name,
        abi: LirCallableAbi::Plain,
        params: Vec::new(),
        return_ty: ir.ty,
        return_abi: ParamAbi::Direct,
        body: Some(body),
        gc_info: None,
        frame_schema: None,
        step_layout: None,
        state_dispatch: None,
        continuation_layout: None,
        effect_info: None,
    })
}

fn map_body(
    body: &scoop2_mir::mir::Body,
    layouts: &TypeLayoutTable,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    step_tags: &StepTagTables,
    frame_local: Option<scoop2_mir::mir::LocalId>,
    global_types: &std::collections::HashMap<String, scoop2_hir::ty::TypeId>,
) -> Result<LirBody, LirError> {
    let locals = body
        .locals
        .iter()
        .enumerate()
        .map(|(i, d)| LirLocalDecl {
            id: i as u32,
            name: d.name.clone(),
            ty: d.ty,
            mutable: d.mutable,
            // EffectStep 的 frame local 持有堆分配的 GC 对象指针（声明类型是
            // frame tuple 值类型，is_gc_traceable_type 判不出），强制标记为
            // GC root，使其进入 root frame slot（load/store 走 slot 重载协议）。
            gc_traceable: gc::is_gc_traceable_type(d.ty, layouts)
                || frame_local.is_some_and(|fl| fl.0 as usize == i),
        })
        .collect();
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for (bi, blk) in body.blocks.iter().enumerate() {
        let mut stmts = Vec::with_capacity(blk.stmts.len());
        for s in &blk.stmts {
            stmts.push(map_stmt(s, layouts, types, hir, interner, step_tags, global_types)?);
        }
        blocks.push(LirBlock {
            id: bi as u32,
            stmts,
            terminator: map_term(&blk.terminator.kind),
        });
    }
    Ok(LirBody {
        locals,
        blocks,
        start_block: body.start.0,
    })
}

fn map_stmt(
    stmt: &scoop2_mir::mir::Statement,
    layouts: &TypeLayoutTable,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    step_tags: &StepTagTables,
    global_types: &std::collections::HashMap<String, scoop2_hir::ty::TypeId>,
) -> Result<LirStmt, LirError> {
    use scoop2_mir::mir::StatementKind;
    let kind = match &stmt.kind {
        StatementKind::Nop => LirStmtKind::Nop,
        StatementKind::Assign { target, value } => LirStmtKind::Assign {
            target: target.0,
            value: map_rvalue(value, layouts, types, hir, interner, step_tags, global_types)?,
        },
        StatementKind::StoreMember {
            receiver,
            member,
            value,
            value_ty,
            ..
        } => {
            let receiver_ty = member.receiver_ty;
            LirStmtKind::StoreMember {
                receiver_local: map_operand(receiver),
                receiver_ty,
                member_name: member.name.clone(),
                field_offset: compute_field_offset(
                    receiver_ty,
                    &member.name,
                    layouts,
                    types,
                    hir,
                    interner,
                )?,
                value_local: map_operand(value),
                value_ty: *value_ty,
            }
        }
        StatementKind::StoreTupleIndex {
            receiver,
            index,
            value,
            value_ty,
        } => LirStmtKind::StoreTupleIndex {
            receiver_local: map_operand(receiver),
            index: *index,
            value_local: map_operand(value),
            value_ty: *value_ty,
        },
        StatementKind::StoreTopLevelVar {
            fqn,
            value,
            value_ty,
        } => LirStmtKind::StoreGlobal {
            global_fqn: interner.resolve(*fqn).to_string(),
            value_local: map_operand(value),
            value_ty: *value_ty,
        },
        StatementKind::Panic { message } => LirStmtKind::Panic {
            message: message.clone(),
        },
    };
    Ok(LirStmt {
        span: stmt.span,
        kind,
    })
}

fn map_rvalue(
    rv: &scoop2_mir::mir::Rvalue,
    layouts: &TypeLayoutTable,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    step_tags: &StepTagTables,
    global_types: &std::collections::HashMap<String, scoop2_hir::ty::TypeId>,
) -> Result<LirRvalue, LirError> {
    use scoop2_mir::mir::{CallKind, Operand, Rvalue};
    let out = match rv {
        Rvalue::Use(op) => match op {
            Operand::Local(lid) => LirRvalue::Use(lid.0),
            Operand::Const(c) => LirRvalue::Const(map_const(c)),
        },
        Rvalue::TopLevelRef(tl) => LirRvalue::TopLevelRef {
            fqn: tl.fqn.clone(),
            // 真实全局类型：优先从 global_types 按 fqn 查（权威来源）；
            // 否则回退 generic_type_args / find_any_type。
            ty: global_types
                .get(&tl.fqn)
                .copied()
                .or_else(|| tl.generic_type_args.first().copied())
                .unwrap_or_else(|| find_any_type(types)),
        },
        Rvalue::UnresolvedName { name } => LirRvalue::TopLevelRef {
            fqn: format!("<unresolved:{}>", name),
            ty: find_any_type(types),
        },
        Rvalue::TypeTest {
            value, metadata, ..
        } => LirRvalue::TypeTest {
            value_local: map_operand(value),
            target_ty: metadata.target_ty,
            static_fold: metadata.static_fold,
            descriptor: metadata.descriptor.clone(),
        },
        Rvalue::Cast {
            value, metadata, ..
        } => match &metadata.result {
            scoop2_mir::mir::transport::RuntimeCastResult::Target { ty } => LirRvalue::Cast {
                value_local: map_operand(value),
                target_ty: *ty,
                descriptor: metadata.test.descriptor.clone(),
                failure: metadata.failure.clone(),
            },
            scoop2_mir::mir::transport::RuntimeCastResult::Option { option_ty, .. } => {
                LirRvalue::Cast {
                    value_local: map_operand(value),
                    target_ty: *option_ty,
                    descriptor: metadata.test.descriptor.clone(),
                    failure: metadata.failure.clone(),
                }
            }
        },
        Rvalue::MemberAccess {
            receiver, member, ..
        } => {
            let receiver_ty = member.receiver_ty;
            // 从 HIR members 表查得成员的真实声明类型，而非用 receiver_ty 占位。
            // 旧实现把 result_ty 设为 receiver_ty，对 codegen 解析成员类型是错误的：
            // 例如 `animal.sound` 的结果应是 Int，而非 Animal 引用类型本身。
            let result_ty = {
                use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
                let fqn = match types.kind(member.receiver_ty) {
                    TypeKind::Ref(RefTypeKind::Nominal(n)) => Some(n.fqn),
                    TypeKind::Value(ValueTypeKind::Nominal(n)) => Some(n.fqn),
                    _ => None,
                };
                let member_sym = interner.get(&member.name);
                // ordered_class_fields 覆盖自身 members 且沿超类链含继承字段
                // （class 自身无字段时 members 无条目，仅查 members 会漏掉继承字段）。
                fqn.and_then(|f| {
                    member_sym.and_then(|s| {
                        hir.ordered_class_fields(f)
                            .into_iter()
                            .find(|(n, _)| *n == s)
                            .map(|(_, t)| t)
                    })
                })
                .unwrap_or(member.receiver_ty)
            };
            LirRvalue::MemberAccess {
                receiver_local: map_operand(receiver),
                receiver_ty,
                member_name: member.name.clone(),
                field_offset: compute_field_offset(
                    receiver_ty,
                    &member.name,
                    layouts,
                    types,
                    hir,
                    interner,
                )?,
                result_ty,
            }
        }
        Rvalue::TupleIndex {
            receiver,
            index,
            element_ty,
        } => LirRvalue::TupleIndex {
            receiver_local: map_operand(receiver),
            index: *index,
            element_ty: *element_ty,
        },
        Rvalue::IndexAccess {
            receiver,
            indices,
            element_ty,
            receiver_ty,
        } => LirRvalue::IndexAccess {
            receiver_local: map_operand(receiver),
            index_locals: indices.iter().map(map_operand).collect(),
            element_ty: *element_ty,
            receiver_mutable: is_mutable_array_ty(*receiver_ty, types, interner),
        },
        Rvalue::EnumVariant {
            enum_ty,
            enum_fqn,
            variant_name,
            args,
            payload,
            ..
        } => {
            let vname = interner.resolve(*variant_name).to_string();
            // 从 HIR enum_variants 查找变体序号作为 tag_value。
            // 优先用 MIR 携带的 enum_fqn（对内建 Option<T> 等非 Nominal 的
            // enum_ty 同样成立——Option 的 enum_ty 是 Value(Option(_))，
            // 走 Nominal 分支会漏掉，导致 Some/None tag 都为 0）。
            let tag_from = |fqn: &scoop2_base::Symbol| -> Option<u64> {
                hir.enum_variants.get(fqn).and_then(|variants| {
                    variants
                        .iter()
                        .position(|&v| interner.resolve(v) == vname)
                        .map(|i| i as u64)
                })
            };
            // 合成 Step enum（MIR effect lowering 产物）不在 HIR enum_variants 中，
            // tag 取 effect_abi.step_variants 声明序，见 NEW-LLVM-CODEGEN.md §3.1。
            let step_tag = step_tags.rvalue_tags.get(&(*enum_ty, *variant_name)).copied();
            let tag_value = step_tag.or_else(|| {
                tag_from(enum_fqn).or_else(|| {
                    if let scoop2_hir::ty::TypeKind::Value(
                        scoop2_hir::ty::ValueTypeKind::Nominal(n),
                    ) = types.kind(*enum_ty)
                    {
                        tag_from(&n.fqn)
                    } else {
                        None
                    }
                })
            });
            let tag_value = match tag_value {
                Some(t) => t,
                None => {
                    // enum_variants 中有该 enum 但找不到变体 = 编译器内部不一致，报错；
                    // enum 完全未注册（外部/未知 enum）保持旧回退 0。
                    let known = hir.enum_variants.contains_key(enum_fqn);
                    if known {
                        return Err(LirError::new(format!(
                            "EnumVariant {}.{} 在 enum_variants 中找不到变体序号",
                            interner.resolve(*enum_fqn),
                            vname
                        )));
                    }
                    0
                }
            };
            // payload 类型：用户 enum 的单字段 variant 以 HIR 登记的字段类型
            // 为准（`<enum_fqn>.<variant>` members，与布局计算的 payload 类型
            // 一致）；多字段 variant 无单一 payload 类型（codegen 按布局的
            // payload_fields 逐字段写入，不得回退 aggregate_ty——那会把首实参
            // 按 enum 自身类型误写）；其余（合成 Step enum / 内建 Option 等）
            // 从 transport 的 aggregate 获取。
            let variant_fields: Vec<scoop2_hir::ty::TypeId> = (|| {
                let variant_fqn_text = format!("{}.{}", interner.resolve(*enum_fqn), vname);
                let vf = interner.get(&variant_fqn_text)?;
                Some(
                    hir.ordered_members(&vf)
                        .into_iter()
                        .map(|(_, t)| t)
                        .collect(),
                )
            })()
            .unwrap_or_default();
            let payload_ty = if args.is_empty() {
                None
            } else if variant_fields.len() == 1 {
                Some(variant_fields[0])
            } else if variant_fields.len() > 1 {
                None
            } else {
                Some(payload.aggregate_ty)
            };
            LirRvalue::EnumVariant {
                enum_ty: *enum_ty,
                variant_name: vname,
                tag_value,
                args: args.iter().map(|a| map_operand(&a.value)).collect(),
                payload_ty,
            }
        }
        Rvalue::ClassCtor { type_fqn, args, .. } => LirRvalue::ClassCtor {
            class_fqn: interner.resolve(*type_fqn).to_string(),
            args: args.iter().map(|a| map_operand(&a.value)).collect(),
        },
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => {
            let ck = match kind {
                CallKind::Direct {
                    callee_fqn,
                    stable_instance_key,
                    generic_type_args,
                    ..
                } => {
                    // 对带类型实参的 Direct 调用，解析到目标实例的唯一符号名
                    // （与 materialize 的 compute_instance_symbol 同公式），确保
                    // 同 FQN 不同实参的重载（println<Int>/println<String>）指向
                    // 正确实例。无类型实参时保留 FQN（由 backfill/codegen 解析）。
                    let callee_symbol = if generic_type_args.is_empty() {
                        callee_fqn.clone()
                    } else {
                        scoop2_mir::mir::materialize::compute_instance_symbol(
                            callee_fqn,
                            generic_type_args,
                            types,
                            interner,
                        )
                    };
                    LirCallKind::Direct {
                        callee_symbol,
                        callee_fqn: callee_fqn.clone(),
                        stable_instance_key: stable_instance_key.clone(),
                    }
                }
                CallKind::Virtual {
                    receiver, dispatch, ..
                } => LirCallKind::Virtual {
                    receiver_local: map_operand(receiver),
                    owner_fqn: dispatch.owner_fqn.clone(),
                    method_name: dispatch.member_name.clone(),
                    vtable_slot: 0,
                },
                CallKind::Interface {
                    receiver, dispatch, ..
                } => LirCallKind::Interface {
                    receiver_local: map_operand(receiver),
                    interface_fqn: dispatch.owner_fqn.clone(),
                    method_name: dispatch.member_name.clone(),
                    interface_id: 0,
                    itable_slot: 0,
                },
                CallKind::Closure { callee, .. } => LirCallKind::Closure {
                    callee_local: map_operand(callee),
                },
                CallKind::FunValue { callee } => LirCallKind::FunValue {
                    callee_local: map_operand(callee),
                },
                CallKind::Resume {
                    continuation,
                    resume_value,
                    ..
                } => LirCallKind::Resume {
                    continuation: map_operand(continuation),
                    resume_value: map_operand(resume_value),
                },
            };
            LirRvalue::Call(LirCall {
                kind: ck,
                args: args.iter().map(|a| map_operand(&a.value)).collect(),
                result_ty: transport.result.source_ty,
            })
        }
        Rvalue::MakeTuple {
            elements,
            transport,
        } => LirRvalue::MakeTuple {
            elements: elements.iter().map(map_operand).collect(),
            ty: transport.aggregate_ty,
        },
        Rvalue::MakeArray {
            elements,
            result_ty,
        } => LirRvalue::MakeArray {
            elements: elements.iter().map(map_operand).collect(),
            ty: *result_ty,
            mutable: is_mutable_array_ty(*result_ty, types, interner),
        },
        Rvalue::StructLit {
            type_fqn,
            fields,
            transport,
        } => LirRvalue::StructLit {
            type_fqn: interner.resolve(*type_fqn).to_string(),
            fields: fields
                .iter()
                .map(|f| (interner.resolve(f.name).to_string(), map_operand(&f.value)))
                .collect(),
            ty: transport.aggregate_ty,
        },
        Rvalue::InterpolatedString { parts } => LirRvalue::InterpolatedString {
            parts: parts
                .iter()
                .map(|p| match p {
                    scoop2_mir::mir::InterpolatedPart::Lit(s) => {
                        LirInterpolatedPart::Lit(s.clone())
                    }
                    scoop2_mir::mir::InterpolatedPart::Expr(op) => {
                        LirInterpolatedPart::Expr(map_operand(op))
                    }
                })
                .collect(),
        },
        Rvalue::WithUpdate {
            base,
            updates,
            result_ty,
        } => LirRvalue::WithUpdate {
            base_local: map_operand(base),
            updates: updates
                .iter()
                .map(|u| {
                    let (variant, path) = resolve_with_update_path(
                        *result_ty, &u.path, layouts, types, hir, interner,
                    )?;
                    Ok(LirWithUpdateField {
                        variant,
                        path,
                        value: map_operand(&u.value),
                        value_ty: u.value_ty,
                    })
                })
                .collect::<Result<Vec<_>, LirError>>()?,
            result_ty: *result_ty,
        },
        Rvalue::MakeClosure {
            env, invoke_fqn, ..
        } => LirRvalue::MakeClosure {
            env_local: map_operand(env),
            invoke_fqn: invoke_fqn.clone(),
        },
        Rvalue::ClassLit { type_fqn } => LirRvalue::ClassLit {
            type_fqn: interner.resolve(*type_fqn).to_string(),
        },
        Rvalue::PerformResult { .. } => {
            // PerformResult 是 effect lowering 前的占位值，lowering 后不应出现。
            // 映射为 Unit 常量（其类型信息已在 effect lowering 中处理）。
            LirRvalue::Const(LirConstValue::Unit)
        }
        Rvalue::PatternMatch { subject, pattern } => LirRvalue::PatternMatch {
            subject_local: map_operand(subject),
            pattern: map_pattern(pattern, types, hir, interner, step_tags),
        },
        Rvalue::PatternExtract {
            subject,
            path,
            result_ty,
        } => LirRvalue::PatternExtract {
            subject_local: map_operand(subject),
            path: path.clone(),
            result_ty: *result_ty,
        },
        Rvalue::IntEq { lhs, rhs } => LirRvalue::IntEq {
            lhs_local: map_operand(lhs),
            rhs_local: map_operand(rhs),
        },
        Rvalue::MakeContinuation { state } => LirRvalue::MakeContinuation {
            state: *state as u64,
        },
        Rvalue::MakeChainLink { state } => LirRvalue::MakeChainLink {
            state: *state as u64,
        },
        Rvalue::TakeChainLink { result_ty } => LirRvalue::TakeChainLink {
            result_ty: *result_ty,
        },
        Rvalue::ResumeChainLink {
            link_slot,
            result_ty,
        } => LirRvalue::ResumeChainLink {
            link_slot: *link_slot as u64,
            result_ty: *result_ty,
        },
    };
    Ok(out)
}

fn map_pattern(
    p: &scoop2_mir::mir::Pattern,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    step_tags: &StepTagTables,
) -> LirPattern {
    use scoop2_mir::mir::Pattern;
    match p {
        Pattern::Wildcard => LirPattern::Wildcard,
        Pattern::Bind { ty, .. } => LirPattern::Bind { ty: *ty },
        Pattern::IntLit(v) => LirPattern::IntLit(*v),
        Pattern::CharLit(c) => LirPattern::CharLit(*c),
        Pattern::StringLit(s) => LirPattern::StringLit(s.clone()),
        Pattern::BoolLit(b) => LirPattern::BoolLit(*b),
        Pattern::Is { ty, negated } => {
            // 解析目标类型的 FQN（供 codegen 计算 type_id）。
            let target_fqn = match types.kind(*ty) {
                scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => {
                    Some(interner.resolve(n.fqn).to_string())
                }
                scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Nominal(n)) => {
                    Some(interner.resolve(n.fqn).to_string())
                }
                _ => None,
            };
            LirPattern::Is {
                ty: *ty,
                negated: *negated,
                target_fqn,
            }
        }
        Pattern::Tuple { elements } => LirPattern::Tuple {
            elements: elements
                .iter()
                .map(|p| map_pattern(p, types, hir, interner, step_tags))
                .collect(),
        },
        Pattern::Struct { type_fqn, fields } => LirPattern::Struct {
            type_fqn: interner.resolve(*type_fqn).to_string(),
            fields: fields
                .iter()
                .map(|f| {
                    (
                        interner.resolve(f.name).to_string(),
                        map_pattern(&f.pattern, types, hir, interner, step_tags),
                    )
                })
                .collect(),
        },
        Pattern::Variant {
            enum_fqn,
            variant_name,
            args,
        } => {
            let vname = interner.resolve(*variant_name).to_string();
            // 判别值 = 变体在 enum_variants 声明序中的下标（与 EnumVariant 构造同源）。
            // 合成 Step enum（effect lowering 产物）不在 HIR enum_variants 中，
            // tag 取 effect_abi.step_variants 声明序（enum_fqn = 所属函数 FQN sym）。
            let tag_value = step_tags
                .pattern_tags
                .get(&(*enum_fqn, *variant_name))
                .copied()
                .or_else(|| {
                    hir.enum_variants.get(enum_fqn).and_then(|variants| {
                        variants
                            .iter()
                            .position(|&v| interner.resolve(v) == vname)
                            .map(|i| i as u64)
                    })
                });
            LirPattern::Variant {
                variant_name: vname,
                tag_value,
                args: args
                    .iter()
                    .map(|p| map_pattern(p, types, hir, interner, step_tags))
                    .collect(),
            }
        }
        Pattern::Or { patterns } => LirPattern::Or {
            patterns: patterns
                .iter()
                .map(|p| map_pattern(p, types, hir, interner, step_tags))
                .collect(),
        },
    }
}

fn map_term(kind: &scoop2_mir::mir::TerminatorKind) -> LirTerminator {
    use scoop2_mir::mir::TerminatorKind;
    match kind {
        TerminatorKind::Return { value } => LirTerminator::Return {
            value: value.as_ref().map(map_operand),
        },
        TerminatorKind::Goto { target } => LirTerminator::Goto { target: target.0 },
        TerminatorKind::CondBr {
            cond,
            then_target,
            else_target,
        } => LirTerminator::CondBr {
            cond: map_operand(cond),
            then_target: then_target.0,
            else_target: else_target.0,
        },
        TerminatorKind::Unreachable => LirTerminator::Unreachable,
        // Perform/Handle 已被 effect lowering 消除。
        _ => LirTerminator::Unreachable,
    }
}

/// 映射 MIR Operand → LIR LirOperand。
/// Local → LirOperand::Local；Const → LirOperand::Const。
/// 不使用哨兵值——常量被完整保留在 LirOperand::Const 中。
fn map_operand(op: &scoop2_mir::mir::Operand) -> LirOperand {
    match op {
        scoop2_mir::mir::Operand::Local(lid) => LirOperand::Local(lid.0),
        scoop2_mir::mir::Operand::Const(c) => LirOperand::Const(map_const(c)),
    }
}

/// 映射 MIR ConstValue → LIR LirConstValue。
fn map_const(c: &scoop2_mir::mir::ConstValue) -> LirConstValue {
    use scoop2_mir::mir::ConstValue;
    match c {
        ConstValue::Bool(b) => LirConstValue::Bool(*b),
        ConstValue::Char(ch) => LirConstValue::Char(*ch),
        ConstValue::Unit => LirConstValue::Unit,
        ConstValue::Int(v, suf) => LirConstValue::Int(
            *v,
            suf.as_ref().map(|s| match s {
                scoop2_mir::mir::IntSuffix::U => LirIntSuffix::U,
                scoop2_mir::mir::IntSuffix::L => LirIntSuffix::L,
                scoop2_mir::mir::IntSuffix::UL => LirIntSuffix::UL,
            }),
        ),
        ConstValue::Float(v, suf) => LirConstValue::Float(
            *v,
            suf.as_ref().map(|s| match s {
                scoop2_mir::mir::FloatSuffix::F32 => LirFloatSuffix::F32,
            }),
        ),
        ConstValue::String(s) => LirConstValue::String(s.clone()),
        ConstValue::Null => LirConstValue::Null,
    }
}

/// 计算 struct/class 字段的字节偏移。
///
/// - value struct：字段偏移从 0 起，布局与 [`layout`] 模块的 Struct 布局算法一致
///   （优先直接读 `TypeLayoutTable` 的 Struct fields——单一事实来源；
///   布局缺失时按同算法用 `member_order` 声明序现算）。
/// - class 引用：字段偏移从 GC 对象头（32B）之后开始，超类字段在前、自身字段
///   按声明序在后，每个字段按 ptr_size(8) 对齐打包（与 codegen class_ctor 一致）。
///
/// 错误策略：receiver 是 nominal 且 HIR 注册了 members、却找不到该成员名
/// （或布局表字段数与声明数不一致）时返回 [`LirError`]——绝不静默返回 0。
/// receiver 完全未知（内建 / 外部类型的固有成员，如 String.length）保持旧回退 0。
fn compute_field_offset(
    receiver_ty: scoop2_hir::ty::TypeId,
    member_name: &str,
    layouts: &TypeLayoutTable,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
) -> Result<u64, LirError> {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    // 解析 receiver 类型到 nominal FQN。
    let (fqn_sym, is_ref) = match types.kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => (n.fqn, true),
        TypeKind::Value(ValueTypeKind::Nominal(n)) => (n.fqn, false),
        _ => return Ok(0),
    };
    // HIR 完全未知该类型（内建 / 外部 opaque 类型，如 String.length）：旧回退。
    // 已知 class（class_fqns 登记）不走此回退：class 可能自身无字段（members
    // 无条目）但沿超类链继承字段，由下方 ordered_class_fields 解析。
    let known_class = is_ref && hir.class_fqns.contains(&fqn_sym);
    if !known_class && !hir.members.contains_key(&fqn_sym) {
        return Ok(0);
    }
    let fqn_text = interner.resolve(fqn_sym);
    let not_found = || {
        LirError::new(format!(
            "类型 {} 注册了 members 但找不到字段 {}",
            fqn_text, member_name
        ))
    };
    if is_ref {
        // class：超类字段在前 + 自身字段按声明序，ptr_size 对齐打包。
        let header_size: u64 = 32;
        let ptr_size: u64 = 8;
        let ordered = hir.ordered_class_fields(fqn_sym);
        let mut offset: u64 = header_size;
        for (name_sym, member_ty) in &ordered {
            offset = align_up_u64(offset, ptr_size);
            if interner.resolve(*name_sym) == member_name {
                return Ok(offset);
            }
            let member_size = layouts
                .get(*member_ty)
                .map(|l| l.size)
                .unwrap_or(ptr_size)
                .max(ptr_size);
            offset += member_size;
        }
        Err(not_found())
    } else {
        // value struct：优先读 TypeLayoutTable 的 Struct fields（与 codegen 同源）。
        let ordered = hir.ordered_members(&fqn_sym);
        let layout_fields = layouts.get(receiver_ty).and_then(|l| match &l.kind {
            TypeLayoutKind::Struct { fields } => Some(fields),
            _ => None,
        });
        if let Some(fields) = layout_fields {
            let pos = ordered
                .iter()
                .position(|(name_sym, _)| interner.resolve(*name_sym) == member_name);
            let pos = pos.ok_or_else(not_found)?;
            let field = fields.get(pos).ok_or_else(|| {
                LirError::new(format!(
                    "类型 {} 的 Struct 布局字段数 ({}) 与声明成员数 ({}) 不一致",
                    fqn_text,
                    fields.len(),
                    ordered.len()
                ))
            })?;
            return Ok(field.offset);
        }
        // 布局缺失：按 layout 模块同算法现算（sub size/align 取自布局表）。
        let mut offset: u64 = 0;
        for (name_sym, member_ty) in &ordered {
            let (fsize, falign) = layouts
                .get(*member_ty)
                .map(|l| (l.size, l.align))
                .unwrap_or((8, 8));
            offset = align_up_u64(offset, falign.max(1));
            if interner.resolve(*name_sym) == member_name {
                return Ok(offset);
            }
            offset += fsize;
        }
        Err(not_found())
    }
}

/// 对齐向上取整：返回 >= val 的最小 align 倍数（align > 0）。
fn align_up_u64(val: u64, align: u64) -> u64 {
    if align == 0 {
        return val;
    }
    let rem = val % align;
    if rem == 0 { val } else { val + (align - rem) }
}

/// 类型是否为 `scoop.core.MutableArray<T>` 引用（决定数组布局分派与 MakeArray 是否 freeze）。
fn is_mutable_array_ty(
    ty: scoop2_hir::ty::TypeId,
    types: &scoop2_hir::ty::TypeStore,
    interner: &Interner,
) -> bool {
    use scoop2_hir::ty::{RefTypeKind, TypeKind};
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            interner.resolve(n.fqn) == "scoop.core.MutableArray"
        }
        _ => false,
    }
}

/// 解析 nominal receiver 的命名字段类型（与 compute_field_offset 的字段定位同源：
/// value struct 用 `ordered_members`，class 用 `ordered_class_fields`，均按声明序）。
fn resolve_named_field_ty(
    receiver_ty: scoop2_hir::ty::TypeId,
    member_name: &str,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
) -> Result<scoop2_hir::ty::TypeId, LirError> {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    let ordered = match types.kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => hir.ordered_class_fields(n.fqn),
        TypeKind::Value(ValueTypeKind::Nominal(n)) => hir.ordered_members(&n.fqn),
        _ => {
            return Err(LirError::new(format!(
                "with 更新的 receiver 不是 nominal 类型（字段 {member_name}）"
            )));
        }
    };
    ordered
        .iter()
        .find(|(name_sym, _)| interner.resolve(*name_sym) == member_name)
        .map(|(_, member_ty)| *member_ty)
        .ok_or_else(|| LirError::new(format!("with 更新找不到字段 {member_name}")))
}

/// 把 MIR with 更新路径逐段解析为 LIR 段（字段名 + 布局偏移 + 字段类型）。
///
/// 偏移用 [`compute_field_offset`]（与 MemberAccess 同源）；tuple 段直接读
/// TypeLayoutTable 的 Tuple elements。解析失败（非 nominal receiver / 字段缺失 /
/// tuple 布局缺失或索引越界）返回 [`LirError`]——绝不静默落到字段 0。
///
/// enum variant 模式（`err with { Ok.point.x: 7 }`）：首段匹配 result_ty 的
/// enum variant 名时，返回 [`LirWithUpdateVariantTarget`]（运行时 tag 检查），
/// 且之后各段的 offset 是 enum 值内的绝对字节偏移（variant slot 起点累计，
/// 首字段段按 `<enum>.<variant>` 名义下登记的 payload 成员定位）。
fn resolve_with_update_path(
    result_ty: scoop2_hir::ty::TypeId,
    segments: &[scoop2_mir::mir::WithUpdateSegment],
    layouts: &TypeLayoutTable,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
) -> Result<
    (
        Option<crate::LirWithUpdateVariantTarget>,
        Vec<crate::LirWithUpdateSegment>,
    ),
    LirError,
> {
    use scoop2_hir::ty::{TypeKind, ValueTypeKind};
    let mut out = Vec::with_capacity(segments.len());
    let mut variant_target = None;
    let mut cur_ty = result_ty;
    // enum variant 模式下的绝对偏移累计（None = 非 enum 模式，offset 逐层相对）。
    let mut abs_acc: Option<u64> = None;
    let mut iter = segments.iter();
    if let Some(scoop2_mir::mir::WithUpdateSegment::Named(sym)) = segments.first() {
        let name = interner.resolve(*sym);
        let variant = layouts.get(result_ty).and_then(|l| match &l.kind {
            TypeLayoutKind::Enum { variants, .. } => {
                variants.iter().find(|v| v.name == name).cloned()
            }
            _ => None,
        });
        if let Some(v) = variant {
            variant_target = Some(crate::LirWithUpdateVariantTarget {
                name: name.to_string(),
                tag_value: v.tag_value,
            });
            iter.next();
            // 首个字段段：owner 是 `<enum_fqn>.<variant>` 名义下的 payload 成员。
            let Some(scoop2_mir::mir::WithUpdateSegment::Named(field_sym)) = iter.next()
            else {
                return Err(LirError::new(format!(
                    "with 更新的 enum variant `{name}` 后缺少字段路径"
                )));
            };
            let field_name = interner.resolve(*field_sym);
            let enum_fqn = match types.kind(result_ty) {
                TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn,
                _ => {
                    return Err(LirError::new(
                        "with 更新的 enum receiver 不是 nominal 值类型",
                    ));
                }
            };
            let owner_text = format!("{}.{}", interner.resolve(enum_fqn), name);
            let owner = interner
                .get(&owner_text)
                .ok_or_else(|| LirError::new(format!("with 更新找不到 variant 类型 {owner_text}")))?;
            let ordered = hir.ordered_members(&owner);
            let idx = ordered
                .iter()
                .position(|(n, _)| interner.resolve(*n) == field_name)
                .ok_or_else(|| LirError::new(format!("with 更新找不到字段 {field_name}")))?;
            let field_ty = ordered[idx].1;
            // 单字段 variant（payload_fields 为空）字段在 slot 起点；
            // 多字段 variant 按声明序取 payload_fields 相对偏移。
            let rel = if v.payload_fields.is_empty() {
                0
            } else {
                v.payload_fields.get(idx).map(|f| f.offset).ok_or_else(|| {
                    LirError::new(format!(
                        "with 更新字段 {field_name} 在 variant `{name}` 的布局中缺失"
                    ))
                })?
            };
            let abs = v.slot_offset + rel;
            out.push(crate::LirWithUpdateSegment {
                name: field_name.to_string(),
                offset: abs,
                ty: field_ty,
            });
            cur_ty = field_ty;
            abs_acc = Some(abs);
        }
    }
    for seg in iter {
        match seg {
            scoop2_mir::mir::WithUpdateSegment::Named(sym) => {
                let name = interner.resolve(*sym).to_string();
                let rel = compute_field_offset(cur_ty, &name, layouts, types, hir, interner)?;
                let field_ty = resolve_named_field_ty(cur_ty, &name, types, hir, interner)?;
                let offset = abs_acc.map(|b| b + rel).unwrap_or(rel);
                if abs_acc.is_some() {
                    abs_acc = Some(offset);
                }
                out.push(crate::LirWithUpdateSegment {
                    name,
                    offset,
                    ty: field_ty,
                });
                cur_ty = field_ty;
            }
            scoop2_mir::mir::WithUpdateSegment::TupleIndex(idx) => {
                let fields = layouts.get(cur_ty).and_then(|l| match &l.kind {
                    TypeLayoutKind::Tuple { elements } => Some(elements),
                    _ => None,
                });
                let field = fields.and_then(|fs| fs.get(*idx as usize)).ok_or_else(|| {
                    LirError::new(format!(
                        "with 更新路径段 _{idx} 的 receiver 不是含该索引的 tuple 布局"
                    ))
                })?;
                let offset = abs_acc.map(|b| b + field.offset).unwrap_or(field.offset);
                if abs_acc.is_some() {
                    abs_acc = Some(offset);
                }
                out.push(crate::LirWithUpdateSegment {
                    name: format!("_{idx}"),
                    offset,
                    ty: field.ty,
                });
                cur_ty = field.ty;
            }
        }
    }
    Ok((variant_target, out))
}

/// 在 TypeStore 中查找 Any 引用类型的 TypeId（不修改 store）。
fn find_any_type(types: &scoop2_hir::ty::TypeStore) -> scoop2_hir::ty::TypeId {
    // TypeStore 的 Any 类型在 intern 时分配了固定 ID。
    // 遍历前几个 ID 查找 Ref(Any)。
    for i in 0..100u32 {
        let tid = scoop2_hir::ty::TypeId(i);
        match types.kind(tid) {
            scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Any) => return tid,
            _ => {}
        }
    }
    // 回退：返回 Nothing（size 0）。
    for i in 0..100u32 {
        let tid = scoop2_hir::ty::TypeId(i);
        if matches!(types.kind(tid), scoop2_hir::ty::TypeKind::Nothing) {
            return tid;
        }
    }
    scoop2_hir::ty::TypeId(0)
}
