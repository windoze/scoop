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

/// 主入口：将 MaterializedMir + TypedHir 降级为 LirProgram。
pub fn lower_to_lir(
    mir: &MaterializedMir,
    hir: &TypedHir,
    interner: &Interner,
) -> LirProgram {
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
    // 7. MIR→LIR body 映射（含 GC info + effect schema 挂载）
    map_bodies(&mut program, mir, hir, interner);
    // 8. 回填分发表信息到调用点
    dispatch::backfill_call_sites(&mut program);
    // 9. 验证
    verify::verify_lir(&program);
    program
}

// =========================================================================
// MIR→LIR body 映射
// =========================================================================

fn map_bodies(program: &mut LirProgram, mir: &MaterializedMir, hir: &TypedHir, interner: &Interner) {
    for item in &mir.module.items {
        match item {
            scoop2_mir::mir::Item::Fun(fd) => {
                if fd.body.is_some() {
                    // 有函数体：放入 callables。
                    program.callables.push(map_callable(fd, mir, hir, &program.type_layouts, interner));
                } else {
                    // 无函数体（extern / abstract / intrinsic）：放入 declarations。
                    let symbol_name = &fd.fqn;
                    program.declarations.push(LirDeclaration {
                        fqn: fd.fqn.clone(),
                        symbol_name: symbol_name.clone(),
                        params: fd.params.iter().map(|p| LirParam {
                            name: p.name.clone(), ty: p.ty,
                            abi: abi::param_abi_for_type(p.ty, &program.type_layouts),
                        }).collect(),
                        return_ty: fd.return_ty,
                        return_abi: abi::param_abi_for_type(fd.return_ty, &program.type_layouts),
                        is_extern: true,
                        extern_symbol: Some(symbol_name.clone()),
                    });
                }
            }
            scoop2_mir::mir::Item::Initializer(ir) => {
                program.callables.push(map_initializer(ir, &program.type_layouts, &mir.module.types, hir, interner));
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
                });
            }
            _ => {}
        }
    }
}

fn map_callable(
    fd: &scoop2_mir::mir::FunDecl,
    mir: &MaterializedMir,
    hir: &TypedHir,
    layouts: &TypeLayoutTable,
    interner: &Interner,
) -> LirCallable {
    let symbol_name = abi::mangle_symbol(&fd.fqn, &fd.stable_template_key);
    let abi_kind = if fd.effect_abi.is_some() { LirCallableAbi::EffectStep } else { LirCallableAbi::Plain };
    let params: Vec<LirParam> = fd.params.iter().map(|p| LirParam {
        name: p.name.clone(), ty: p.ty, abi: abi::param_abi_for_type(p.ty, layouts),
    }).collect();
    let return_abi = abi::param_abi_for_type(fd.return_ty, layouts);

    // 计算函数体的 GC info 和 effect schema。
    let (body, gc_info, frame_schema, step_layout, state_dispatch, continuation_layout) = if let Some(mir_body) = &fd.body {
        let lir_body = map_body(mir_body, layouts, &mir.module.types, hir, interner);
        let frame_for_gc = fd.effect_abi.as_ref()
            .map(|ea| (ea.frame_ty, ea.frame_local));
        let gc = gc::compute_gc_info_for_body(mir_body, &fd.fqn, layouts, frame_for_gc);
        let (fs, sl, sd, cl) = if let Some(ref eff_abi) = fd.effect_abi {
            effect::prepare_effect_abi(eff_abi, &fd.fqn, layouts, hir, interner)
        } else {
            (None, None, None, None)
        };
        (Some(lir_body), Some(gc), fs, sl, sd, cl)
    } else {
        (None, None, None, None, None, None)
    };

    LirCallable {
        fqn: fd.fqn.clone(), symbol_name, abi: abi_kind, params,
        return_ty: fd.return_ty, return_abi, body,
        gc_info, frame_schema, step_layout, state_dispatch, continuation_layout,
    }
}

fn map_initializer(ir: &scoop2_mir::mir::InitializerRoot, layouts: &TypeLayoutTable, types: &scoop2_hir::ty::TypeStore, hir: &TypedHir, interner: &Interner) -> LirCallable {
    let symbol_name = abi::mangle_symbol(&ir.fqn, &None);
    let body = map_body(&ir.body, layouts, types, hir, interner);
    LirCallable {
        fqn: ir.fqn.clone(), symbol_name, abi: LirCallableAbi::Plain,
        params: Vec::new(), return_ty: ir.ty, return_abi: ParamAbi::Direct,
        body: Some(body), gc_info: None, frame_schema: None, step_layout: None, state_dispatch: None, continuation_layout: None,
    }
}

fn map_body(body: &scoop2_mir::mir::Body, layouts: &TypeLayoutTable, types: &scoop2_hir::ty::TypeStore, hir: &TypedHir, interner: &Interner) -> LirBody {
    let locals = body.locals.iter().enumerate().map(|(i, d)| LirLocalDecl {
        id: i as u32, name: d.name.clone(), ty: d.ty, mutable: d.mutable,
        gc_traceable: gc::is_gc_traceable_type(d.ty, layouts),
    }).collect();
    let blocks = body.blocks.iter().enumerate().map(|(bi, blk)| LirBlock {
        id: bi as u32,
        stmts: blk.stmts.iter().map(|s| map_stmt(s, types, hir, interner)).collect(),
        terminator: map_term(&blk.terminator.kind),
    }).collect();
    LirBody { locals, blocks, start_block: body.start.0 }
}

fn map_stmt(stmt: &scoop2_mir::mir::Statement, types: &scoop2_hir::ty::TypeStore, hir: &TypedHir, interner: &Interner) -> LirStmt {
    use scoop2_mir::mir::StatementKind;
    let kind = match &stmt.kind {
        StatementKind::Nop => LirStmtKind::Nop,
        StatementKind::Assign { target, value } => LirStmtKind::Assign {
            target: target.0, value: map_rvalue(value, types, hir, interner),
        },
        StatementKind::StoreMember { receiver, member, value, value_ty, .. } => {
            let receiver_ty = member.receiver_ty;
            LirStmtKind::StoreMember {
                receiver_local: map_operand(receiver),
                receiver_ty,
                member_name: member.name.clone(),
                field_offset: compute_field_offset(receiver_ty, &member.name, types, hir, interner),
                value_local: map_operand(value),
                value_ty: *value_ty,
            }
        },
        StatementKind::StoreTupleIndex { receiver, index, value, value_ty } => LirStmtKind::StoreTupleIndex {
            receiver_local: map_operand(receiver), index: *index,
            value_local: map_operand(value), value_ty: *value_ty,
        },
        StatementKind::StoreTopLevelVar { fqn, value, value_ty } => LirStmtKind::StoreGlobal {
            global_fqn: interner.resolve(*fqn).to_string(),
            value_local: map_operand(value), value_ty: *value_ty,
        },
        StatementKind::Panic { message } => LirStmtKind::Panic { message: message.clone() },
    };
    LirStmt { span: stmt.span, kind }
}

fn map_rvalue(rv: &scoop2_mir::mir::Rvalue, types: &scoop2_hir::ty::TypeStore, hir: &TypedHir, interner: &Interner) -> LirRvalue {
    use scoop2_mir::mir::{CallKind, Operand, Rvalue};
    match rv {
        Rvalue::Use(op) => match op {
            Operand::Local(lid) => LirRvalue::Use(lid.0),
            Operand::Const(c) => LirRvalue::Const(map_const(c)),
        },
        Rvalue::TopLevelRef(tl) => LirRvalue::TopLevelRef {
            fqn: tl.fqn.clone(),
            // TopLevelRef 不携带类型字段；从已知 locals 的引用类型中取一个作为占位。
            // 优先使用 generic_type_args 中的第一个类型。
            ty: tl.generic_type_args.first().copied()
                .unwrap_or_else(|| find_any_type(types)),
        },
        Rvalue::UnresolvedName { name } => LirRvalue::TopLevelRef {
            fqn: format!("<unresolved:{}>", name),
            ty: find_any_type(types),
        },
        Rvalue::TypeTest { value, metadata, .. } => LirRvalue::TypeTest {
            value_local: map_operand(value), target_ty: metadata.target_ty,
        },
        Rvalue::Cast { value, metadata, .. } => match &metadata.result {
            scoop2_mir::mir::transport::RuntimeCastResult::Target { ty } => LirRvalue::Cast {
                value_local: map_operand(value), target_ty: *ty,
            },
            scoop2_mir::mir::transport::RuntimeCastResult::Option { option_ty, .. } => LirRvalue::Cast {
                value_local: map_operand(value), target_ty: *option_ty,
            },
        },
        Rvalue::MemberAccess { receiver, member, .. } => {
            let receiver_ty = member.receiver_ty;
            // 从 HIR members 表查得成员的真实声明类型，而非用 receiver_ty 占位。
            // 旧实现把 result_ty 设为 receiver_ty，对 codegen 解析成员类型是错误的：
            // 例如 `animal.sound` 的结果应是 Int，而非 Animal 引用类型本身。
            let result_ty = {
                use scoop2_hir::ty::{TypeKind, RefTypeKind, ValueTypeKind};
                let fqn = match types.kind(member.receiver_ty) {
                    TypeKind::Ref(RefTypeKind::Nominal(n)) => Some(n.fqn),
                    TypeKind::Value(ValueTypeKind::Nominal(n)) => Some(n.fqn),
                    _ => None,
                };
                let member_sym = interner.get(&member.name);
                fqn.and_then(|f| hir.members.get(&f))
                    .and_then(|m| member_sym.and_then(|s| m.get(&s)))
                    .copied()
                    .unwrap_or(member.receiver_ty)
            };
            LirRvalue::MemberAccess {
                receiver_local: map_operand(receiver),
                receiver_ty,
                member_name: member.name.clone(),
                field_offset: compute_field_offset(receiver_ty, &member.name, types, hir, interner),
                result_ty,
            }
        }
        Rvalue::TupleIndex { receiver, index, element_ty } => LirRvalue::TupleIndex {
            receiver_local: map_operand(receiver), index: *index, element_ty: *element_ty,
        },
        Rvalue::IndexAccess { receiver, indices, element_ty } => LirRvalue::IndexAccess {
            receiver_local: map_operand(receiver),
            index_locals: indices.iter().map(map_operand).collect(),
            element_ty: *element_ty,
        },
        Rvalue::EnumVariant { enum_ty, variant_name, args, payload, .. } => {
            let vname = interner.resolve(*variant_name).to_string();
            // 从 HIR enum_variants 查找变体序号作为 tag_value。
            let tag_value = if let scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Nominal(n)) = types.kind(*enum_ty) {
                if let Some(variants) = hir.enum_variants.get(&n.fqn) {
                    variants.iter().position(|&v| interner.resolve(v) == vname)
                        .map(|i| i as u64).unwrap_or(0)
                } else { 0 }
            } else { 0 };
            // payload 类型：从 transport 的 aggregate 获取。
            let payload_ty = if args.is_empty() { None } else { Some(payload.aggregate_ty) };
            LirRvalue::EnumVariant {
                enum_ty: *enum_ty,
                variant_name: vname,
                tag_value,
                args: args.iter().map(|a| map_operand(&a.value)).collect(),
                payload_ty,
            }
        },
        Rvalue::ClassCtor { type_fqn, args, .. } => LirRvalue::ClassCtor {
            class_fqn: interner.resolve(*type_fqn).to_string(),
            args: args.iter().map(|a| map_operand(&a.value)).collect(),
        },
        Rvalue::Call { kind, args, transport, .. } => {
            let ck = match kind {
                CallKind::Direct { callee_fqn, .. } => LirCallKind::Direct {
                    callee_symbol: callee_fqn.clone(),
                },
                CallKind::Virtual { receiver, dispatch, .. } => LirCallKind::Virtual {
                    receiver_local: map_operand(receiver),
                    owner_fqn: dispatch.owner_fqn.clone(),
                    method_name: dispatch.member_name.clone(),
                    vtable_slot: 0,
                },
                CallKind::Interface { receiver, dispatch, .. } => LirCallKind::Interface {
                    receiver_local: map_operand(receiver),
                    interface_fqn: dispatch.owner_fqn.clone(),
                    method_name: dispatch.member_name.clone(),
                    interface_id: 0, itable_slot: 0,
                },
                CallKind::Closure { callee, .. } => LirCallKind::Closure {
                    callee_local: map_operand(callee),
                },
                CallKind::FunValue { callee } => LirCallKind::FunValue {
                    callee_local: map_operand(callee),
                },
                CallKind::Resume { .. } => LirCallKind::Direct {
                    callee_symbol: "scoop.core.Continuation.resume".to_string(),
                },
            };
            LirRvalue::Call(LirCall {
                kind: ck,
                args: args.iter().map(|a| map_operand(&a.value)).collect(),
                result_ty: transport.result.source_ty,
            })
        }
        Rvalue::MakeTuple { elements, transport } => LirRvalue::MakeTuple {
            elements: elements.iter().map(map_operand).collect(),
            ty: transport.aggregate_ty,
        },
        Rvalue::MakeArray { elements, result_ty } => LirRvalue::MakeArray {
            elements: elements.iter().map(map_operand).collect(),
            ty: *result_ty,
        },
        Rvalue::StructLit { type_fqn, fields, transport } => LirRvalue::StructLit {
            type_fqn: interner.resolve(*type_fqn).to_string(),
            fields: fields.iter().map(|f| (interner.resolve(f.name).to_string(), map_operand(&f.value))).collect(),
            ty: transport.aggregate_ty,
        },
        Rvalue::InterpolatedString { parts } => LirRvalue::InterpolatedString {
            parts: parts.iter().map(|p| match p {
                scoop2_mir::mir::InterpolatedPart::Lit(s) => LirInterpolatedPart::Lit(s.clone()),
                scoop2_mir::mir::InterpolatedPart::Expr(op) => LirInterpolatedPart::Expr(map_operand(op)),
            }).collect(),
        },
        Rvalue::WithUpdate { base, updates, result_ty } => LirRvalue::WithUpdate {
            base_local: map_operand(base),
            updates: updates.iter().map(|u| {
                let field_name = u.path.first().map(|seg| {
                    match seg {
                        scoop2_mir::mir::WithUpdateSegment::Named(sym) => interner.resolve(*sym).to_string(),
                        scoop2_mir::mir::WithUpdateSegment::TupleIndex(idx) => format!("_{}", idx),
                    }
                }).unwrap_or_default();
                LirWithUpdateField {
                    field_name,
                    value: map_operand(&u.value),
                    value_ty: u.value_ty,
                }
            }).collect(),
            result_ty: *result_ty,
        },
        Rvalue::MakeClosure { env, invoke_fqn, .. } => LirRvalue::MakeClosure {
            env_local: map_operand(env), invoke_fqn: invoke_fqn.clone(),
        },
        Rvalue::ClassLit { type_fqn } => LirRvalue::ClassLit {
            type_fqn: interner.resolve(*type_fqn).to_string(),
        },
        Rvalue::PerformResult { .. } => {
            // PerformResult 是 effect lowering 前的占位值，lowering 后不应出现。
            // 映射为 Unit 常量（其类型信息已在 effect lowering 中处理）。
            LirRvalue::Const(LirConstValue::Unit)
        },
        Rvalue::PatternMatch { subject, pattern } => LirRvalue::PatternMatch {
            subject_local: map_operand(subject), pattern: map_pattern(pattern, interner),
        },
        Rvalue::PatternExtract { subject, result_ty, .. } => LirRvalue::PatternExtract {
            subject_local: map_operand(subject), result_ty: *result_ty,
        },
        Rvalue::IntEq { lhs, rhs } => LirRvalue::IntEq {
            lhs_local: map_operand(lhs), rhs_local: map_operand(rhs),
        },
    }
}

fn map_pattern(p: &scoop2_mir::mir::Pattern, interner: &Interner) -> LirPattern {
    use scoop2_mir::mir::Pattern;
    match p {
        Pattern::Wildcard => LirPattern::Wildcard,
        Pattern::Bind { ty, .. } => LirPattern::Bind { ty: *ty },
        Pattern::IntLit(v) => LirPattern::IntLit(*v),
        Pattern::CharLit(c) => LirPattern::CharLit(*c),
        Pattern::StringLit(s) => LirPattern::StringLit(s.clone()),
        Pattern::BoolLit(b) => LirPattern::BoolLit(*b),
        Pattern::Is { ty, negated } => LirPattern::Is { ty: *ty, negated: *negated },
        Pattern::Tuple { elements } => LirPattern::Tuple {
            elements: elements.iter().map(|p| map_pattern(p, interner)).collect(),
        },
        Pattern::Struct { type_fqn, fields } => LirPattern::Struct {
            type_fqn: interner.resolve(*type_fqn).to_string(),
            fields: fields.iter().map(|f| {
                (interner.resolve(f.name).to_string(), map_pattern(&f.pattern, interner))
            }).collect(),
        },
        Pattern::Variant { variant_name, args, .. } => LirPattern::Variant {
            variant_name: interner.resolve(*variant_name).to_string(),
            args: args.iter().map(|p| map_pattern(p, interner)).collect(),
        },
        Pattern::Or { patterns } => LirPattern::Or {
            patterns: patterns.iter().map(|p| map_pattern(p, interner)).collect(),
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
        TerminatorKind::CondBr { cond, then_target, else_target } => LirTerminator::CondBr {
            cond: map_operand(cond), then_target: then_target.0, else_target: else_target.0,
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
        ConstValue::Int(v, suf) => LirConstValue::Int(*v, suf.as_ref().map(|s| match s {
            scoop2_mir::mir::IntSuffix::U => LirIntSuffix::U,
            scoop2_mir::mir::IntSuffix::L => LirIntSuffix::L,
            scoop2_mir::mir::IntSuffix::UL => LirIntSuffix::UL,
        })),
        ConstValue::Float(v, suf) => LirConstValue::Float(*v, suf.as_ref().map(|s| match s {
            scoop2_mir::mir::FloatSuffix::F32 => LirFloatSuffix::F32,
        })),
        ConstValue::String(s) => LirConstValue::String(s.clone()),
        ConstValue::Null => LirConstValue::Null,
    }
}

/// 从 HIR members 和类型布局计算 struct/class 字段的字节偏移。
///
/// 对于 class 引用类型，字段偏移从 GC 对象头之后开始（偏移 8）。
/// 对于 value struct 类型，字段偏移从 0 开始。
fn compute_field_offset(
    receiver_ty: scoop2_hir::ty::TypeId,
    member_name: &str,
    types: &scoop2_hir::ty::TypeStore,
    hir: &TypedHir,
    interner: &Interner,
) -> u64 {
    use scoop2_hir::ty::{TypeKind, RefTypeKind, ValueTypeKind};
    // 解析 receiver 类型到 nominal FQN。
    let (fqn_sym, is_ref) = match types.kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => (n.fqn, true),
        TypeKind::Value(ValueTypeKind::Nominal(n)) => (n.fqn, false),
        _ => return 0,
    };
    // 查 HIR members 获取字段列表。
    let members = match hir.members.get(&fqn_sym) {
        Some(m) => m,
        None => return 0,
    };
    // 计算字段偏移：class 引用从 GC 头(8B)开始，value struct 从 0 开始。
    let mut offset: u64 = if is_ref { 8 } else { 0 };
    for (&member_sym, &member_ty) in members {
        if interner.resolve(member_sym) == member_name {
            return offset;
        }
        // 累加前一个字段的大小。
        let member_size = match types.kind(member_ty) {
            TypeKind::Value(ValueTypeKind::Unit) | TypeKind::Nothing => 0,
            TypeKind::Value(ValueTypeKind::Bool) => 1,
            TypeKind::Value(ValueTypeKind::Char) => 4,
            TypeKind::Value(ValueTypeKind::Int)
            | TypeKind::Value(ValueTypeKind::UInt)
            | TypeKind::Value(ValueTypeKind::Float64) => 8,
            TypeKind::Value(ValueTypeKind::Float32) => 4,
            TypeKind::Ref(_) => 8,
            _ => 8, // 保守默认
        };
        offset += member_size;
    }
    0
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
