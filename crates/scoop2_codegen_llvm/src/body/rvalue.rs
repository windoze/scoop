//! rvalue lowering：`LirRvalue` → LLVM 值。
//!
//! 当前实现覆盖最小子集：Use、Const、Call(Direct→intrinsic/runtime)。
//! 其余 rvalue（MemberAccess/构造/模式/调用分发等）在 W1-5/W1-6 完善，
//! 未覆盖的返回明确错误（绝不静默/panic）。

use inkwell::values::BasicValueEnum;
use inkwell::AddressSpace;

use scoop2_hir::ty::TypeId;
use scoop2_lir::{LirOperand, LirRvalue};

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 顶层入口：lowering 一个 rvalue，返回其 LLVM 值。`target_ty` 提示结果类型。
pub fn lower_rvalue<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    rv: &LirRvalue,
    target_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match rv {
        LirRvalue::Use(id) => fl.load_local(*id),
        LirRvalue::Const(c) => super::consts::lower_const(fl, c, target_ty),
        LirRvalue::Call(call) => super::call::lower_call(fl, call),
        LirRvalue::MakeTuple { elements, ty } => lower_make_tuple(fl, elements, *ty),
        LirRvalue::TupleIndex {
            receiver_local,
            index,
            element_ty,
        } => lower_tuple_index(fl, receiver_local, *index as u32, *element_ty),
        LirRvalue::StructLit { fields, ty, .. } => lower_struct_lit(fl, fields, *ty),
        LirRvalue::IntEq {
            lhs_local,
            rhs_local,
        } => lower_int_eq(fl, lhs_local, rhs_local),
        LirRvalue::MemberAccess {
            receiver_local,
            member_name,
            field_offset,
            result_ty,
            ..
        } => lower_member_access(fl, receiver_local, member_name, *field_offset, *result_ty),
        LirRvalue::TopLevelRef { fqn, .. } => lower_top_level_ref(fl, fqn),
        LirRvalue::IndexAccess {
            receiver_local,
            index_locals,
            element_ty,
            receiver_mutable,
        } => lower_index_access(
            fl,
            receiver_local,
            index_locals,
            *element_ty,
            *receiver_mutable,
        ),
        LirRvalue::TypeTest {
            value_local,
            target_ty,
            static_fold,
            descriptor,
        } => lower_type_test(fl, value_local, *target_ty, *static_fold, descriptor),
        LirRvalue::Cast {
            value_local,
            target_ty,
            descriptor,
            failure,
        } => lower_cast(fl, value_local, *target_ty, descriptor, failure),
        LirRvalue::PatternMatch {
            subject_local,
            pattern,
        } => lower_pattern_match(fl, subject_local, pattern),
        LirRvalue::PatternExtract {
            subject_local,
            path,
            result_ty,
        } => lower_pattern_extract(fl, subject_local, path, *result_ty),
        LirRvalue::InterpolatedString { parts } => lower_interpolated_string(fl, parts),
        LirRvalue::WithUpdate {
            base_local,
            updates,
            result_ty,
        } => lower_with_update(fl, base_local, updates, *result_ty),
        LirRvalue::EnumVariant {
            enum_ty,
            tag_value,
            args,
            payload_ty,
            ..
        } => lower_enum_variant(fl, *enum_ty, *tag_value, args, *payload_ty),
        LirRvalue::ClassCtor { class_fqn, args } => lower_class_ctor(fl, class_fqn, args),
        LirRvalue::MakeArray {
            elements,
            ty,
            mutable,
        } => lower_make_array(fl, elements, *ty, *mutable),
        LirRvalue::MakeClosure {
            env_local,
            invoke_fqn,
        } => lower_make_closure(fl, env_local, invoke_fqn),
        LirRvalue::ClassLit { type_fqn } => lower_class_lit(fl, type_fqn),
        LirRvalue::MakeContinuation { state } => lower_make_continuation(fl, *state),
        LirRvalue::MakeChainLink { state } => lower_make_chain_link(fl, *state),
        LirRvalue::TakeChainLink { result_ty } => lower_take_chain_link(fl, *result_ty),
        LirRvalue::ResumeChainLink {
            link_slot,
            result_ty,
        } => lower_resume_chain_link(fl, *link_slot, *result_ty),
    }
}

/// `StructLit { fields, ty }` → LLVM insertvalue 链。
fn lower_struct_lit<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    fields: &[(String, LirOperand)],
    ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let struct_llvm_ty = fl.cg.lower_type(ty, fl.layouts)?;
    let struct_ty = super::expect_struct_type(struct_llvm_ty, "StructLit 类型", &fl.fqn)?;
    let mut agg = struct_ty.const_zero();
    // 字段顺序：按 LIR fields 顺序插入。注意：struct 字段索引需按声明顺序；
    // 当前 LIR StructLit 的 fields 已按声明顺序（与 Struct layout 一致）。
    for (i, (_, operand)) in fields.iter().enumerate() {
        let field_ty = match fl.layouts.get(ty) {
            Some(l) => match &l.kind {
                scoop2_lir::TypeLayoutKind::Struct { fields: fs } => fs.get(i).map(|f| f.ty),
                _ => None,
            },
            None => None,
        }
        .ok_or_else(|| {
            CodegenError::missing_layout(ty.0, "StructLit field ty", scoop2_base::Span::default())
        })?;
        let val = fl.lower_operand(operand, field_ty)?;
        let inserted = fl
            .builder
            .build_insert_value(agg, val, i as u32, &format!("sf{}", i))
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "build_insert_value(struct)",
                    scoop2_base::Span::default(),
                )
            })?;
        agg = inserted.into_struct_value();
    }
    Ok(agg.into())
}

/// `IntEq { lhs, rhs }` → LLVM icmp eq（返回 Bool i8）。
fn lower_int_eq<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    lhs: &LirOperand,
    rhs: &LirOperand,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let lhs_val = match lhs {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let rhs_val = match rhs {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let eq = fl
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            super::expect_int_val(lhs_val, "IntEq lhs", &fl.fqn)?,
            super::expect_int_val(rhs_val, "IntEq rhs", &fl.fqn)?,
            "eq",
        )
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "build_icmp eq", scoop2_base::Span::default())
        })?;
    Ok(fl
        .builder
        .build_int_z_extend(eq, fl.cg.context.i8_type(), "eq_i8")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "build_z_ext", scoop2_base::Span::default())
        })?
        .into())
}

/// `MakeTuple { elements, ty }` → LLVM insertvalue 链。
fn lower_make_tuple<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    elements: &[LirOperand],
    ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let tuple_llvm_ty = fl.cg.lower_type(ty, fl.layouts)?;
    let struct_ty = super::expect_struct_type(tuple_llvm_ty, "MakeTuple 类型", &fl.fqn)?;
    let mut agg = struct_ty.const_zero();
    for (i, operand) in elements.iter().enumerate() {
        // 取第 i 个字段的类型。
        let field_ty = match fl.layouts.get(ty) {
            Some(l) => match &l.kind {
                scoop2_lir::TypeLayoutKind::Tuple { elements: fs } => fs.get(i).map(|f| f.ty),
                _ => None,
            },
            None => None,
        }
        .ok_or_else(|| {
            CodegenError::missing_layout(ty.0, "MakeTuple field ty", scoop2_base::Span::default())
        })?;
        let val = fl.lower_operand(operand, field_ty)?;
        let inserted = fl
            .builder
            .build_insert_value(agg, val, i as u32, &format!("tup{}", i))
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "build_insert_value",
                    scoop2_base::Span::default(),
                )
            })?;
        // build_insert_value 返回 AggregateValueEnum；StructValue 可恢复。
        agg = inserted.into_struct_value();
    }
    Ok(agg.into())
}

/// `TupleIndex { receiver, index }` → LLVM extractvalue。
fn lower_tuple_index<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    index: u32,
    element_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // EffectStep frame 特例：receiver 是 frame local 时，frame 在堆上
    //（GC 对象，header + tuple payload），按字节偏移具类型 load，
    // 不走 extractvalue（frame local 的 alloca 里不是 tuple 值）。
    if let LirOperand::Local(id) = receiver
        && fl.is_effect_frame_local(*id)
    {
        let slot_ptr = fl.frame_slot_ptr_at(index as u64)?;
        let elem_llvm = fl.cg.lower_type(element_ty, fl.layouts)?;
        return fl
            .builder
            .build_load(elem_llvm, slot_ptr, &format!("frame_ld{}", index))
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "frame slot load", scoop2_base::Span::default())
            });
    }
    // receiver 的 tuple 值（StructValue），或指向 tuple 的引用（GC ptr → deref）。
    let agg = load_struct_or_deref(fl, receiver)?;
    let _ = element_ty;
    let v = fl
        .builder
        .build_extract_value(agg, index, &format!("ti{}", index))
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "build_extract_value",
                scoop2_base::Span::default(),
            )
        })?;
    Ok(v)
}

/// `MakeContinuation { state }` → canonical continuation 堆对象（GC ptr）。
///
/// 布局（`scoop2_lir::effect::CONT_OFFSET_*`）：
///   header(0..32) | resumed(32, i8) | state(40, i64) | frame(48) | step_fn(56) | resume_value(64)
/// descriptor bitmap = 0b100（只 trace frame 指针）。
/// 只能出现在 EffectStep 函数体内（frame 指针 / step_fn 从 effect ctx 推导）。
fn lower_make_continuation<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    state: u64,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use scoop2_lir::effect::{
        CONT_OFFSET_FRAME, CONT_OFFSET_RESUME_VALUE, CONT_OFFSET_RESUMED, CONT_OFFSET_STATE,
        CONT_OFFSET_STEP_FN, CONT_SIZE_BYTES,
    };
    let llvm = |e: inkwell::builder::BuilderError, what: &str| {
        CodegenError::llvm(e.to_string(), what, scoop2_base::Span::default())
    };
    let step_fn_sym = match &fl.effect {
        Some(e) => e.step_fn_sym.clone(),
        None => {
            return Err(CodegenError::llvm(
                "MakeContinuation 出现在非 EffectStep 函数".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let i64_ty = fl.cg.context.i64_type();
    let i8_ty = fl.cg.context.i8_type();
    let native_ptr = fl.cg.native_ptr_ty();
    // 1. 堆分配（descriptor bitmap = 0b100）。
    let desc = fl.cg.get_or_create_continuation_type_descriptor();
    let cont = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[desc.into(), i64_ty.const_int(CONT_SIZE_BYTES, false).into()],
            "cont_alloc",
        )
        .map_err(|e| llvm(e, "alloc continuation"))?;
    let cont_gc = match cont.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => {
            super::expect_ptr_val(v, "continuation alloc", &fl.fqn)?
        }
        inkwell::values::ValueKind::Instruction(_) => {
            return Err(CodegenError::llvm(
                "scoop_alloc_typed 未返回值".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let cont_int = fl
        .builder
        .build_ptr_to_int(cont_gc, i64_ty, "cont_int")
        .map_err(|e| llvm(e, "cont ptrtoint"))?;
    let cont_native = fl
        .builder
        .build_int_to_ptr(cont_int, native_ptr, "cont_native")
        .map_err(|e| llvm(e, "cont inttoptr"))?;
    let field = |off: u64, name: &str| -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
        unsafe {
            fl.builder
                .build_in_bounds_gep(i8_ty, cont_native, &[i64_ty.const_int(off, false)], name)
        }
        .map_err(|e| llvm(e, "gep continuation field"))
    };
    // 2. resumed = 0（i8）。
    fl.builder
        .build_store(field(CONT_OFFSET_RESUMED, "cont_resumed")?, i8_ty.const_zero())
        .map_err(|e| llvm(e, "store cont resumed"))?;
    // 3. state。
    fl.builder
        .build_store(
            field(CONT_OFFSET_STATE, "cont_state")?,
            i64_ty.const_int(state, false),
        )
        .map_err(|e| llvm(e, "store cont state"))?;
    // 4. frame 指针（当前函数的 frame，root slot 重载）。
    let frame = fl.effect_frame_ptr()?;
    fl.builder
        .build_store(field(CONT_OFFSET_FRAME, "cont_frame")?, frame)
        .map_err(|e| llvm(e, "store cont frame"))?;
    // 5. step_fn 地址（`sym$step` 函数）。
    let step_fv = fl.cg.module.get_function(&step_fn_sym).ok_or_else(|| {
        CodegenError::llvm(
            format!("step 函数 {} 未声明", step_fn_sym),
            &fl.fqn,
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder
        .build_store(
            field(CONT_OFFSET_STEP_FN, "cont_stepfn")?,
            step_fv.as_global_value().as_pointer_value(),
        )
        .map_err(|e| llvm(e, "store cont step_fn"))?;
    // 6. resume_value = 0。
    fl.builder
        .build_store(
            field(CONT_OFFSET_RESUME_VALUE, "cont_rv")?,
            i64_ty.const_zero(),
        )
        .map_err(|e| llvm(e, "store cont resume_value"))?;
    Ok(cont_gc.into())
}

/// `MakeChainLink { state }`：构造 chain link 对象并写入 TLS
/// `__scoop_effect_chain`，产出 Unit（i8 0）。
///
/// 用于 EffectStep 函数向外传播 callee 的挂起：link 记录本函数的 frame 与
/// `sym$step` 地址，外层 resume 时沿链逐层恢复。`state` 是本函数传播路径的
/// 续点编号（外层 ResumeChainLink 调用本层 step_fn 时经 word 无关的 state
/// dispatch 进入对应续点）。
fn lower_make_chain_link<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    state: u64,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use scoop2_lir::effect::{LINK_OFFSET_FRAME, LINK_OFFSET_STEP_FN, LINK_SIZE_BYTES};
    let llvm = |e: inkwell::builder::BuilderError, what: &str| {
        CodegenError::llvm(e.to_string(), what, scoop2_base::Span::default())
    };
    let step_fn_sym = match &fl.effect {
        Some(e) => e.step_fn_sym.clone(),
        None => {
            return Err(CodegenError::llvm(
                "MakeChainLink 出现在非 EffectStep 函数".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let i64_ty = fl.cg.context.i64_type();
    let i8_ty = fl.cg.context.i8_type();
    let native_ptr = fl.cg.native_ptr_ty();
    // 1. 堆分配（descriptor bitmap = 0b01，trace frame 指针）。
    let desc = fl.cg.get_or_create_chain_link_type_descriptor();
    let link = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[desc.into(), i64_ty.const_int(LINK_SIZE_BYTES, false).into()],
            "link_alloc",
        )
        .map_err(|e| llvm(e, "alloc chain link"))?;
    let link_gc = match link.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => {
            super::expect_ptr_val(v, "chain link alloc", &fl.fqn)?
        }
        inkwell::values::ValueKind::Instruction(_) => {
            return Err(CodegenError::llvm(
                "scoop_alloc_typed 未返回值".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let link_int = fl
        .builder
        .build_ptr_to_int(link_gc, i64_ty, "link_int")
        .map_err(|e| llvm(e, "link ptrtoint"))?;
    let link_native = fl
        .builder
        .build_int_to_ptr(link_int, native_ptr, "link_native")
        .map_err(|e| llvm(e, "link inttoptr"))?;
    let field = |off: u64, name: &str| -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
        unsafe {
            fl.builder
                .build_in_bounds_gep(i8_ty, link_native, &[i64_ty.const_int(off, false)], name)
        }
        .map_err(|e| llvm(e, "gep chain link field"))
    };
    // 2. frame 指针（当前函数的 frame，root slot 重载）。
    let frame = fl.effect_frame_ptr()?;
    fl.builder
        .build_store(field(LINK_OFFSET_FRAME, "link_frame")?, frame)
        .map_err(|e| llvm(e, "store link frame"))?;
    // 3. step_fn 地址（`sym$step` 函数）。
    let step_fv = fl.cg.module.get_function(&step_fn_sym).ok_or_else(|| {
        CodegenError::llvm(
            format!("step 函数 {} 未声明", step_fn_sym),
            &fl.fqn,
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder
        .build_store(
            field(LINK_OFFSET_STEP_FN, "link_stepfn")?,
            step_fv.as_global_value().as_pointer_value(),
        )
        .map_err(|e| llvm(e, "store link step_fn"))?;
    // 4. state 仅供诊断对称性——chain link 不带 state 字段：本层 frame 的
    //    state 槽（slot 0）在挂出前已由 housekeeping 写入，resume 时 step_fn
    //    从 frame 自身读取 state 做 dispatch。此处只把 link 写入 TLS。
    let _ = state;
    let tls = fl.cg.effect_chain_global();
    fl.builder
        .build_store(tls, link_native)
        .map_err(|e| llvm(e, "store effect chain TLS"))?;
    // 产出 Unit（i8 0）。
    Ok(i8_ty.const_zero().into())
}

/// `TakeChainLink { result_ty }`：读取 TLS `__scoop_effect_chain` 并清零
/// （消费语义），产出 GC 指针（Any ref）。
fn lower_take_chain_link<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    _result_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let llvm = |e: inkwell::builder::BuilderError, what: &str| {
        CodegenError::llvm(e.to_string(), what, scoop2_base::Span::default())
    };
    let i64_ty = fl.cg.context.i64_type();
    let native_ptr = fl.cg.native_ptr_ty();
    let tls = fl.cg.effect_chain_global();
    let link_native = fl
        .builder
        .build_load(native_ptr, tls, "chain_take")
        .map_err(|e| llvm(e, "load effect chain TLS"))?
        .into_pointer_value();
    // 清零（消费语义：每个 link 只被取走一次）。
    fl.builder
        .build_store(tls, native_ptr.const_null())
        .map_err(|e| llvm(e, "clear effect chain TLS"))?;
    // native ptr → GC ptr（Any ref）。
    let as_int = fl
        .builder
        .build_ptr_to_int(link_native, i64_ty, "chain_take_int")
        .map_err(|e| llvm(e, "chain take ptrtoint"))?;
    let gc_ptr = fl
        .builder
        .build_int_to_ptr(as_int, fl.cg.gc_ptr_ty(), "chain_take_gc")
        .map_err(|e| llvm(e, "chain take inttoptr"))?;
    Ok(gc_ptr.into())
}

/// `ResumeChainLink { link_slot, result_ty }`：从本函数 frame 的 link 槽取出
/// chain link，间接调用 `step_fn(link.frame, resume_word)`，产出 callee 的
/// Step 值（result_ty）。
///
/// 仅出现在 EffectStep 函数的 call-chain resume 续点块；resume word 从
/// step 函数参数（经 `resume_word_alloca`）读取。
fn lower_resume_chain_link<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    link_slot: u64,
    result_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use scoop2_lir::effect::{LINK_OFFSET_FRAME, LINK_OFFSET_STEP_FN};
    let llvm = |e: inkwell::builder::BuilderError, what: &str| {
        CodegenError::llvm(e.to_string(), what, scoop2_base::Span::default())
    };
    let word_alloca = match &fl.effect {
        Some(e) => e.resume_word_alloca.ok_or_else(|| {
            CodegenError::llvm(
                "ResumeChainLink：step 函数缺 resume word alloca".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            )
        })?,
        None => {
            return Err(CodegenError::llvm(
                "ResumeChainLink 出现在非 EffectStep 函数".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let i64_ty = fl.cg.context.i64_type();
    let i8_ty = fl.cg.context.i8_type();
    let native_ptr = fl.cg.native_ptr_ty();
    // 1. 从 frame 槽 load link（槽类型 Any ref = GC ptr）。
    let slot_ptr = fl.frame_slot_ptr_at(link_slot)?;
    let link_gc = fl
        .builder
        .build_load(fl.cg.gc_ptr_ty(), slot_ptr, "chain_resume_link")
        .map_err(|e| llvm(e, "load chain link slot"))?
        .into_pointer_value();
    let link_int = fl
        .builder
        .build_ptr_to_int(link_gc, i64_ty, "chain_resume_int")
        .map_err(|e| llvm(e, "chain resume ptrtoint"))?;
    let link_native = fl
        .builder
        .build_int_to_ptr(link_int, native_ptr, "chain_resume_native")
        .map_err(|e| llvm(e, "chain resume inttoptr"))?;
    // 2. null 检查（防御：link 槽在 capture 时必已填入）。
    let is_null = fl
        .builder
        .build_is_null(link_native, "chain_link_null")
        .map_err(|e| llvm(e, "chain link null check"))?;
    let ok_bb = fl.cg.context.append_basic_block(fl.fv, "chain_link_ok");
    let panic_bb = fl.cg.context.append_basic_block(fl.fv, "chain_link_panic");
    fl.builder
        .build_conditional_branch(is_null, panic_bb, ok_bb)
        .map_err(|e| llvm(e, "chain link br"))?;
    fl.builder.position_at_end(panic_bb);
    let msg = fl
        .cg
        .get_or_create_string_literal("continuation chain broken")?;
    let msg_int = fl
        .builder
        .build_ptr_to_int(msg, i64_ty, "chain_panic_msg_int")
        .map_err(|e| llvm(e, "chain panic msg int"))?;
    let msg_native = fl
        .builder
        .build_int_to_ptr(msg_int, native_ptr, "chain_panic_msg")
        .map_err(|e| llvm(e, "chain panic msg"))?;
    fl.builder
        .build_call(fl.rt.panic, &[msg_native.into()], "chain_panic_call")
        .map_err(|e| llvm(e, "chain panic call"))?;
    fl.builder
        .build_unreachable()
        .map_err(|e| llvm(e, "chain panic unreachable"))?;
    fl.builder.position_at_end(ok_bb);
    // 3. 读 link.frame / link.step_fn。
    let field = |off: u64, name: &str| -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
        unsafe {
            fl.builder
                .build_in_bounds_gep(i8_ty, link_native, &[i64_ty.const_int(off, false)], name)
        }
        .map_err(|e| llvm(e, "gep chain link field"))
    };
    let frame_ptr = fl
        .builder
        .build_load(native_ptr, field(LINK_OFFSET_FRAME, "cl_frame_slot")?, "cl_frame")
        .map_err(|e| llvm(e, "load link frame"))?
        .into_pointer_value();
    let step_fn = fl
        .builder
        .build_load(native_ptr, field(LINK_OFFSET_STEP_FN, "cl_stepfn_slot")?, "cl_stepfn")
        .map_err(|e| llvm(e, "load link step_fn"))?
        .into_pointer_value();
    // 4. resume word（step 函数参数）。
    let word = fl
        .builder
        .build_load(i64_ty, word_alloca, "cl_word")
        .map_err(|e| llvm(e, "load resume word"))?;
    // 5. 间接调用 step_fn(frame, word) → callee Step（result_ty）。
    let step_ret_llvm = fl.cg.lower_type(result_ty, fl.layouts)?;
    let step_fn_ty = match step_ret_llvm {
        inkwell::types::BasicTypeEnum::IntType(t) => {
            t.fn_type(&[native_ptr.into(), i64_ty.into()], false)
        }
        inkwell::types::BasicTypeEnum::FloatType(t) => {
            t.fn_type(&[native_ptr.into(), i64_ty.into()], false)
        }
        inkwell::types::BasicTypeEnum::PointerType(t) => {
            t.fn_type(&[native_ptr.into(), i64_ty.into()], false)
        }
        inkwell::types::BasicTypeEnum::StructType(t) => {
            t.fn_type(&[native_ptr.into(), i64_ty.into()], false)
        }
        inkwell::types::BasicTypeEnum::ArrayType(t) => {
            t.fn_type(&[native_ptr.into(), i64_ty.into()], false)
        }
        inkwell::types::BasicTypeEnum::VectorType(t) => {
            t.fn_type(&[native_ptr.into(), i64_ty.into()], false)
        }
        inkwell::types::BasicTypeEnum::ScalableVectorType(_) => {
            return Err(CodegenError::llvm(
                "Step 类型不合法".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let call = fl
        .builder
        .build_indirect_call(step_fn_ty, step_fn, &[frame_ptr.into(), word.into()], "cl_step_call")
        .map_err(|e| llvm(e, "chain step call"))?;
    match call.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => Ok(v),
        inkwell::values::ValueKind::Instruction(_) => Err(CodegenError::llvm(
            "ResumeChainLink: step_fn 未返回值".to_string(),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 加载一个聚合值（Struct/Tuple）。
///
/// 若值本身是 StructValue，直接返回。
/// 若值是指针（GC ref 或 native ptr），按其指向的聚合类型 deref + load。
fn load_struct_or_deref<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    operand: &LirOperand,
) -> CodegenResult<inkwell::values::StructValue<'ctx>> {
    let val = match operand {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    match val {
        BasicValueEnum::StructValue(s) => Ok(s),
        BasicValueEnum::PointerValue(p) => {
            // 指针 → deref：按 local 声明类型 load struct。
            let ty = match operand {
                LirOperand::Local(id) => fl.local_types.get(id).copied(),
                _ => None,
            };
            let native = if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, fl.cg.context.i64_type(), "deref_int")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), "deref_int", scoop2_base::Span::default())
                    })?;
                fl.builder
                    .build_int_to_ptr(as_int, fl.cg.native_ptr_ty(), "deref_native")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "deref_native",
                            scoop2_base::Span::default(),
                        )
                    })?
            } else {
                p
            };
            let agg_ty = ty
                .and_then(|t| fl.cg.lower_type(t, fl.layouts).ok())
                .map(|t| super::expect_struct_type(t, "aggregate deref 类型", &fl.fqn))
                .transpose()?
                .ok_or_else(|| {
                    CodegenError::llvm(
                        "aggregate deref：local 类型未知，无法确定聚合类型",
                        "load_struct_or_deref",
                        scoop2_base::Span::default(),
                    )
                })?;
            let loaded = fl
                .builder
                .build_load(agg_ty, native, "deref_agg")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "deref_load", scoop2_base::Span::default())
                })?;
            Ok(loaded.into_struct_value())
        }
        _ => Err(CodegenError::llvm(
            "expected struct or pointer for aggregate access",
            "load_struct_or_deref",
            scoop2_base::Span::default(),
        )),
    }
}

/// `MemberAccess { receiver, member }` → struct extractvalue / class GEP+load。
fn lower_member_access<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    member_name: &str,
    lir_field_offset: u64,
    result_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let (recv_val, recv_ty) = lower_receiver_with_ty(fl, receiver)?;
    let layout = fl.layouts.get(recv_ty).ok_or_else(|| {
        CodegenError::missing_layout(
            recv_ty.0,
            "MemberAccess receiver",
            scoop2_base::Span::default(),
        )
    })?;
    match &layout.kind {
        scoop2_lir::TypeLayoutKind::Struct { fields }
        | scoop2_lir::TypeLayoutKind::Tuple { elements: fields } => {
            // struct/tuple 值类型：extractvalue。
            // 字段索引按 LIR field_offset（来自 TypeLayoutTable，与布局同源）
            // 在 layout fields 中定位；辅以字段类型校验，拒绝按类型猜第一个
            // （两个同类型字段的旧逻辑会静默错位到字段 0）。
            let field_idx = fields
                .iter()
                .position(|f| f.offset == lir_field_offset && f.ty == result_ty)
                .or_else(|| fields.iter().position(|f| f.offset == lir_field_offset))
                .ok_or_else(|| {
                    CodegenError::llvm(
                        format!(
                            "MemberAccess {}: 布局中找不到 offset={} 的字段",
                            member_name, lir_field_offset
                        ),
                        &fl.fqn,
                        scoop2_base::Span::default(),
                    )
                })?;
            let agg = match recv_val {
                BasicValueEnum::StructValue(s) => s,
                BasicValueEnum::PointerValue(p) => {
                    // struct 通过引用存储：deref + load struct。
                    let native =
                        if p.get_type().get_address_space() == crate::context::gc_address_space() {
                            let as_int = fl
                                .builder
                                .build_ptr_to_int(p, fl.cg.context.i64_type(), "ma_deref_int")
                                .map_err(|e| {
                                    CodegenError::llvm(
                                        e.to_string(),
                                        "ma_deref_int",
                                        scoop2_base::Span::default(),
                                    )
                                })?;
                            fl.builder
                                .build_int_to_ptr(as_int, fl.cg.native_ptr_ty(), "ma_deref_native")
                                .map_err(|e| {
                                    CodegenError::llvm(
                                        e.to_string(),
                                        "ma_deref_native",
                                        scoop2_base::Span::default(),
                                    )
                                })?
                        } else {
                            p
                        };
                    let agg_ty = super::expect_struct_type(
                        fl.cg.lower_type(recv_ty, fl.layouts)?,
                        "MemberAccess receiver 类型",
                        &fl.fqn,
                    )?;
                    fl.builder
                        .build_load(agg_ty, native, "ma_deref_agg")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "ma_deref_load",
                                scoop2_base::Span::default(),
                            )
                        })?
                        .into_struct_value()
                }
                _ => {
                    return Err(CodegenError::llvm(
                        "MemberAccess struct expected struct/ptr",
                        "member_access",
                        scoop2_base::Span::default(),
                    ));
                }
            };
            let v = fl
                .builder
                .build_extract_value(agg, field_idx as u32, "member")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "extract member",
                        scoop2_base::Span::default(),
                    )
                })?;
            Ok(v)
        }
        scoop2_lir::TypeLayoutKind::Reference { .. } => {
            // class/interface 引用：GEP 到字段偏移 + load。
            // receiver 是 GC ptr → 转 native ptr → GEP(field_offset) → load。
            let native = fl
                .builder
                .build_ptr_to_int(
                    super::expect_ptr_val(recv_val, "MemberAccess class receiver", &fl.fqn)?,
                    fl.cg.context.i64_type(),
                    "recv2int",
                )
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "ptr_to_int member",
                        scoop2_base::Span::default(),
                    )
                })?;
            let native_ptr = fl
                .builder
                .build_int_to_ptr(native, fl.cg.native_ptr_ty(), "recv_native")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "int_to_ptr member",
                        scoop2_base::Span::default(),
                    )
                })?;
            // 查找 class 的字段列表，获取 member_name 对应的偏移。
            // 从 HIR members 表查 member 类型 + 从 LIR TypeLayout 查字段列表。
            // 简化：用 member_name 查 HIR，获取字段类型，然后匹配 TypeLayout 字段。
            let header_size = fl
                .cg
                .target_data
                .get_store_size(&fl.cg.object_header_type());
            // 尝试按 member_name 在 class layout 中查找字段偏移。
            // LIR TypeLayout for Reference 类型只有 ref_kind，不含 payload 字段列表。
            // 使用 HIR members 查 member 类型，然后从 MIR struct_layouts 查偏移。
            // 简化：按 result_ty 在全局 struct_layouts 中查找。
            // 回退：假设字段在 header 之后，按声明顺序排列。
            // 查 HIR members 表获取 field index。
            let class_fqn_text = match fl.layouts.get(recv_ty) {
                Some(l) => match &l.kind {
                    scoop2_lir::TypeLayoutKind::Reference {
                        ref_kind: scoop2_lir::RefKind::Class,
                        ..
                    } => "class",
                    _ => "unknown",
                },
                None => "unknown",
            };
            // field_offset 由 LIR compute_field_offset 预计算（含正确 header_size + 对齐）。
            let _ = class_fqn_text;
            // LIR field_offset 已含正确的 header_size(32) + 对齐字段偏移（与 class_ctor 一致）。
            let field_offset = lir_field_offset;
            let field_llvm_ty = fl.cg.lower_type(result_ty, fl.layouts)?;
            let _ = member_name;
            let field_slot = unsafe {
                fl.builder.build_in_bounds_gep(
                    fl.cg.context.i8_type(),
                    native_ptr,
                    &[fl.cg.context.i64_type().const_int(field_offset, false)],
                    "field_slot",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "gep field_slot",
                    scoop2_base::Span::default(),
                )
            })?;
            let val = fl
                .builder
                .build_load(field_llvm_ty, field_slot, "field_val")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "load field_val",
                        scoop2_base::Span::default(),
                    )
                })?;
            Ok(val)
        }
        _ => Err(CodegenError::unsupported(
            format!("MemberAccess 不支持的布局 {:?}", layout.kind),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 取 receiver 的值 + 类型。
fn lower_receiver_with_ty<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
) -> CodegenResult<(BasicValueEnum<'ctx>, TypeId)> {
    match receiver {
        LirOperand::Local(id) => {
            let ty = fl.local_types.get(id).copied().ok_or_else(|| {
                CodegenError::unsupported(
                    format!("receiver local {} 类型未知", id),
                    &fl.fqn,
                    scoop2_base::Span::default(),
                )
            })?;
            Ok((fl.load_local_typed(*id, ty)?, ty))
        }
        LirOperand::Const(_) => Err(CodegenError::unsupported(
            "MemberAccess 的 receiver 不能是 Const",
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 取 rvalue 变体名（用于诊断）。
fn discriminant(rv: &LirRvalue) -> &'static str {
    match rv {
        LirRvalue::Use(_) => "Use",
        LirRvalue::Const(_) => "Const",
        LirRvalue::Call(_) => "Call",
        LirRvalue::TopLevelRef { .. } => "TopLevelRef",
        LirRvalue::MemberAccess { .. } => "MemberAccess",
        LirRvalue::TupleIndex { .. } => "TupleIndex",
        LirRvalue::IndexAccess { .. } => "IndexAccess",
        LirRvalue::TypeTest { .. } => "TypeTest",
        LirRvalue::Cast { .. } => "Cast",
        LirRvalue::PatternMatch { .. } => "PatternMatch",
        LirRvalue::PatternExtract { .. } => "PatternExtract",
        LirRvalue::IntEq { .. } => "IntEq",
        LirRvalue::InterpolatedString { .. } => "InterpolatedString",
        LirRvalue::WithUpdate { .. } => "WithUpdate",
        LirRvalue::EnumVariant { .. } => "EnumVariant",
        LirRvalue::ClassCtor { .. } => "ClassCtor",
        LirRvalue::MakeTuple { .. } => "MakeTuple",
        LirRvalue::MakeArray { .. } => "MakeArray",
        LirRvalue::StructLit { .. } => "StructLit",
        LirRvalue::MakeContinuation { .. } => "MakeContinuation",
        LirRvalue::MakeChainLink { .. } => "MakeChainLink",
        LirRvalue::TakeChainLink { .. } => "TakeChainLink",
        LirRvalue::ResumeChainLink { .. } => "ResumeChainLink",
        LirRvalue::MakeClosure { .. } => "MakeClosure",
        LirRvalue::ClassLit { .. } => "ClassLit",
    }
}

use inkwell::values::AsValueRef;

// =========================================================================
// W1-4/W1-5 剩余 rvalue 实现
// =========================================================================

use scoop2_lir::{LirInterpolatedPart, LirPattern, LirWithUpdateField};

/// `TopLevelRef { fqn }` → 加载全局变量 backing slot。
/// 顶层 val 在 entry main 中初始化（存入全局 alloca）；此处加载。
fn lower_top_level_ref<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    fqn: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 查找已声明的全局。
    if let Some(gv) = fl.cg.lookup_global(fqn) {
        let ty = fl
            .layouts
            .entries
            .keys()
            .next()
            .copied()
            .unwrap_or(scoop2_hir::ty::TypeId(0));
        let _ = ty;
        // 全局 backing slot 是 native ptr (alloca-like)；但 top-level val 的类型
        // 由 LIR TypeLayout 决定。当前简化：返回全局指针 cast 到 GC ptr。
        let val = fl
            .builder
            .build_load(fl.cg.native_ptr_ty(), gv, "toplevel")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "load toplevel", scoop2_base::Span::default())
            })?;
        return Ok(val);
    }
    // 未找到全局：返回 zero（防御性）。
    Err(CodegenError::undefined_symbol(
        fqn,
        &format!("top-level ref in {}", fl.fqn),
        scoop2_base::Span::default(),
    ))
}

/// `IndexAccess { receiver, indices, element_ty }` → 数组索引访问。
///
/// 两种布局（由 LIR `receiver_mutable` 分派，见 runtime/c/scoop_array_internal.h）：
/// - 不可变 `Array<T>`（ScoopArray）：`{ header; u64 len; u64 elem_size_bytes;
///   u64 data_offset_bytes; ptr elem_desc; u32 elem_kind; u32 _reserved; u8 data[] }`，
///   元素 data 内联在对象中，元素地址 = arr + data_offset_bytes + idx * elem_size。
/// - `MutableArray<T>`（ScoopMutableArray）：`{ header; u64 len; u64 cap;
///   u64 elem_size_bytes; u64 elem_align_bytes; ptr elem_desc; ptr data; ... }`，
///   data 是外置指针，元素地址 = data + idx * elem_size。
///
/// 越界（含负 index，按无符号比较统一捕获）调用 scoop_panic，与 Panic stmt 同路径。
fn lower_index_access<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    indices: &[LirOperand],
    element_ty: scoop2_hir::ty::TypeId,
    receiver_mutable: bool,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 当前简化：仅支持一维索引。receiver 是 Array/MutableArray<T> (GC ptr)。
    let recv_gc = match receiver {
        LirOperand::Local(id) => {
            super::expect_ptr_val(fl.load_local(*id)?, "IndexAccess receiver", &fl.fqn)?
        }
        LirOperand::Const(c) => {
            super::expect_ptr_val(fl.lower_const_value(c)?, "IndexAccess receiver", &fl.fqn)?
        }
    };
    let idx_raw = match indices.first() {
        Some(LirOperand::Local(id)) => {
            super::expect_int_val(fl.load_local(*id)?, "IndexAccess 索引", &fl.fqn)?
        }
        Some(LirOperand::Const(c)) => {
            super::expect_int_val(fl.lower_const_value(c)?, "IndexAccess 索引", &fl.fqn)?
        }
        None => {
            return Err(CodegenError::unsupported(
                "IndexAccess 无索引",
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    let i64_ty = fl.cg.context.i64_type();
    // 索引统一为 i64（有符号语义：小宽度 sext，负值保持负值，交由无符号比较捕获）。
    let idx = normalize_int_to_i64(fl, idx_raw, "arr_idx")?;
    let native = fl
        .builder
        .build_ptr_to_int(recv_gc, i64_ty, "arr2int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int arr",
                scoop2_base::Span::default(),
            )
        })?;
    let native_ptr = fl
        .builder
        .build_int_to_ptr(native, fl.cg.native_ptr_ty(), "arr_native")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr arr",
                scoop2_base::Span::default(),
            )
        })?;
    let header_size = fl
        .cg
        .target_data
        .get_store_size(&fl.cg.object_header_type());
    // len 在两种布局里都在 header + 0。
    let len_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            native_ptr,
            &[i64_ty.const_int(header_size, false)],
            "arr_len_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep arr_len", scoop2_base::Span::default()))?;
    let len = fl
        .builder
        .build_load(i64_ty, len_slot, "arr_len")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "load arr_len", scoop2_base::Span::default())
        })?
        .into_int_value();
    build_array_bounds_check(fl, idx, len)?;
    let elem_addr = if receiver_mutable {
        // MutableArray：elem_size_bytes @ header+16，data 外置指针 @ header+40。
        let stride = load_i64_field(fl, native_ptr, header_size + 16, "arr_esz")?;
        let data_ptr = fl
            .builder
            .build_load(
                fl.cg.native_ptr_ty(),
                unsafe {
                    fl.builder.build_in_bounds_gep(
                        fl.cg.context.i8_type(),
                        native_ptr,
                        &[i64_ty.const_int(header_size + 40, false)],
                        "arr_data_slot",
                    )
                }
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "gep arr_data", scoop2_base::Span::default())
                })?,
                "arr_data",
            )
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "load arr_data", scoop2_base::Span::default())
            })?
            .into_pointer_value();
        let byte_offset = fl
            .builder
            .build_int_mul(idx, stride, "arr_elem_off")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "mul elem off", scoop2_base::Span::default())
            })?;
        unsafe {
            fl.builder.build_in_bounds_gep(
                fl.cg.context.i8_type(),
                data_ptr,
                &[byte_offset],
                "arr_elem_i8",
            )
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), "gep elem", scoop2_base::Span::default()))?
    } else {
        // Array：elem_size_bytes @ header+8，data_offset_bytes @ header+16。
        let stride = load_i64_field(fl, native_ptr, header_size + 8, "arr_esz")?;
        let data_offset = load_i64_field(fl, native_ptr, header_size + 16, "arr_data_off")?;
        // 元素地址 = arr + data_offset + idx * stride。
        let byte_offset = fl
            .builder
            .build_int_mul(idx, stride, "arr_elem_off")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "mul elem off", scoop2_base::Span::default())
            })?;
        let elem_offset = fl
            .builder
            .build_int_add(data_offset, byte_offset, "arr_elem_abs_off")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "add elem off", scoop2_base::Span::default())
            })?;
        unsafe {
            fl.builder.build_in_bounds_gep(
                fl.cg.context.i8_type(),
                native_ptr,
                &[elem_offset],
                "arr_elem_i8",
            )
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), "gep elem", scoop2_base::Span::default()))?
    };
    // 按静态元素类型读取。
    let elem_is_ref = fl
        .layouts
        .get(element_ty)
        .map(|l| {
            matches!(
                &l.kind,
                scoop2_lir::TypeLayoutKind::Reference { .. } | scoop2_lir::TypeLayoutKind::Function
            )
        })
        .unwrap_or(false);
    if elem_is_ref {
        // 引用元素：槽内是对象指针，native → GC ptr。
        let elem_native = fl
            .builder
            .build_load(fl.cg.native_ptr_ty(), elem_addr, "elem")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "load elem", scoop2_base::Span::default())
            })?
            .into_pointer_value();
        let elem_int = fl
            .builder
            .build_ptr_to_int(elem_native, i64_ty, "elem_int")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "ptr_to_int elem",
                    scoop2_base::Span::default(),
                )
            })?;
        let elem_gc = fl
            .builder
            .build_int_to_ptr(elem_int, fl.cg.gc_ptr_ty(), "elem_gc")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "int_to_ptr elem",
                    scoop2_base::Span::default(),
                )
            })?;
        return Ok(elem_gc.into());
    }
    // WORD 元素：槽内是 zext 后的 i64，按元素 LLVM 类型截断/位转。
    let word = fl
        .builder
        .build_load(i64_ty, elem_addr, "elem_word")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "load elem word",
                scoop2_base::Span::default(),
            )
        })?
        .into_int_value();
    let elem_llvm = fl.cg.lower_type(element_ty, fl.layouts)?;
    let val: BasicValueEnum<'ctx> = match elem_llvm {
        inkwell::types::BasicTypeEnum::IntType(t) => {
            if t.get_bit_width() == 64 {
                word.into()
            } else {
                fl.builder
                    .build_int_truncate(word, t, "elem_trunc")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "trunc elem",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into()
            }
        }
        inkwell::types::BasicTypeEnum::FloatType(t) => {
            // Float 元素按位模式存在 8 字节槽中（f64 原样，f32 存低 32 位），
            // 读取时截断到位宽后 bitcast 回浮点（与 lower_make_array 的写入对称）。
            let bits = if t == fl.cg.context.f64_type() {
                word
            } else {
                fl.builder
                    .build_int_truncate(word, fl.cg.context.i32_type(), "elem_trunc")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "trunc elem",
                            scoop2_base::Span::default(),
                        )
                    })?
            };
            fl.builder
                .build_bit_cast(bits, t, "elem_fbits")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "bitcast elem", scoop2_base::Span::default())
                })?
        }
        _ => {
            return Err(CodegenError::unsupported(
                "IndexAccess 元素类型不支持（非标量/引用）",
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    Ok(val)
}

/// 把任意宽度整型规范化为 i64（小宽度按有符号 sext，保持索引的负值语义）。
pub fn normalize_int_to_i64<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    v: inkwell::values::IntValue<'ctx>,
    name: &str,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let i64_ty = fl.cg.context.i64_type();
    let bits = v.get_type().get_bit_width();
    if bits == 64 {
        return Ok(v);
    }
    if bits < 64 {
        return fl.builder.build_int_s_extend(v, i64_ty, name).map_err(|e| {
            CodegenError::llvm(e.to_string(), "sext idx", scoop2_base::Span::default())
        });
    }
    fl.builder
        .build_int_truncate(v, i64_ty, name)
        .map_err(|e| CodegenError::llvm(e.to_string(), "trunc idx", scoop2_base::Span::default()))
}

/// 从对象 native 指针按绝对字节偏移加载一个 u64 字段。
fn load_i64_field<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    obj_native: inkwell::values::PointerValue<'ctx>,
    byte_offset: u64,
    name: &str,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let i64_ty = fl.cg.context.i64_type();
    let slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_native,
            &[i64_ty.const_int(byte_offset, false)],
            &format!("{name}_slot"),
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep field", scoop2_base::Span::default()))?;
    Ok(fl
        .builder
        .build_load(i64_ty, slot, name)
        .map_err(|e| CodegenError::llvm(e.to_string(), "load field", scoop2_base::Span::default()))?
        .into_int_value())
}

/// 数组边界检查：`idx >=u len`（无符号比较同时捕获负 index——负值按无符号解释
/// 为巨大正数）时分叉到 panic 块：调用 scoop_panic（与 Panic stmt 同一路径）后
/// unreachable；否则继续落在当前插入点之后的 ok 块。
pub fn build_array_bounds_check<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    idx: inkwell::values::IntValue<'ctx>,
    len: inkwell::values::IntValue<'ctx>,
) -> CodegenResult<()> {
    let oob = fl
        .builder
        .build_int_compare(inkwell::IntPredicate::UGE, idx, len, "arr_oob")
        .map_err(|e| CodegenError::llvm(e.to_string(), "cmp oob", scoop2_base::Span::default()))?;
    let cur_bb = fl.builder.get_insert_block().ok_or_else(|| {
        CodegenError::llvm(
            "bounds check 无插入点",
            "bounds_check",
            scoop2_base::Span::default(),
        )
    })?;
    let parent_fn = cur_bb.get_parent().ok_or_else(|| {
        CodegenError::llvm(
            "bounds check 所在块无父函数",
            "bounds_check",
            scoop2_base::Span::default(),
        )
    })?;
    let panic_bb = fl.cg.context.append_basic_block(parent_fn, "arr_oob_panic");
    let ok_bb = fl.cg.context.append_basic_block(parent_fn, "arr_inbounds");
    fl.builder
        .build_conditional_branch(oob, panic_bb, ok_bb)
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "condbr oob", scoop2_base::Span::default())
        })?;
    fl.builder.position_at_end(panic_bb);
    let msg = fl
        .cg
        .get_or_create_string_literal("array index out of bounds")?;
    // scoop_panic 是 native void*；string literal 是 GC ptr（addrspace 1），
    // 跨地址空间不能 bitcast，走 ptrtoint/inttoptr。
    let msg_int = fl
        .builder
        .build_ptr_to_int(msg, fl.cg.context.i64_type(), "oob_msg_int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int oob msg",
                scoop2_base::Span::default(),
            )
        })?;
    let native = fl
        .builder
        .build_int_to_ptr(msg_int, fl.cg.native_ptr_ty(), "oob_msg")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr oob msg",
                scoop2_base::Span::default(),
            )
        })?;
    let _ = fl
        .builder
        .build_call(fl.rt.panic, &[native.into()], "oob_panic")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "call oob panic",
                scoop2_base::Span::default(),
            )
        })?;
    fl.builder.build_unreachable().map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "unreachable oob",
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder.position_at_end(ok_bb);
    Ok(())
}

/// `TypeTest { value, target_ty }` → `is T` 类型检查。
///
/// 运行期读取 value 对象头部的 type_desc → type_id，与 target FQN 的
/// type_id（FNV-1a 哈希）比较。命中返回 1（true），否则 0（false）。
///
/// 子类型：当前仅精确 type_id 比较。完整子类型匹配需遍历 parent_type_desc
/// 链或预计算的 type_id 集合；精确比较覆盖 sealed/final 类型的常见场景。
/// 若 metadata.static_fold != Dynamic，直接折叠为编译期常量。
fn lower_type_test<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &LirOperand,
    _target_ty: scoop2_hir::ty::TypeId,
    static_fold: scoop2_mir::mir::transport::RuntimeTypeStaticFold,
    descriptor: &scoop2_mir::mir::transport::RuntimeTypeDescriptorKey,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use scoop2_mir::mir::transport::RuntimeTypeStaticFold as Fold;
    // 静态折叠：编译期已知结果。
    match static_fold {
        Fold::AlwaysTrue => return Ok(fl.cg.context.i8_type().const_int(1, false).into()),
        Fold::AlwaysFalse => return Ok(fl.cg.context.i8_type().const_int(0, false).into()),
        Fold::Dynamic => {}
    }

    // 提取目标 FQN（Nominal descriptor）。
    let (target_fqn, is_string) = match &descriptor.kind {
        scoop2_mir::mir::transport::RuntimeTypeDescriptorKind::Nominal { fqn, .. } => {
            (fqn.clone(), false)
        }
        scoop2_mir::mir::transport::RuntimeTypeDescriptorKind::String => {
            ("scoop.core.String".to_string(), true)
        }
        // 非名义类型（Any/Tuple/Function/...）暂不支持动态匹配：保守返回 false。
        _ => return Ok(fl.cg.context.i8_type().const_int(0, false).into()),
    };

    let val = match value {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };

    // 若目标是 interface：遍历对象的 itable，查找匹配的 interface_id。
    if !is_string {
        if let Some(&target_iface_id) = fl.cg.interface_id_map.get(&target_fqn) {
            return interface_itable_matches(fl, val, target_iface_id);
        }
    }

    // 目标是 class（或 String）：精确 type_id 比较。
    let target_type_id = crate::globals::stable_hash_u64_pub(&target_fqn);
    let result = type_id_equals(fl, val, target_type_id)?;
    Ok(result.into())
}

/// 遍历对象的 itable 容器，查找是否存在 interface_id == target_iface_id 的条目。
/// 返回 i8（1 = 实现，0 = 未实现）。
///
/// itable 容器布局：`{ i32 count; i32 pad; ptr entries }`，
/// entries 指向 `{ u64 interface_id; ptr methods }[count]`。
fn interface_itable_matches<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    target_iface_id: u64,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let i32_ty = fl.cg.context.i32_type();
    let i64 = fl.cg.context.i64_type();
    let native_ptr = fl.cg.native_ptr_ty();

    // obj → native ptr。
    let obj_ptr = match val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "iim_ptr2int")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "iim_ptr2int",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder
                    .build_int_to_ptr(as_int, native_ptr, "iim_native")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "iim_int2ptr",
                            scoop2_base::Span::default(),
                        )
                    })?
            } else {
                p
            }
        }
        _ => return Ok(i8.const_int(0, false).into()),
    };

    // 读取 type_desc（header 第 2 个字）。
    let ptr_size = fl.cg.pointer_byte_size;
    let desc_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                fl.cg.context.i8_type(),
                obj_ptr,
                &[i64.const_int(ptr_size, false)],
                "iim_desc_slot",
            )
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "iim_gep_desc", scoop2_base::Span::default())
            })?
    };
    let type_desc = fl
        .builder
        .build_load(native_ptr, desc_slot, "iim_desc")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_load_desc", scoop2_base::Span::default())
        })?
        .into_pointer_value();
    // 若 type_desc 为 null：未实现任何接口。
    let desc_null = fl
        .builder
        .build_is_null(type_desc, "iim_desc_null")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_desc_null", scoop2_base::Span::default())
        })?;
    // 读 itable 容器（type_desc 第 13 个字段，偏移见 ScoopTypeDescriptor）。
    let td_ty = fl.cg.type_descriptor_type();
    let itable_field = unsafe {
        fl.builder
            .build_struct_gep(td_ty, type_desc, 12, "iim_itable_field")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "iim_gep_itable",
                    scoop2_base::Span::default(),
                )
            })?
    };
    let itable_container = fl
        .builder
        .build_load(native_ptr, itable_field, "iim_itable")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_load_itable",
                scoop2_base::Span::default(),
            )
        })?
        .into_pointer_value();
    // 若 itable 为 null：未实现任何接口。
    let itable_null = fl
        .builder
        .build_is_null(itable_container, "iim_itable_null")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_itable_null",
                scoop2_base::Span::default(),
            )
        })?;
    let any_null = fl
        .builder
        .build_or(desc_null, itable_null, "iim_any_null")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_any_null", scoop2_base::Span::default())
        })?;

    // 读 count（容器第 0 字段 i32）。
    let container_ty = fl.cg.itable_container_type_pub();
    let count_slot = unsafe {
        fl.builder
            .build_struct_gep(container_ty, itable_container, 0, "iim_count_slot")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "iim_gep_count", scoop2_base::Span::default())
            })?
    };
    let count = fl
        .builder
        .build_load(i32_ty, count_slot, "iim_count")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_load_count",
                scoop2_base::Span::default(),
            )
        })?
        .into_int_value();
    // entries 指针（容器第 2 字段）。
    let entries_slot = unsafe {
        fl.builder
            .build_struct_gep(container_ty, itable_container, 2, "iim_entries_slot")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "iim_gep_entries",
                    scoop2_base::Span::default(),
                )
            })?
    };
    let entries = fl
        .builder
        .build_load(native_ptr, entries_slot, "iim_entries")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_load_entries",
                scoop2_base::Span::default(),
            )
        })?
        .into_pointer_value();

    let entry_ty = fl.cg.itable_entry_type_pub();
    // 构建循环：
    //   entry_bb: if (!any_null && i < count) goto body_bb; else goto no_bb
    //   body_bb:  load entry.interface_id; if == target goto yes_bb; i++; goto entry_bb
    //   yes_bb/no_bb/merge_bb: 汇总结果。
    let entry_bb = fl.cg.context.append_basic_block(fl.fv, "iim_loop");
    let body_bb = fl.cg.context.append_basic_block(fl.fv, "iim_body");
    let yes_bb = fl.cg.context.append_basic_block(fl.fv, "iim_yes");
    let no_bb = fl.cg.context.append_basic_block(fl.fv, "iim_no");
    let idx_slot = fl.builder.build_alloca(i32_ty, "iim_idx").map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "iim_alloca_idx",
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder
        .build_store(idx_slot, i32_ty.const_zero())
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_store_idx0",
                scoop2_base::Span::default(),
            )
        })?;
    fl.builder
        .build_unconditional_branch(entry_bb)
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_br_loop", scoop2_base::Span::default())
        })?;

    fl.builder.position_at_end(entry_bb);
    let i = fl
        .builder
        .build_load(i32_ty, idx_slot, "iim_i")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_i", scoop2_base::Span::default()))?
        .into_int_value();
    let in_range = fl
        .builder
        .build_int_compare(inkwell::IntPredicate::ULT, i, count, "iim_in_range")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_in_range", scoop2_base::Span::default())
        })?;
    let not_null = fl
        .builder
        .build_not(any_null, "iim_not_null")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_not_null", scoop2_base::Span::default())
        })?;
    let cond = fl
        .builder
        .build_and(not_null, in_range, "iim_cond")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_cond", scoop2_base::Span::default()))?;
    fl.builder
        .build_conditional_branch(cond, body_bb, no_bb)
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_br_cond", scoop2_base::Span::default())
        })?;

    fl.builder.position_at_end(body_bb);
    // GEP 到 entries[i]。
    let entry_ptr = unsafe {
        fl.builder
            .build_in_bounds_gep(entry_ty, entries, &[i], "iim_entry")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "iim_gep_entry", scoop2_base::Span::default())
            })?
    };
    // entry.interface_id（第 0 字段）。
    let iface_id_slot = unsafe {
        fl.builder
            .build_struct_gep(entry_ty, entry_ptr, 0, "iim_iface_id_slot")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "iim_gep_iface_id",
                    scoop2_base::Span::default(),
                )
            })?
    };
    let iface_id = fl
        .builder
        .build_load(i64, iface_id_slot, "iim_iface_id")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_load_iface_id",
                scoop2_base::Span::default(),
            )
        })?
        .into_int_value();
    let eq = fl
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            iface_id,
            i64.const_int(target_iface_id, false),
            "iim_eq",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_eq", scoop2_base::Span::default()))?;
    // i++。
    let i_next = fl
        .builder
        .build_int_add(i, i32_ty.const_int(1, false), "iim_i_next")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_i_next", scoop2_base::Span::default())
        })?;
    fl.builder.build_store(idx_slot, i_next).map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "iim_store_i_next",
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder
        .build_conditional_branch(eq, yes_bb, entry_bb)
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "iim_br_eq", scoop2_base::Span::default())
        })?;

    fl.builder.position_at_end(yes_bb);
    let yes_val = i8.const_int(1, false);
    let yes_bb_end = yes_bb;
    fl.builder.position_at_end(no_bb);
    let no_val = i8.const_int(0, false);

    // 合并 yes/no 到一个结果（用 phi）。需要共同的 merge block。
    let merge_bb = fl.cg.context.append_basic_block(fl.fv, "iim_merge");
    fl.builder.position_at_end(yes_bb_end);
    fl.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_br_yes_merge",
                scoop2_base::Span::default(),
            )
        })?;
    fl.builder.position_at_end(no_bb);
    fl.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "iim_br_no_merge",
                scoop2_base::Span::default(),
            )
        })?;
    fl.builder.position_at_end(merge_bb);
    let phi = fl
        .builder
        .build_phi(i8, "iim_result")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_phi", scoop2_base::Span::default()))?;
    phi.add_incoming(&[(&yes_val, yes_bb_end), (&no_val, no_bb)]);
    Ok(phi.as_basic_value())
}

/// `Cast { value, target_ty }` → `as T` 类型转换。
///
/// 运行期类型检查：匹配则返回原值；不匹配按 failure 策略处理
/// （Panic → 调用 scoop_panic；ReturnNone → 返回 Option.None 表示）。
fn lower_cast<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &LirOperand,
    _target_ty: scoop2_hir::ty::TypeId,
    descriptor: &scoop2_mir::mir::transport::RuntimeTypeDescriptorKey,
    failure: &scoop2_mir::mir::transport::RuntimeCastFailure,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 先做 TypeTest：命中则原值返回；否则按 failure 处理。
    let test = lower_type_test(
        fl,
        value,
        _target_ty,
        scoop2_mir::mir::transport::RuntimeTypeStaticFold::Dynamic,
        descriptor,
    )?;
    // test 是 i8（Bool）；select/condbr 需 i1。truncate 到 i1（非 0 → true）。
    let test_i8 = test.into_int_value();
    let test_i1 = fl
        .builder
        .build_int_truncate(test_i8, fl.cg.context.bool_type(), "cast_test_i1")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "cast_trunc_test",
                scoop2_base::Span::default(),
            )
        })?;
    let val = match value {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    match failure {
        scoop2_mir::mir::transport::RuntimeCastFailure::Panic { message } => {
            // 失败路径：调用 scoop_panic。
            let ok_bb = fl.cg.context.append_basic_block(fl.fv, "cast_ok");
            let fail_bb = fl.cg.context.append_basic_block(fl.fv, "cast_fail");
            fl.builder
                .build_conditional_branch(test_i1, ok_bb, fail_bb)
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "cast_br", scoop2_base::Span::default())
                })?;
            fl.builder.position_at_end(fail_bb);
            // panic message：传递 null（runtime 降级处理）；message 文本保留供诊断。
            let _ = message;
            let native_null = fl.cg.native_ptr_ty().const_null().into();
            fl.builder
                .build_call(fl.rt.panic, &[native_null], "cast_panic")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "cast_panic", scoop2_base::Span::default())
                })?;
            fl.builder.build_unreachable().map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "cast_unreachable",
                    scoop2_base::Span::default(),
                )
            })?;
            fl.builder.position_at_end(ok_bb);
            Ok(val)
        }
        scoop2_mir::mir::transport::RuntimeCastFailure::ReturnNone => {
            // as? T：失败返回 None。Option 布局：null 指针 niche（None = null）。
            // select 需要操作数类型一致：None 用与 val 同类型的 null。
            let none_val: BasicValueEnum<'ctx> = match val {
                BasicValueEnum::PointerValue(_) => fl.cg.gc_ptr_ty().const_null().into(),
                BasicValueEnum::IntValue(_) => fl.cg.context.i64_type().const_zero().into(),
                _ => fl.cg.context.i64_type().const_zero().into(),
            };
            let result = fl
                .builder
                .build_select(test_i1, val, none_val, "cast_opt")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "cast_select", scoop2_base::Span::default())
                })?;
            Ok(result)
        }
    }
}

/// `PatternMatch { subject, pattern }` → 模式匹配测试（返回 Bool i8）。
///
/// 递归处理所有模式类型：
/// - Wildcard/Bind：恒真。
/// - IntLit/CharLit/BoolLit：标量相等比较。
/// - StringLit：scoop_string_equals 比较。
/// - Is { ty, negated }：运行期类型测试（type_id 比较），支持取反。
/// - Tuple/Struct/Variant：提取子值递归匹配，AND 合并。
/// - Or：任一子模式匹配即真。
fn lower_pattern_match<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    subject: &LirOperand,
    pattern: &LirPattern,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let i64 = fl.cg.context.i64_type();
    match pattern {
        LirPattern::Wildcard | LirPattern::Bind { .. } => Ok(i8.const_int(1, false).into()),
        LirPattern::IntLit(v) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let subj_i = super::expect_int_val(subj, "pattern subject", &fl.fqn)?;
            let rhs = i64.const_int(*v as u64, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj_i, rhs, "pat_int_eq")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "pat_int_eq", scoop2_base::Span::default())
                })?;
            Ok(fl
                .builder
                .build_int_z_extend(eq, i8, "pat_int_i8")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "zext pat_int", scoop2_base::Span::default())
                })?
                .into())
        }
        LirPattern::CharLit(c) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let subj_i = super::expect_int_val(subj, "pattern subject", &fl.fqn)?;
            // Char 为 i32。
            let rhs = fl.cg.context.i32_type().const_int(*c as u64, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj_i, rhs, "pat_char_eq")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "pat_char_eq", scoop2_base::Span::default())
                })?;
            Ok(fl
                .builder
                .build_int_z_extend(eq, i8, "pat_char_i8")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "zext pat_char", scoop2_base::Span::default())
                })?
                .into())
        }
        LirPattern::BoolLit(b) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let subj_i = super::expect_int_val(subj, "pattern subject", &fl.fqn)?;
            let rhs = i8.const_int(if *b { 1 } else { 0 }, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj_i, rhs, "pat_bool_eq")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "pat_bool_eq", scoop2_base::Span::default())
                })?;
            Ok(fl
                .builder
                .build_int_z_extend(eq, i8, "pat_bool_i8")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "zext pat_bool", scoop2_base::Span::default())
                })?
                .into())
        }
        LirPattern::StringLit(s) => {
            // subject 是 String 引用（GC ptr）。
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            // 字面量 String 全局。
            let lit_gc = fl.cg.get_or_create_string_literal(s)?;
            // scoop_string_equals 返回 i64（非 0 = 相等）。
            let eq_call = fl
                .builder
                .build_call(
                    fl.rt.string_equals,
                    &[subj.into(), lit_gc.into()],
                    "pat_str_eq",
                )
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "pat_str_eq", scoop2_base::Span::default())
                })?;
            let eq_i64 = match eq_call.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
                _ => {
                    return Err(CodegenError::llvm(
                        "string_equals 返回非 BasicValue",
                        "pat_str",
                        scoop2_base::Span::default(),
                    ));
                }
            };
            // 非 0 → true：eq_i64 != 0。
            let ne_zero = fl
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    eq_i64,
                    i64.const_zero(),
                    "pat_str_nez",
                )
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "pat_str_nez", scoop2_base::Span::default())
                })?;
            Ok(fl
                .builder
                .build_int_z_extend(ne_zero, i8, "pat_str_i8")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "zext pat_str", scoop2_base::Span::default())
                })?
                .into())
        }
        LirPattern::Is {
            ty: _,
            negated,
            target_fqn,
        } => {
            // 运行期类型测试：subject 必须是对象引用。
            let Some(fqn) = target_fqn else {
                // 无 FQN（非名义类型）：保守匹配。
                return Ok(i8.const_int(if *negated { 0 } else { 1 }, false).into());
            };
            let target_type_id = crate::globals::stable_hash_u64_pub(fqn);
            let val = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let is_match = type_id_equals(fl, val, target_type_id)?;
            let result = if *negated {
                // 取反：!is_match。
                let ne = fl
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        is_match,
                        i8.const_zero(),
                        "pat_is_neg",
                    )
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "pat_is_neg",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder
                    .build_int_z_extend(ne, i8, "pat_is_neg_i8")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "zext pat_is_neg",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into()
            } else {
                is_match.into()
            };
            Ok(result)
        }
        LirPattern::Or { patterns } => {
            // 任一匹配即真。短路求值：逐个测试，首个真即返回。
            let mut result = i8.const_int(0, false);
            for (i, sub) in patterns.iter().enumerate() {
                if i + 1 == patterns.len() {
                    // 最后一个：直接返回其结果（OR 短路）。
                    return lower_pattern_match(fl, subject, sub);
                }
                let r = lower_pattern_match(fl, subject, sub)?.into_int_value();
                // 若 r != 0，结果为 1 并短路。
                let is_true = fl
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        r,
                        i8.const_zero(),
                        &format!("pat_or_{}", i),
                    )
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), "pat_or", scoop2_base::Span::default())
                    })?;
                result = fl
                    .builder
                    .build_select(
                        is_true,
                        i8.const_int(1, false),
                        result,
                        &format!("pat_or_sel_{}", i),
                    )
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "pat_or_sel",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into_int_value();
            }
            Ok(result.into())
        }
        LirPattern::Variant {
            variant_name,
            tag_value,
            args,
        } => lower_variant_match(fl, subject, variant_name, *tag_value, args),
        LirPattern::Tuple { .. } | LirPattern::Struct { .. } => {
            // 聚合模式：需要字段提取。当前 PatternMatch 仅做「是否匹配」测试；
            // 绑定提取由 PatternExtract 单独处理。对聚合模式，仅检查结构存在性
            // （结构已由类型系统保证）。保守返回 true。
            // 完整的字段级子模式匹配需要 codegen 感知聚合布局，留作后续增强。
            Ok(i8.const_int(1, false).into())
        }
    }
}

/// `Pattern::Variant` 的真实匹配测试：提取 subject 的 tag / niche 编码并与
/// 变体判别值比较（返回 Bool i8）。
///
/// - nominal 值枚举（`TypeLayoutKind::Enum`）：extractvalue 取 tag 字段，
///   与判别值（LIR 按 enum_variants 声明序计算；缺失时按布局变体名查找）比较。
/// - 内建 Option（`TypeLayoutKind::Option`）：按 NicheStorage——Pointer 比 null、
///   U8 比 none_value、Tagged 比 tag 字节。
fn lower_variant_match<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    subject: &LirOperand,
    variant_name: &str,
    pattern_tag: Option<u64>,
    _args: &[LirPattern],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let subj_ty = match subject {
        LirOperand::Local(id) => fl.local_types.get(id).copied(),
        LirOperand::Const(_) => None,
    }
    .ok_or_else(|| {
        CodegenError::llvm(
            "Variant pattern: subject 类型未知",
            &fl.fqn,
            scoop2_base::Span::default(),
        )
    })?;
    let layout = fl.layouts.get(subj_ty).ok_or_else(|| {
        CodegenError::missing_layout(
            subj_ty.0,
            "Variant pattern subject",
            scoop2_base::Span::default(),
        )
    })?;
    let zext = |fl: &FunctionLowerer<'a, 'ctx>,
                v: inkwell::values::IntValue<'ctx>|
     -> CodegenResult<BasicValueEnum<'ctx>> {
        Ok(fl
            .builder
            .build_int_z_extend(v, i8, "pat_var_i8")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "zext pat_var", scoop2_base::Span::default())
            })?
            .into())
    };
    match &layout.kind {
        scoop2_lir::TypeLayoutKind::Enum { variants, .. } => {
            let expect = pattern_tag
                .or_else(|| {
                    variants
                        .iter()
                        .find(|v| v.name == variant_name)
                        .map(|v| v.tag_value)
                })
                .ok_or_else(|| {
                    CodegenError::llvm(
                        format!("Variant pattern: 变体 {} 的判别值未知", variant_name),
                        &fl.fqn,
                        scoop2_base::Span::default(),
                    )
                })?;
            let agg = load_struct_or_deref(fl, subject)?;
            let tag = fl
                .builder
                .build_extract_value(agg, 0, "pat_tag")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "extract pat_tag",
                        scoop2_base::Span::default(),
                    )
                })?;
            let tag_i = super::expect_int_val(tag, "enum tag", &fl.fqn)?;
            let rhs = tag_i.get_type().const_int(expect, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, tag_i, rhs, "pat_tag_eq")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "pat_tag_eq", scoop2_base::Span::default())
                })?;
            zext(fl, eq)
        }
        scoop2_lir::TypeLayoutKind::Option { storage, .. } => {
            // Some=0 / None=1（sysroot Option 声明序）；LIR 未给出时按名字回退。
            let expect = pattern_tag.unwrap_or(if variant_name == "None" { 1 } else { 0 });
            match storage {
                scoop2_lir::NicheStorage::Pointer => {
                    let v = match subject {
                        LirOperand::Local(id) => fl.load_local(*id)?,
                        LirOperand::Const(c) => fl.lower_const_value(c)?,
                    };
                    let p = super::expect_ptr_val(v, "Option(Pointer) subject", &fl.fqn)?;
                    let is_null = fl.builder.build_is_null(p, "pat_opt_null").map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "pat_opt_null",
                            scoop2_base::Span::default(),
                        )
                    })?;
                    let cond = if expect == 1 {
                        is_null
                    } else {
                        fl.builder
                            .build_not(is_null, "pat_opt_nonnull")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "pat_opt_nonnull",
                                    scoop2_base::Span::default(),
                                )
                            })?
                    };
                    zext(fl, cond)
                }
                scoop2_lir::NicheStorage::U8 { none_value } => {
                    let v = match subject {
                        LirOperand::Local(id) => fl.load_local(*id)?,
                        LirOperand::Const(c) => fl.lower_const_value(c)?,
                    };
                    let vi = super::expect_int_val(v, "Option(U8) subject", &fl.fqn)?;
                    let rhs = vi.get_type().const_int(*none_value as u64, false);
                    let is_none = fl
                        .builder
                        .build_int_compare(inkwell::IntPredicate::EQ, vi, rhs, "pat_opt_u8")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "pat_opt_u8",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    let cond = if expect == 1 {
                        is_none
                    } else {
                        fl.builder
                            .build_not(is_none, "pat_opt_u8_not")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "pat_opt_u8_not",
                                    scoop2_base::Span::default(),
                                )
                            })?
                    };
                    zext(fl, cond)
                }
                scoop2_lir::NicheStorage::Tagged => {
                    let agg = load_struct_or_deref(fl, subject)?;
                    let tag = fl
                        .builder
                        .build_extract_value(agg, 0, "pat_opt_tag")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "extract pat_opt_tag",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    let tag_i = super::expect_int_val(tag, "Option tag", &fl.fqn)?;
                    let rhs = tag_i.get_type().const_int(expect, false);
                    let eq = fl
                        .builder
                        .build_int_compare(inkwell::IntPredicate::EQ, tag_i, rhs, "pat_opt_tag_eq")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "pat_opt_tag_eq",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    zext(fl, eq)
                }
            }
        }
        _ => Err(CodegenError::unsupported(
            format!(
                "Variant pattern: subject 布局 {:?} 不是 enum/Option",
                layout.kind
            ),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 比较一个对象引用的实际 type_id 是否等于 `target_type_id`。
/// 返回 i8（1 = 匹配，0 = 不匹配）。
fn type_id_equals<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    target_type_id: u64,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let i64 = fl.cg.context.i64_type();
    let obj_ptr = match val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "tid_ptr2int")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "tid_ptr2int",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder
                    .build_int_to_ptr(as_int, fl.cg.native_ptr_ty(), "tid_native")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "tid_int2ptr",
                            scoop2_base::Span::default(),
                        )
                    })?
            } else {
                p
            }
        }
        _ => return Ok(i8.const_zero()),
    };
    let ptr_size = fl.cg.pointer_byte_size;
    let type_desc_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                fl.cg.context.i8_type(),
                obj_ptr,
                &[i64.const_int(ptr_size, false)],
                "tid_desc_slot",
            )
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "tid_gep_desc", scoop2_base::Span::default())
            })?
    };
    let type_desc_ptr = fl
        .builder
        .build_load(fl.cg.native_ptr_ty(), type_desc_slot, "tid_desc")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "tid_load_desc", scoop2_base::Span::default())
        })?
        .into_pointer_value();
    let type_id_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                fl.cg.context.i8_type(),
                type_desc_ptr,
                &[i64.const_int(64, false)],
                "tid_id_slot",
            )
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "tid_gep_id", scoop2_base::Span::default())
            })?
    };
    let actual = fl
        .builder
        .build_load(i64, type_id_slot, "tid_id")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "tid_load_id", scoop2_base::Span::default())
        })?
        .into_int_value();
    let eq = fl
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            actual,
            i64.const_int(target_type_id, false),
            "tid_eq",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "tid_eq", scoop2_base::Span::default()))?;
    Ok(fl
        .builder
        .build_int_z_extend(eq, i8, "tid_result")
        .map_err(|e| CodegenError::llvm(e.to_string(), "zext tid", scoop2_base::Span::default()))?)
}

/// `PatternExtract { subject, path, result_ty }` → 模式提取。
///
/// 对 Option subject 提取 payload（Some(x) 的 x）：
/// - Pointer / U8 niche：payload 就是 subject 值本身，原样返回。
/// - Tagged：`{ i8 tag; payload }` → extractvalue 取字段 1。
/// 对值枚举 subject（`{ iN tag; [payload_bytes x i8] }`）：按 `path` 定位——
/// `VariantField { variant, field_index }` 给出 variant 名与字段序号，偏移 =
/// 该 variant 的 slot_offset +（多字段时）payload_fields[field_index].offset；
/// 无 path 时退化为 scalar union slot 起点（EffectStep 的 Step enum 等，
/// 其 variant 共用 payload_offset 区）。与 lower_enum_variant 的写入对称。
/// 其他 subject（无 payload 的值枚举等）保持旧行为：返回 subject 本身。
fn lower_pattern_extract<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    subject: &LirOperand,
    path: &[scoop2_mir::mir::transport::PatternBindingStep],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let subj_ty = match subject {
        LirOperand::Local(id) => fl.local_types.get(id).copied(),
        LirOperand::Const(_) => None,
    };
    if let Some(ty) = subj_ty
        && let Some(layout) = fl.layouts.get(ty)
        && let scoop2_lir::TypeLayoutKind::Option { storage, .. } = &layout.kind
    {
        match storage {
            scoop2_lir::NicheStorage::Pointer | scoop2_lir::NicheStorage::U8 { .. } => {
                // payload = subject 本身。
                return match subject {
                    LirOperand::Local(id) => fl.load_local(*id),
                    LirOperand::Const(c) => fl.lower_const_value(c),
                };
            }
            scoop2_lir::NicheStorage::Tagged => {
                let agg = load_struct_or_deref(fl, subject)?;
                let v = fl
                    .builder
                    .build_extract_value(agg, 1, "opt_payload")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "extract opt_payload",
                            scoop2_base::Span::default(),
                        )
                    })?;
                return Ok(v);
            }
        }
    }
    if let Some(ty) = subj_ty
        && let Some(layout) = fl.layouts.get(ty)
        && matches!(&layout.kind, scoop2_lir::TypeLayoutKind::Enum { .. })
    {
        // 值枚举 payload 提取：内存 round-trip——subject 落 scratch alloca，
        // 按 payload 偏移以 result_ty 具类型 load。result size=0（Unit payload）
        // 时没有可读的 payload，退化为返回 subject（与旧行为一致）。
        let result_size = fl.layouts.get(result_ty).map(|l| l.size).unwrap_or(0);
        if result_size > 0 {
            // 提取偏移：优先按 path 的 VariantField 定位（variant slot_offset +
            // 多字段相对偏移）；无 path 信息时用 scalar union slot 起点。
            let offset = if let [scoop2_mir::mir::transport::PatternBindingStep::VariantField {
                variant,
                field_index,
            }] = path
            {
                match &layout.kind {
                    scoop2_lir::TypeLayoutKind::Enum { variants, .. } => variants
                        .iter()
                        .find(|v| v.name == *variant)
                        .map(|v| {
                            v.payload_fields
                                .get(*field_index)
                                .map(|f| v.slot_offset + f.offset)
                                .unwrap_or(v.slot_offset)
                        })
                        .unwrap_or_else(|| enum_payload_offset(layout, fl.layouts)),
                    _ => enum_payload_offset(layout, fl.layouts),
                }
            } else {
                enum_payload_offset(layout, fl.layouts)
            };
            let agg = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let enum_llvm_ty = fl.cg.lower_type(ty, fl.layouts)?;
            let scratch = fl
                .builder
                .build_alloca(enum_llvm_ty, "enum_extract_scratch")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "enum extract alloca",
                        scoop2_base::Span::default(),
                    )
                })?;
            fl.builder.build_store(scratch, agg).map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "enum extract store",
                    scoop2_base::Span::default(),
                )
            })?;
            let payload_ptr = unsafe {
                fl.builder.build_gep(
                    fl.cg.context.i8_type(),
                    scratch,
                    &[fl.cg.context.i64_type().const_int(offset, false)],
                    "enum_extract_ptr",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "enum extract gep",
                    scoop2_base::Span::default(),
                )
            })?;
            let result_llvm_ty = fl.cg.lower_type(result_ty, fl.layouts)?;
            let v = fl
                .builder
                .build_load(result_llvm_ty, payload_ptr, "enum_extract_payload")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "enum extract load",
                        scoop2_base::Span::default(),
                    )
                })?;
            return Ok(v);
        }
    }
    // tuple / struct 字段提取（path 首步 TupleIndex）：内存 round-trip——
    // subject 落 scratch alloca，按字段偏移以 result_ty 具类型 load（与 enum
    // payload 提取同法；聚合字段不能按首类值错位读取）。
    if let Some(ty) = subj_ty
        && let Some(layout) = fl.layouts.get(ty)
    {
        let field_offset = match (&layout.kind, path.first()) {
            (
                scoop2_lir::TypeLayoutKind::Tuple { elements },
                Some(scoop2_mir::mir::transport::PatternBindingStep::TupleIndex(i)),
            ) => elements.get(*i).map(|f| f.offset),
            (
                scoop2_lir::TypeLayoutKind::Struct { fields },
                Some(scoop2_mir::mir::transport::PatternBindingStep::TupleIndex(i)),
            ) => fields.get(*i).map(|f| f.offset),
            _ => None,
        };
        let result_size = fl.layouts.get(result_ty).map(|l| l.size).unwrap_or(0);
        if let Some(offset) = field_offset
            && result_size > 0
        {
            let agg = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let agg_llvm_ty = fl.cg.lower_type(ty, fl.layouts)?;
            let scratch = fl
                .builder
                .build_alloca(agg_llvm_ty, "field_extract_scratch")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "field extract alloca",
                        scoop2_base::Span::default(),
                    )
                })?;
            fl.builder.build_store(scratch, agg).map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "field extract store",
                    scoop2_base::Span::default(),
                )
            })?;
            let field_ptr = unsafe {
                fl.builder.build_gep(
                    fl.cg.context.i8_type(),
                    scratch,
                    &[fl.cg.context.i64_type().const_int(offset, false)],
                    "field_extract_ptr",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "field extract gep",
                    scoop2_base::Span::default(),
                )
            })?;
            let result_llvm_ty = fl.cg.lower_type(result_ty, fl.layouts)?;
            let v = fl
                .builder
                .build_load(result_llvm_ty, field_ptr, "field_extract_payload")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "field extract load",
                        scoop2_base::Span::default(),
                    )
                })?;
            return Ok(v);
        }
    }
    match subject {
        LirOperand::Local(id) => fl.load_local(*id),
        LirOperand::Const(c) => fl.lower_const_value(c),
    }
}

/// `InterpolatedString { parts }` → f-string 拼接。
fn lower_interpolated_string<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    parts: &[LirInterpolatedPart],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 简化：仅支持纯字面量部分拼接。
    // 完整实现需要将每个 expr 部分 toString 后 concat。
    let mut result: Option<inkwell::values::PointerValue<'ctx>> = None;
    for part in parts {
        match part {
            LirInterpolatedPart::Lit(s) => {
                let lit = fl.cg.get_or_create_string_literal(s)?;
                result = Some(match result {
                    Some(prev) => {
                        // concat(prev, lit) — string_concat 接受 GC ptr 参数。
                        let concat_result = fl
                            .builder
                            .build_call(fl.rt.string_concat, &[prev.into(), lit.into()], "concat")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "call concat",
                                    scoop2_base::Span::default(),
                                )
                            })?;
                        match concat_result.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
                            _ => lit,
                        }
                    }
                    None => lit,
                });
            }
            LirInterpolatedPart::Expr(operand) => {
                // expr 部分：转 String（按类型 dispatch toString intrinsic），然后 concat。
                let expr_str = lower_interp_expr_to_string(fl, operand)?;
                result = Some(match result {
                    Some(prev) => {
                        let concat_result = fl
                            .builder
                            .build_call(
                                fl.rt.string_concat,
                                &[prev.into(), expr_str.into()],
                                "concat_e",
                            )
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "call concat_e",
                                    scoop2_base::Span::default(),
                                )
                            })?;
                        match concat_result.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
                            _ => expr_str,
                        }
                    }
                    None => expr_str,
                });
            }
        }
    }
    Ok(result
        .unwrap_or_else(|| fl.cg.gc_ptr_ty().const_null())
        .into())
}

/// `WithUpdate { base, updates, result_ty }` → 值类型字段更新（copy + modify）。
/// f-string expr 部分 → String。
///
/// 按值类型 dispatch：
/// - String：原值返回（无需转换）。
/// - Int：scoop_int_to_string(i64) → String。
/// - Bool：scoop_bool_to_string(i8) → String。
/// - Char：scoop_char_to_string(i32) → String。
/// - Float：scoop_float64_to_string(f64) → String。
/// - 其它（对象）：调用对象的 toString 方法（接口分发；当前简化用 type_id 查 itable）。
fn lower_interp_expr_to_string<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    operand: &LirOperand,
) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
    let val = match operand {
        LirOperand::Local(id) => {
            let ty = fl.local_types.get(id).copied().ok_or_else(|| {
                CodegenError::unsupported(
                    "interp expr local type unknown",
                    &fl.fqn,
                    scoop2_base::Span::default(),
                )
            })?;
            let layout = fl.layouts.get(ty);
            let v = fl.load_local(*id)?;
            (v, layout.map(|l| l.kind.clone()))
        }
        LirOperand::Const(c) => {
            let v = fl.lower_const_value(c)?;
            // 常量无 local 类型可查，按常量种类合成 layout kind（否则落入 null 回退，
            // f-string 中的字面量（如 f"${1.5}"）会打印为空）。
            let kind = match c {
                scoop2_lir::LirConstValue::Bool(_) => Some(scoop2_lir::TypeLayoutKind::Scalar {
                    scalar_kind: scoop2_lir::ScalarKind::Bool,
                }),
                scoop2_lir::LirConstValue::Char(_) => Some(scoop2_lir::TypeLayoutKind::Scalar {
                    scalar_kind: scoop2_lir::ScalarKind::Char,
                }),
                scoop2_lir::LirConstValue::Int(_, suffix) => {
                    Some(scoop2_lir::TypeLayoutKind::Scalar {
                        scalar_kind: scoop2_lir::ScalarKind::Int {
                            bits: 64,
                            unsigned: matches!(
                                suffix,
                                Some(scoop2_lir::LirIntSuffix::U | scoop2_lir::LirIntSuffix::UL)
                            ),
                        },
                    })
                }
                scoop2_lir::LirConstValue::Float(_, suffix) => {
                    Some(scoop2_lir::TypeLayoutKind::Scalar {
                        scalar_kind: scoop2_lir::ScalarKind::Float {
                            bits: if matches!(suffix, Some(scoop2_lir::LirFloatSuffix::F32)) {
                                32
                            } else {
                                64
                            },
                        },
                    })
                }
                scoop2_lir::LirConstValue::String(_) => {
                    Some(scoop2_lir::TypeLayoutKind::Reference {
                        gc_traceable: true,
                        ref_kind: scoop2_lir::RefKind::String,
                    })
                }
                scoop2_lir::LirConstValue::Unit | scoop2_lir::LirConstValue::Null => None,
            };
            (v, kind)
        }
    };
    let (v, kind) = val;
    let gc_ptr_ty = fl.cg.gc_ptr_ty();
    let i64 = fl.cg.context.i64_type();
    // 按 layout kind dispatch。
    let str_val = match &kind {
        Some(scoop2_lir::TypeLayoutKind::Reference {
            gc_traceable: true, ..
        }) => {
            // 已是引用：若是 String 直接返回；否则需 toString（简化：假设 String）。
            super::expect_ptr_val(v, "interp expr 引用", &fl.fqn)?
        }
        Some(scoop2_lir::TypeLayoutKind::Scalar { scalar_kind }) => {
            use scoop2_lir::ScalarKind;
            match scalar_kind {
                ScalarKind::Int { .. } => {
                    let iv = crate::intrinsics::zext_to_i64(
                        fl,
                        super::expect_int_val(v, "interp expr 标量", &fl.fqn)?,
                    );
                    let call = fl
                        .builder
                        .build_call(fl.rt.int_to_string, &[iv.into()], "i2s")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "int_to_string",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    match call.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(b) => b.into_pointer_value(),
                        _ => gc_ptr_ty.const_null(),
                    }
                }
                ScalarKind::Bool => {
                    let iv = crate::intrinsics::zext_to_i64(
                        fl,
                        super::expect_int_val(v, "interp expr 标量", &fl.fqn)?,
                    );
                    let call = fl
                        .builder
                        .build_call(fl.rt.bool_to_string, &[iv.into()], "b2s")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "bool_to_string",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    match call.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(b) => b.into_pointer_value(),
                        _ => gc_ptr_ty.const_null(),
                    }
                }
                ScalarKind::Char => {
                    let iv = crate::intrinsics::zext_to_i64(
                        fl,
                        super::expect_int_val(v, "interp expr 标量", &fl.fqn)?,
                    );
                    let call = fl
                        .builder
                        .build_call(fl.rt.char_to_string, &[iv.into()], "c2s")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "char_to_string",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    match call.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(b) => b.into_pointer_value(),
                        _ => gc_ptr_ty.const_null(),
                    }
                }
                ScalarKind::Float { bits } => {
                    let fv = super::expect_float_val(v, "interp expr 浮点", &fl.fqn)?;
                    // 按位宽 dispatch：f32 → scoop_float32_to_string，f64 → scoop_float64_to_string。
                    let rt_fn = if *bits == 32 {
                        fl.rt.float32_to_string
                    } else {
                        fl.rt.float64_to_string
                    };
                    let call = fl
                        .builder
                        .build_call(rt_fn, &[fv.into()], "f2s")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "float_to_string",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    match call.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(b) => b.into_pointer_value(),
                        _ => gc_ptr_ty.const_null(),
                    }
                }
                _ => gc_ptr_ty.const_null(),
            }
        }
        _ => gc_ptr_ty.const_null(),
    };
    Ok(str_val)
}

fn lower_with_update<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    base: &LirOperand,
    updates: &[LirWithUpdateField],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // enum variant with 更新（`err with { Ok.point.x: 7 }`）：运行时 tag 与
    // 目标 variant 不匹配 → scoop_panic（exit 3）；匹配则按绝对偏移写字段。
    if updates.iter().any(|u| u.variant.is_some()) {
        return lower_enum_with_update(fl, base, updates, result_ty);
    }
    // 值语义：拷贝 base 聚合值，再按 LIR 解析好的字段路径逐条 insertvalue。
    let mut agg = match base {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    for update in updates {
        let val = match &update.value {
            LirOperand::Local(id) => fl.load_local(*id)?,
            LirOperand::Const(c) => fl.lower_const_value(c)?,
        };
        agg = apply_with_update_path(fl, agg, result_ty, &update.path, val)?;
    }
    Ok(agg)
}

/// enum `with` 更新：base 落 scratch alloca，逐条 update 做 tag 检查
/// （不匹配 → `scoop_panic`，exit 3）后按绝对偏移写字段，最后整体 reload。
///
/// 走内存 round-trip 而非 insertvalue：enum 布局是 `{ tag; [N x i8] }` 字节
/// 数组，payload 字段没有对应的 LLVM 结构字段下标。嵌套路径（`Ok.point.x`）
/// 的中间聚合不变，直接按末段绝对偏移写入即可。不切换 variant（ mismatch
/// 已 panic），各 ref 区内容保持原状，满足 GC trace 的零初始化纪律。
fn lower_enum_with_update<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    base: &LirOperand,
    updates: &[LirWithUpdateField],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let layout = fl.layouts.get(result_ty).ok_or_else(|| {
        CodegenError::missing_layout(result_ty.0, "WithUpdate enum", scoop2_base::Span::default())
    })?;
    let (tag_offset, tag_size) = match &layout.kind {
        scoop2_lir::TypeLayoutKind::Enum {
            tag_offset,
            tag_size,
            ..
        } => (*tag_offset, *tag_size),
        other => {
            return Err(CodegenError::unsupported(
                format!("enum with 更新的 receiver 布局不是 Enum：{:?}", other),
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    let enum_llvm_ty = fl.cg.lower_type(result_ty, fl.layouts)?;
    let agg = match base {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let scratch = fl
        .builder
        .build_alloca(enum_llvm_ty, "enum_wu_scratch")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "enum with alloca", scoop2_base::Span::default())
        })?;
    fl.builder.build_store(scratch, agg).map_err(|e| {
        CodegenError::llvm(e.to_string(), "enum with store", scoop2_base::Span::default())
    })?;
    let parent_fn = fl
        .builder
        .get_insert_block()
        .and_then(|b| b.get_parent())
        .ok_or_else(|| {
            CodegenError::llvm(
                "enum with update 不在函数体内".to_string(),
                &fl.fqn,
                scoop2_base::Span::default(),
            )
        })?;
    for update in updates {
        let Some(vt) = &update.variant else {
            return Err(CodegenError::unsupported(
                "enum with 更新混用了非 variant 路径",
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        };
        // tag 运行时检查：tag != variant.tag_value → scoop_panic(exit 3)。
        let tag_ptr = unsafe {
            fl.builder.build_gep(
                fl.cg.context.i8_type(),
                scratch,
                &[fl.cg.context.i64_type().const_int(tag_offset, false)],
                "enum_wu_tag_ptr",
            )
        }
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "enum with tag gep", scoop2_base::Span::default())
        })?;
        let tag_ty = fl
            .cg
            .context
            .custom_width_int_type((tag_size.max(1) * 8) as u32);
        let tag = fl
            .builder
            .build_load(tag_ty, tag_ptr, "enum_wu_tag")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "enum with tag load",
                    scoop2_base::Span::default(),
                )
            })?
            .into_int_value();
        let expect = tag_ty.const_int(vt.tag_value, false);
        let mismatch = fl
            .builder
            .build_int_compare(inkwell::IntPredicate::NE, tag, expect, "enum_wu_tag_ne")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "enum with tag cmp", scoop2_base::Span::default())
            })?;
        let panic_bb = fl.cg.context.append_basic_block(parent_fn, "enum_wu_panic");
        let ok_bb = fl.cg.context.append_basic_block(parent_fn, "enum_wu_ok");
        fl.builder
            .build_conditional_branch(mismatch, panic_bb, ok_bb)
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "enum with condbr",
                    scoop2_base::Span::default(),
                )
            })?;
        fl.builder.position_at_end(panic_bb);
        let msg = fl
            .cg
            .get_or_create_string_literal("enum with update: variant mismatch")?;
        let msg_int = fl
            .builder
            .build_ptr_to_int(msg, fl.cg.context.i64_type(), "enum_wu_msg_int")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "ptr_to_int enum with msg",
                    scoop2_base::Span::default(),
                )
            })?;
        let native = fl
            .builder
            .build_int_to_ptr(msg_int, fl.cg.native_ptr_ty(), "enum_wu_msg")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "int_to_ptr enum with msg",
                    scoop2_base::Span::default(),
                )
            })?;
        let _ = fl
            .builder
            .build_call(fl.rt.panic, &[native.into()], "enum_wu_panic")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "call enum with panic",
                    scoop2_base::Span::default(),
                )
            })?;
        fl.builder.build_unreachable().map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "unreachable enum with",
                scoop2_base::Span::default(),
            )
        })?;
        fl.builder.position_at_end(ok_bb);
        // 写字段：末段绝对偏移 + 新值（嵌套路径的中间聚合不变）。
        let last = update.path.last().ok_or_else(|| {
            CodegenError::llvm(
                format!("enum with 更新 `{}` 缺少字段路径", vt.name),
                &fl.fqn,
                scoop2_base::Span::default(),
            )
        })?;
        let val = match &update.value {
            LirOperand::Local(id) => fl.load_local(*id)?,
            LirOperand::Const(c) => fl.lower_const_value(c)?,
        };
        let field_ptr = unsafe {
            fl.builder.build_gep(
                fl.cg.context.i8_type(),
                scratch,
                &[fl.cg.context.i64_type().const_int(last.offset, false)],
                "enum_wu_field_ptr",
            )
        }
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "enum with field gep",
                scoop2_base::Span::default(),
            )
        })?;
        fl.builder.build_store(field_ptr, val).map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "enum with field store",
                scoop2_base::Span::default(),
            )
        })?;
    }
    let result = fl
        .builder
        .build_load(enum_llvm_ty, scratch, "enum_wu_result")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "enum with reload",
                scoop2_base::Span::default(),
            )
        })?;
    Ok(result)
}

/// 递归应用一条 with 更新路径：定位当前层字段（与 lower_member_access 同一套
/// (offset, ty) 定位逻辑），单段直接 insert，嵌套先 extract 内层聚合、递归更新后
/// 再插回。找不到字段或非 struct/tuple 布局报 CodegenError（绝不静默落到字段 0）。
fn apply_with_update_path<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    agg: BasicValueEnum<'ctx>,
    agg_ty: scoop2_hir::ty::TypeId,
    path: &[scoop2_lir::LirWithUpdateSegment],
    val: BasicValueEnum<'ctx>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let seg = path.first().ok_or_else(|| {
        CodegenError::llvm(
            "with 更新路径为空",
            "with_update",
            scoop2_base::Span::default(),
        )
    })?;
    let layout = fl.layouts.get(agg_ty).ok_or_else(|| {
        CodegenError::missing_layout(
            agg_ty.0,
            "WithUpdate receiver",
            scoop2_base::Span::default(),
        )
    })?;
    let fields = match &layout.kind {
        scoop2_lir::TypeLayoutKind::Struct { fields } => fields,
        scoop2_lir::TypeLayoutKind::Tuple { elements } => elements,
        other => {
            return Err(CodegenError::unsupported(
                format!("with 更新仅支持 struct/tuple 值类型，收到 {:?}", other),
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    let field_idx = fields
        .iter()
        .position(|f| f.offset == seg.offset && f.ty == seg.ty)
        .or_else(|| fields.iter().position(|f| f.offset == seg.offset))
        .ok_or_else(|| {
            CodegenError::llvm(
                format!(
                    "with 更新 {}: 布局中找不到 offset={} 的字段",
                    seg.name, seg.offset
                ),
                &fl.fqn,
                scoop2_base::Span::default(),
            )
        })?;
    let agg_s = super::expect_struct_val(agg, "WithUpdate base", &fl.fqn)?;
    let new_val = if path.len() == 1 {
        val
    } else {
        let inner = fl
            .builder
            .build_extract_value(agg_s, field_idx as u32, "wu_inner")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "extract with_update inner",
                    scoop2_base::Span::default(),
                )
            })?;
        apply_with_update_path(fl, inner, seg.ty, &path[1..], val)?
    };
    let inserted = fl
        .builder
        .build_insert_value(agg_s, new_val, field_idx as u32, "wu_set")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "insert_with_update",
                scoop2_base::Span::default(),
            )
        })?;
    Ok(inserted.into_struct_value().into())
}

/// `EnumVariant { enum_ty, tag_value, args, payload_ty }` → 构造 enum / Option 值。
///
/// 三种表示（与 lower_type 的布局 lowering 一一对应）：
/// - `TypeLayoutKind::Enum`（nominal 值枚举）：`{ iN tag; [payload_bytes x i8] }`，
///   tag 常量按布局 tag_size 的实际整型宽度构造（旧代码硬塞 i64 会触发
///   LLVM 验证错误）。
/// - `TypeLayoutKind::Option`（内建 Option<T>）：按 NicheStorage——
///   Pointer：Some(r) = r、None = null；U8：Some(v) = v、None = none_value；
///   Tagged：`{ i8 tag; payload }`。
fn lower_enum_variant<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    enum_ty: scoop2_hir::ty::TypeId,
    tag_value: u64,
    args: &[LirOperand],
    payload_ty: Option<scoop2_hir::ty::TypeId>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let layout = fl.layouts.get(enum_ty).ok_or_else(|| {
        CodegenError::missing_layout(enum_ty.0, "EnumVariant", scoop2_base::Span::default())
    })?;
    // 内建 Option<T>：按 niche 表示构造。
    if let scoop2_lir::TypeLayoutKind::Option {
        storage,
        payload_ty,
        ..
    } = &layout.kind
    {
        return lower_option_variant(fl, storage, *payload_ty, tag_value, args);
    }
    let enum_llvm_ty = fl.cg.lower_type(enum_ty, fl.layouts)?;
    let enum_struct = super::expect_struct_type(enum_llvm_ty, "EnumVariant 类型", &fl.fqn)?;
    let agg = enum_struct.const_zero();
    // field 0 = tag；tag 常量必须用字段的实际整型宽度（iN，N 来自布局 tag_size）。
    let tag_field_ty = enum_struct
        .get_field_type_at_index(0)
        .and_then(|t| match t {
            inkwell::types::BasicTypeEnum::IntType(i) => Some(i),
            _ => None,
        })
        .ok_or_else(|| {
            CodegenError::llvm(
                "EnumVariant: tag 字段不是整型",
                "enum_variant",
                scoop2_base::Span::default(),
            )
        })?;
    let tag_const = tag_field_ty.const_int(tag_value, false);
    let with_tag = fl
        .builder
        .build_insert_value(agg, tag_const, 0, "enum_tag")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "insert enum_tag",
                scoop2_base::Span::default(),
            )
        })?;
    // 关联数据 payload：值枚举表示为 `{ iN tag; [payload_bytes x i8] }` 扁平字节
    // blob，无法用 insertvalue 写入具类型 payload——经内存 round-trip：先把
    // with_tag 落进 scratch alloca，再按字节偏移以实际类型 store，最后整体
    // load 回 SSA 值。const_zero 聚合保证所有 ref slot（含非本 variant 的
    // 独立 ref 区）为零——零初始化纪律（NEW-LLVM-CODEGEN.md §3.1）。
    // 写入位置取自布局：多字段 variant 按 payload_fields 逐字段写（偏移 =
    // 本 variant slot_offset + 字段相对偏移）；单字段 variant 写 payload_ty
    // 于 slot_offset。size=0 的 payload（如 Unit）无需写入。
    // （EffectStep 的 Step enum 携带 op 实参 payload，其 variant 共用
    // payload_offset 区，见 NEW-LLVM-CODEGEN.md §3.1。）
    let mut writes: Vec<(u64, scoop2_hir::ty::TypeId, &LirOperand)> = Vec::new();
    let variant_layout = match &layout.kind {
        scoop2_lir::TypeLayoutKind::Enum { variants, .. } => {
            variants.iter().find(|v| v.tag_value == tag_value)
        }
        _ => None,
    };
    if let Some(vl) = variant_layout
        && !vl.payload_fields.is_empty()
        && vl.payload_fields.len() == args.len()
    {
        // 多字段 variant：逐字段写入本 variant 的 slot。
        for (f, a) in vl.payload_fields.iter().zip(args.iter()) {
            if f.size > 0 {
                writes.push((vl.slot_offset + f.offset, f.ty, a));
            }
        }
    } else if let (Some(payload_ty), Some(arg)) = (payload_ty, args.first()) {
        let payload_size = fl.layouts.get(payload_ty).map(|l| l.size).unwrap_or(0);
        if payload_size > 0 {
            let offset = variant_layout
                .map(|v| v.slot_offset)
                .unwrap_or_else(|| enum_payload_offset(layout, fl.layouts));
            writes.push((offset, payload_ty, arg));
        }
    }
    if !writes.is_empty() {
        let scratch = fl
            .builder
            .build_alloca(enum_struct, "enum_payload_scratch")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "enum scratch alloca", scoop2_base::Span::default())
            })?;
        fl.builder.build_store(scratch, with_tag).map_err(|e| {
            CodegenError::llvm(e.to_string(), "enum scratch store", scoop2_base::Span::default())
        })?;
        for (offset, field_ty, arg) in writes {
            let payload_val = fl.lower_operand(arg, field_ty)?;
            let payload_ptr = unsafe {
                fl.builder.build_gep(
                    fl.cg.context.i8_type(),
                    scratch,
                    &[fl.cg.context.i64_type().const_int(offset, false)],
                    "enum_payload_ptr",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "enum payload gep", scoop2_base::Span::default())
            })?;
            fl.builder.build_store(payload_ptr, payload_val).map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "enum payload store",
                    scoop2_base::Span::default(),
                )
            })?;
        }
        let loaded = fl
            .builder
            .build_load(enum_struct, scratch, "enum_with_payload")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "enum reload", scoop2_base::Span::default())
            })?;
        return Ok(loaded);
    }
    Ok(with_tag.into_struct_value().into())
}

/// enum 值（`{ iN tag; [payload_bytes x i8] }`）中 scalar union slot 的起始字节偏移。
/// 直接读布局预计算的 `payload_offset`（`align_to(tag_size, max_scalar_payload_align)`，
/// 见 `scoop2_lir::layout`）；含 ref 的 variant 另有独立 slot（EnumVariantLayout.slot_offset）。
pub(crate) fn enum_payload_offset(
    layout: &scoop2_lir::TypeLayout,
    _layouts: &scoop2_lir::TypeLayoutTable,
) -> u64 {
    match &layout.kind {
        scoop2_lir::TypeLayoutKind::Enum { payload_offset, .. } => *payload_offset,
        _ => layout.size,
    }
}

/// 构造内建 Option<T> 的 Some/None 值（按 niche 表示）。
/// `tag_value` = 变体判别值（Some=0 / None=1，LIR 按 enum_variants 声明序计算）。
fn lower_option_variant<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    storage: &scoop2_lir::NicheStorage,
    payload_ty: scoop2_hir::ty::TypeId,
    tag_value: u64,
    args: &[LirOperand],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match storage {
        scoop2_lir::NicheStorage::Pointer => {
            // Some(r) = r 本身；None = null。
            match args.first() {
                Some(arg) => fl.lower_operand(arg, payload_ty),
                None => Ok(fl.cg.gc_ptr_ty().const_null().into()),
            }
        }
        scoop2_lir::NicheStorage::U8 { none_value } => {
            // Some(v) = payload 值本身；None = none_value 编码。
            match args.first() {
                Some(arg) => fl.lower_operand(arg, payload_ty),
                None => {
                    let payload_llvm = fl.cg.lower_type(payload_ty, fl.layouts)?;
                    let int_ty = match payload_llvm {
                        inkwell::types::BasicTypeEnum::IntType(i) => i,
                        _ => fl.cg.context.i8_type(),
                    };
                    Ok(int_ty.const_int(*none_value as u64, false).into())
                }
            }
        }
        scoop2_lir::NicheStorage::Tagged => {
            // { i8 tag; payload }：None 的 payload 为 zero。
            let payload_llvm = fl.cg.lower_type(payload_ty, fl.layouts)?;
            let opt_ty = fl
                .cg
                .context
                .struct_type(&[fl.cg.context.i8_type().into(), payload_llvm], false);
            let mut agg = opt_ty.const_zero();
            agg = fl
                .builder
                .build_insert_value(
                    agg,
                    fl.cg.context.i8_type().const_int(tag_value, false),
                    0,
                    "opt_tag",
                )
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "insert opt_tag",
                        scoop2_base::Span::default(),
                    )
                })?
                .into_struct_value();
            if let Some(arg) = args.first() {
                let v = fl.lower_operand(arg, payload_ty)?;
                agg = fl
                    .builder
                    .build_insert_value(agg, v, 1, "opt_payload")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "insert opt_payload",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into_struct_value();
            }
            Ok(agg.into())
        }
    }
}

/// 把 Option 值从一种 niche 表示转换到另一种（Some/None 语义保持不变）。
///
/// 典型场景：`return None()` —— `None` 的静态类型是 `Option(Nothing)`
/// （Pointer niche，值为 null），而函数声明返回类型是 `Option<Int>`（Tagged）。
/// LIR 按各自类型 lowering，二者表示不同，返回/赋值处必须显式转换，
/// 否则函数签名与实际返回值不一致（LLVM 验证失败）。
pub fn coerce_option_value<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    from_ty: scoop2_hir::ty::TypeId,
    to_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let from_layout = fl.layouts.get(from_ty).cloned().ok_or_else(|| {
        CodegenError::missing_layout(
            from_ty.0,
            "coerce_option from",
            scoop2_base::Span::default(),
        )
    })?;
    let to_layout = fl.layouts.get(to_ty).cloned().ok_or_else(|| {
        CodegenError::missing_layout(to_ty.0, "coerce_option to", scoop2_base::Span::default())
    })?;
    let (from_storage, from_payload) = match &from_layout.kind {
        scoop2_lir::TypeLayoutKind::Option {
            storage,
            payload_ty,
            ..
        } => (storage.clone(), *payload_ty),
        _ => {
            return Err(CodegenError::llvm(
                format!("coerce_option: 源类型 {:?} 不是 Option", from_ty),
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    let (to_storage, to_payload) = match &to_layout.kind {
        scoop2_lir::TypeLayoutKind::Option {
            storage,
            payload_ty,
            ..
        } => (storage.clone(), *payload_ty),
        _ => {
            return Err(CodegenError::llvm(
                format!("coerce_option: 目标类型 {:?} 不是 Option", to_ty),
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };
    if from_storage == to_storage && from_payload == to_payload {
        return Ok(val);
    }
    let i8_ty = fl.cg.context.i8_type();
    // 1. 计算 is_none（i1）。
    let is_none = match &from_storage {
        scoop2_lir::NicheStorage::Pointer => {
            let p = super::expect_ptr_val(val, "coerce_option 源(Pointer)", &fl.fqn)?;
            fl.builder.build_is_null(p, "coerce_is_none").map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "coerce_is_none",
                    scoop2_base::Span::default(),
                )
            })?
        }
        scoop2_lir::NicheStorage::U8 { none_value } => {
            let vi = super::expect_int_val(val, "coerce_option 源(U8)", &fl.fqn)?;
            let rhs = vi.get_type().const_int(*none_value as u64, false);
            fl.builder
                .build_int_compare(inkwell::IntPredicate::EQ, vi, rhs, "coerce_is_none")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce_is_none u8",
                        scoop2_base::Span::default(),
                    )
                })?
        }
        scoop2_lir::NicheStorage::Tagged => {
            let agg = super::expect_struct_val(val, "coerce_option 源(Tagged)", &fl.fqn)?;
            let tag = fl
                .builder
                .build_extract_value(agg, 0, "coerce_tag")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce extract tag",
                        scoop2_base::Span::default(),
                    )
                })?;
            let tag_i = super::expect_int_val(tag, "coerce_option tag", &fl.fqn)?;
            fl.builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    tag_i,
                    tag_i.get_type().const_zero(),
                    "coerce_is_none",
                )
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce_is_none tagged",
                        scoop2_base::Span::default(),
                    )
                })?
        }
    };
    // 2. 提取源 payload（仅 Some 时有意义；类型不符时表示源不可能是 Some，用零值占位）。
    let src_payload: Option<BasicValueEnum<'ctx>> = match &from_storage {
        scoop2_lir::NicheStorage::Pointer | scoop2_lir::NicheStorage::U8 { .. } => Some(val),
        scoop2_lir::NicheStorage::Tagged => {
            let agg = super::expect_struct_val(val, "coerce_option 源(Tagged) payload", &fl.fqn)?;
            Some(
                fl.builder
                    .build_extract_value(agg, 1, "coerce_payload")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "coerce extract payload",
                            scoop2_base::Span::default(),
                        )
                    })?,
            )
        }
    };
    // 3. 构建目标表示的 none / some，然后 select。
    match &to_storage {
        scoop2_lir::NicheStorage::Pointer => {
            let none = fl.cg.gc_ptr_ty().const_null();
            let some = match src_payload {
                Some(BasicValueEnum::PointerValue(p)) => p,
                _ => none,
            };
            Ok(fl
                .builder
                .build_select(is_none, none, some, "coerce_opt")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce select ptr",
                        scoop2_base::Span::default(),
                    )
                })?)
        }
        scoop2_lir::NicheStorage::U8 { none_value } => {
            let payload_llvm = fl.cg.lower_type(to_payload, fl.layouts)?;
            let int_ty = match payload_llvm {
                inkwell::types::BasicTypeEnum::IntType(i) => i,
                _ => i8_ty,
            };
            let none = int_ty.const_int(*none_value as u64, false);
            let some = match src_payload {
                Some(BasicValueEnum::IntValue(i)) if i.get_type() == int_ty => i,
                _ => none,
            };
            Ok(fl
                .builder
                .build_select(is_none, none, some, "coerce_opt")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce select u8",
                        scoop2_base::Span::default(),
                    )
                })?)
        }
        scoop2_lir::NicheStorage::Tagged => {
            let payload_llvm = fl.cg.lower_type(to_payload, fl.layouts)?;
            let opt_ty = fl
                .cg
                .context
                .struct_type(&[i8_ty.into(), payload_llvm], false);
            // none = { tag=1, payload=zero }。
            let none = opt_ty.const_zero();
            let none = fl
                .builder
                .build_insert_value(none, i8_ty.const_int(1, false), 0, "coerce_none_tag")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce none_tag",
                        scoop2_base::Span::default(),
                    )
                })?
                .into_struct_value();
            // some = { tag=0, payload=源 payload（类型一致时） }。
            let some = opt_ty.const_zero();
            let some = match src_payload {
                Some(p) if p.get_type() == payload_llvm => fl
                    .builder
                    .build_insert_value(some, p, 1, "coerce_some_payload")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "coerce some_payload",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into_struct_value(),
                _ => some,
            };
            Ok(fl
                .builder
                .build_select(is_none, none, some, "coerce_opt")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "coerce select tagged",
                        scoop2_base::Span::default(),
                    )
                })?
                .into())
        }
    }
}

/// 对齐向上取整：返回 >= val 的最小 align 倍数（align > 0）。
fn align_up(val: u64, align: u64) -> u64 {
    if align == 0 {
        return val;
    }
    let rem = val % align;
    if rem == 0 { val } else { val + (align - rem) }
}

/// `ClassCtor { class_fqn, args }` → 分配 GC 对象 + 初始化字段。
///
/// 字段布局从 class_inits 的 field_inits 推导：每个字段按其 LLVM 类型的 store size
/// 排列，带对齐填充。这与 type descriptor 的 size_bytes/trace_bitmap 计算对齐
/// （都基于 ptr-sized slot 打包——当前简化：每个字段取 max(field_size, ptr_size)，
/// 按 ptr_size 对齐，保证 MemberAccess 的偏移计算一致）。
fn lower_class_ctor<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    class_fqn: &str,
    args: &[LirOperand],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 1. 获取 type descriptor。
    let type_desc = fl.cg.get_or_create_type_descriptor(class_fqn);
    // 2. 构造对象布局：{ header; field0; field1; ... }。
    let header_ty = fl.cg.object_header_type();
    let header_size = fl.cg.target_data.get_store_size(&header_ty);
    let ptr_size = fl.cg.pointer_byte_size;
    // 从 class_inits 取字段类型，计算每个字段的 LLVM size（回退到 ptr_size）。
    let class_init = fl
        .cg
        .class_inits
        .iter()
        .find(|ci| ci.class_fqn == class_fqn);
    let field_layouts: Vec<(u64, inkwell::types::BasicTypeEnum<'ctx>)> = args
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let field_ty = class_init
                .and_then(|ci| ci.field_inits.get(i))
                .map(|fi| fi.ty);
            let llvm_ty = field_ty
                .and_then(|ty| fl.cg.lower_type(ty, fl.layouts).ok())
                .unwrap_or_else(|| fl.cg.native_ptr_ty().into());
            let store_size = fl.cg.target_data.get_store_size(&llvm_ty).max(ptr_size);
            (store_size, llvm_ty)
        })
        .collect();
    // 计算每个字段的偏移（累加 size，按 ptr_size 对齐）。
    let mut offsets: Vec<u64> = Vec::with_capacity(field_layouts.len());
    let mut cur = header_size;
    for &(size, _) in &field_layouts {
        cur = align_up(cur, ptr_size);
        offsets.push(cur);
        cur += size;
    }
    let total_size = align_up(cur, ptr_size).max(header_size + ptr_size);
    // 3. scoop_alloc_typed(type_desc, size)。
    let alloc_result = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[
                type_desc.into(),
                fl.cg.context.i64_type().const_int(total_size, false).into(),
            ],
            "ctor_alloc",
        )
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "alloc_typed ctor",
                scoop2_base::Span::default(),
            )
        })?;
    let obj_native = match alloc_result.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => {
            super::expect_ptr_val(v, "alloc_typed 返回值", &fl.fqn)?
        }
        _ => {
            return Err(CodegenError::llvm(
                "alloc_typed 返回非 BasicValue",
                "class_ctor",
                scoop2_base::Span::default(),
            ));
        }
    };
    // 4. memset payload 为 0（header 已由 runtime 初始化）。
    // 简化：跳过 memset，直接写字段。
    // 5. 写入字段（按计算偏移 + 字段 LLVM 类型）。
    for (i, arg) in args.iter().enumerate() {
        let field_offset = offsets[i];
        let (_, field_llvm_ty) = field_layouts[i];
        let field_slot = unsafe {
            fl.builder.build_in_bounds_gep(
                fl.cg.context.i8_type(),
                obj_native,
                &[fl.cg.context.i64_type().const_int(field_offset, false)],
                &format!("ctor_field{}", i),
            )
        }
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "gep ctor_field",
                scoop2_base::Span::default(),
            )
        })?;
        let val = match arg {
            LirOperand::Local(id) => fl.load_local(*id)?,
            LirOperand::Const(c) => fl.lower_const_value(c)?,
        };
        // 字段值存储：按字段 LLVM 类型 store 到 field_slot。
        // - 指针：GC ptr → native ptr 后 store。
        // - 标量（Int/Bool/Char/Float）：直接 store（bit pattern）。
        // - 聚合（Struct/Tuple）：按聚合 LLVM 类型 store（field_slot 类型需匹配）。
        match val {
            BasicValueEnum::PointerValue(p) => {
                let pi = fl
                    .builder
                    .build_ptr_to_int(p, fl.cg.context.i64_type(), "arg_int")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "ptr_to_int ctor_arg",
                            scoop2_base::Span::default(),
                        )
                    })?;
                let native = fl
                    .builder
                    .build_int_to_ptr(pi, fl.cg.native_ptr_ty(), "arg_native")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "int_to_ptr ctor_arg",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder.build_store(field_slot, native).map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "store ctor_ptr_field",
                        scoop2_base::Span::default(),
                    )
                })?;
            }
            BasicValueEnum::IntValue(iv) => {
                // 标量直接 store（field_slot 类型需匹配——当前是 i8* GEP，store int 需 cast）。
                // 简化：转 i64 后按 ptr bit pattern store（与原有逻辑一致）。
                let iv64 = if iv.get_type().get_bit_width() == 64 {
                    iv
                } else {
                    fl.builder
                        .build_int_z_extend(iv, fl.cg.context.i64_type(), "arg_ext")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "zext ctor_arg",
                                scoop2_base::Span::default(),
                            )
                        })?
                };
                let as_ptr = fl
                    .builder
                    .build_int_to_ptr(iv64, fl.cg.native_ptr_ty(), "arg_int_as_ptr")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "int_to_ptr ctor_int",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder.build_store(field_slot, as_ptr).map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "store ctor_int_field",
                        scoop2_base::Span::default(),
                    )
                })?;
            }
            BasicValueEnum::StructValue(sv) => {
                // 聚合字段：按**字节偏移**定位（与读取路径 MemberAccess 一致：
                // i8 GEP 到 field_offset，再 bitcast 成聚合指针类型 store）。
                // 错误做法是用 `gep struct_ty, base, [offset/ptr_size]`——那把
                // 字节偏移当成「结构体元素个数」，写入地址 = base + idx*sizeof(struct)，
                // 与读取的 base+field_offset 不一致，导致字段读写错位。
                let struct_ty = sv.get_type();
                let byte_slot = unsafe {
                    fl.builder.build_in_bounds_gep(
                        fl.cg.context.i8_type(),
                        obj_native,
                        &[fl.cg
                            .context
                            .i64_type()
                            .const_int(field_offset, false)],
                        &format!("ctor_field{}_bytes", i),
                    )
                }
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "gep ctor_struct_field",
                        scoop2_base::Span::default(),
                    )
                })?;
                let typed_slot = fl
                    .builder
                    .build_pointer_cast(byte_slot, struct_ty.ptr_type(AddressSpace::from(0)), &format!("ctor_field{}_s", i))
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "pointer_cast ctor_struct_field",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder.build_store(typed_slot, sv).map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "store ctor_struct_field",
                        scoop2_base::Span::default(),
                    )
                })?;
            }
            other => {
                return Err(CodegenError::llvm(
                    &format!("unsupported ctor field value type: {:?}", other),
                    "class_ctor field",
                    scoop2_base::Span::default(),
                ));
            }
        }
    }
    // 6. 返回 GC ptr（native → addrspace 1）。
    let obj_int = fl
        .builder
        .build_ptr_to_int(obj_native, fl.cg.context.i64_type(), "obj_int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int obj",
                scoop2_base::Span::default(),
            )
        })?;
    let obj_gc = fl
        .builder
        .build_int_to_ptr(obj_int, fl.cg.gc_ptr_ty(), "obj_gc")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr obj",
                scoop2_base::Span::default(),
            )
        })?;
    Ok(obj_gc.into())
}

/// `MakeArray { elements, ty }` → 构造不可变数组。
///
/// 元素种类由元素类型决定：GC 引用 → REF (push_ref)，标量 → WORD (push_word)。
fn lower_make_array<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    elements: &[LirOperand],
    ty: scoop2_hir::ty::TypeId,
    mutable: bool,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let gc_ptr_ty = fl.cg.gc_ptr_ty();
    let native_ptr = fl.cg.native_ptr_ty();
    // 推断元素是否为 GC 引用：从首个元素的本地类型判断（数组类型 ty 本身总是
    // Reference，无法区分元素种类）。空数组默认按 WORD（标量）处理。
    let elem_is_ref = elements.first().is_some_and(|e| match e {
        LirOperand::Local(id) => fl
            .local_types
            .get(id)
            .and_then(|et| fl.layouts.get(*et))
            .map(|l| {
                matches!(
                    &l.kind,
                    // Function 值（闭包）也是 GC 引用：{ header; env; fn_ptr } 堆对象。
                    scoop2_lir::TypeLayoutKind::Reference {
                        gc_traceable: true,
                        ..
                    } | scoop2_lir::TypeLayoutKind::Function
                )
            })
            .unwrap_or(false),
        LirOperand::Const(c) => matches!(
            c,
            scoop2_lir::LirConstValue::String(_) | scoop2_lir::LirConstValue::Null
        ),
    });
    let _ = ty;
    let (elem_kind, push_fn) = if elem_is_ref {
        (2u64, fl.rt.mutable_array_push_ref)
    } else {
        (1u64, fl.rt.mutable_array_push_word)
    };
    // scoop_mutable_array_new(elem_kind, elem_size=8, elem_align=8, desc=null, capacity)
    let arr = fl
        .builder
        .build_call(
            fl.rt.mutable_array_new,
            &[
                fl.cg.context.i32_type().const_int(elem_kind, false).into(),
                fl.cg.context.i64_type().const_int(8, false).into(), // elem_size
                fl.cg.context.i64_type().const_int(8, false).into(), // elem_align
                native_ptr.const_null().into(),                      // desc
                fl.cg
                    .context
                    .i64_type()
                    .const_int(elements.len() as u64, false)
                    .into(), // capacity
            ],
            "arr_new",
        )
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "mutable_array_new",
                scoop2_base::Span::default(),
            )
        })?;
    let mut_arr = match arr.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => {
            super::expect_ptr_val(v, "array_new 返回值", &fl.fqn)?
        }
        _ => {
            return Err(CodegenError::llvm(
                "array_new 返回非 BasicValue",
                "make_array",
                scoop2_base::Span::default(),
            ));
        }
    };
    // push each element。
    for elem in elements {
        let val = match elem {
            LirOperand::Local(id) => fl.load_local(*id)?,
            LirOperand::Const(c) => fl.lower_const_value(c)?,
        };
        if elem_is_ref {
            // 引用元素：GC ptr → native ptr → push_ref(arr, native)。
            let val_native = match val {
                BasicValueEnum::PointerValue(p) => {
                    if p.get_type().get_address_space() == crate::context::gc_address_space() {
                        let pi = fl
                            .builder
                            .build_ptr_to_int(p, fl.cg.context.i64_type(), "elem_int")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "ptr_to_int elem",
                                    scoop2_base::Span::default(),
                                )
                            })?;
                        fl.builder
                            .build_int_to_ptr(pi, native_ptr, "elem_native")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "int_to_ptr elem",
                                    scoop2_base::Span::default(),
                                )
                            })?
                    } else {
                        p
                    }
                }
                _ => {
                    return Err(CodegenError::llvm(
                        "REF array element expected pointer",
                        "make_array",
                        scoop2_base::Span::default(),
                    ));
                }
            };
            let _ = fl
                .builder
                .build_call(push_fn, &[mut_arr.into(), val_native.into()], "arr_push")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "push_ref", scoop2_base::Span::default())
                })?;
        } else {
            // 标量元素：转 i64 字（整型 zext / 浮点按位模式 bitcast）→ push_word(arr, i64)。
            let val_i64 = match val {
                BasicValueEnum::IntValue(i) => {
                    let bits = i.get_type().get_bit_width();
                    if bits == 64 {
                        i
                    } else {
                        fl.builder
                            .build_int_z_extend(i, fl.cg.context.i64_type(), "elem_ext")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "zext elem",
                                    scoop2_base::Span::default(),
                                )
                            })?
                    }
                }
                BasicValueEnum::FloatValue(f) => {
                    // 浮点按位模式存入 8 字节槽：f64 bitcast 到 i64；
                    // f32 先 bitcast 到 i32 再 zext（与 IndexAccess 读取对称）。
                    let fty = f.get_type();
                    if fty == fl.cg.context.f64_type() {
                        fl.builder
                            .build_bit_cast(f, fl.cg.context.i64_type(), "elem_fbits")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "bitcast f64 elem",
                                    scoop2_base::Span::default(),
                                )
                            })?
                            .into_int_value()
                    } else {
                        let as_i32 = fl
                            .builder
                            .build_bit_cast(f, fl.cg.context.i32_type(), "elem_fbits")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "bitcast f32 elem",
                                    scoop2_base::Span::default(),
                                )
                            })?
                            .into_int_value();
                        fl.builder
                            .build_int_z_extend(as_i32, fl.cg.context.i64_type(), "elem_ext")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "zext f32 elem",
                                    scoop2_base::Span::default(),
                                )
                            })?
                    }
                }
                _ => {
                    return Err(CodegenError::llvm(
                        "WORD array element expected int/float",
                        "make_array",
                        scoop2_base::Span::default(),
                    ));
                }
            };
            let _ = fl
                .builder
                .build_call(push_fn, &[mut_arr.into(), val_i64.into()], "arr_push")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "push_word", scoop2_base::Span::default())
                })?;
        }
    }
    // freeze → immutable Array（期望类型为 MutableArray<T> 时保留可变数组本体）。
    if mutable {
        let arr_int = fl
            .builder
            .build_ptr_to_int(mut_arr, fl.cg.context.i64_type(), "mut_arr_int")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "ptr_to_int arr",
                    scoop2_base::Span::default(),
                )
            })?;
        let arr_gc = fl
            .builder
            .build_int_to_ptr(arr_int, fl.cg.gc_ptr_ty(), "mut_arr_gc")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "int_to_ptr arr",
                    scoop2_base::Span::default(),
                )
            })?;
        return Ok(arr_gc.into());
    }
    let frozen = fl
        .builder
        .build_call(fl.rt.mutable_array_freeze, &[mut_arr.into()], "arr_freeze")
        .map_err(|e| CodegenError::llvm(e.to_string(), "freeze", scoop2_base::Span::default()))?;
    Ok(match frozen.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v,
        _ => gc_ptr_ty.const_null().into(),
    })
}

/// `MakeClosure { env_local, invoke_fqn }` → 构造闭包对象。
fn lower_make_closure<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    env_local: &LirOperand,
    invoke_fqn: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 闭包对象布局：{ header; env_ptr; invoke_fn_ptr }。
    let header_ty = fl.cg.object_header_type();
    let header_size = fl.cg.target_data.get_store_size(&header_ty);
    let total_size = header_size + 2 * fl.cg.pointer_byte_size;
    // 分配。
    let type_desc = fl.cg.get_or_create_closure_type_descriptor();
    let alloc = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[
                type_desc.into(),
                fl.cg.context.i64_type().const_int(total_size, false).into(),
            ],
            "closure_alloc",
        )
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "alloc closure", scoop2_base::Span::default())
        })?;
    let obj_native = match alloc.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => {
            super::expect_ptr_val(v, "alloc closure 返回值", &fl.fqn)?
        }
        _ => {
            return Err(CodegenError::llvm(
                "alloc closure 返回非 BasicValue",
                "make_closure",
                scoop2_base::Span::default(),
            ));
        }
    };
    // env_ptr at offset header_size。
    //
    // 构造窗口保护：env 计算（pack_closure_env）内部有 GC 分配，而此刻
    // 闭包对象尚未链接进任何 root（本 rvalue 的结果还没写入目标 local 的
    // root frame slot）。若 GC 在该分配点运行，闭包对象会被误判为不可达
    // 而回收。因此先把 env slot 清零（避免 GC trace 到未初始化的垃圾
    // word），再 pin 住闭包对象；env 写入后 unpin。
    let env_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_native,
            &[fl.cg.context.i64_type().const_int(header_size, false)],
            "closure_env_slot",
        )
    }
    .map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "gep closure_env",
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder
        .build_store(env_slot, fl.cg.native_ptr_ty().const_null())
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "store closure_env null",
                scoop2_base::Span::default(),
            )
        })?;
    let obj_pin_int = fl
        .builder
        .build_ptr_to_int(obj_native, fl.cg.context.i64_type(), "closure_pin_int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int closure",
                scoop2_base::Span::default(),
            )
        })?;
    let obj_pin_arg = fl
        .builder
        .build_int_to_ptr(obj_pin_int, fl.cg.native_ptr_ty(), "closure_pin_arg")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr closure",
                scoop2_base::Span::default(),
            )
        })?;
    fl.builder
        .build_call(fl.rt.pin, &[obj_pin_arg.into()], "pin_closure")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "pin closure", scoop2_base::Span::default())
        })?;
    let env = match env_local {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let env_ty = match env_local {
        LirOperand::Local(id) => fl.local_types.get(id).copied(),
        LirOperand::Const(_) => None,
    };
    let env_native = match (env_ty, env) {
        // env 是 tuple struct 值（常规路径：MakeTuple 产物）→ 打包进堆 blob，
        // 与统一 ABI（invoke 首参 $env = blob 指针）配套；解包见 unpack_closure_env。
        (Some(ety), BasicValueEnum::StructValue(sv)) => pack_closure_env(fl, ety, sv)?,
        (_, BasicValueEnum::PointerValue(p)) => {
            let pi = fl
                .builder
                .build_ptr_to_int(p, fl.cg.context.i64_type(), "env_int")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "ptr_to_int env",
                        scoop2_base::Span::default(),
                    )
                })?;
            fl.builder
                .build_int_to_ptr(pi, fl.cg.native_ptr_ty(), "env_native")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "int_to_ptr env",
                        scoop2_base::Span::default(),
                    )
                })?
        }
        _ => fl.cg.native_ptr_ty().const_null(),
    };
    fl.builder.build_store(env_slot, env_native).map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "store closure_env",
            scoop2_base::Span::default(),
        )
    })?;
    fl.builder
        .build_call(fl.rt.unpin, &[obj_pin_arg.into()], "unpin_closure")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "unpin closure", scoop2_base::Span::default())
        })?;
    // invoke_fn_ptr at offset header_size + ptr_size。
    let fn_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_native,
            &[fl.cg
                .context
                .i64_type()
                .const_int(header_size + fl.cg.pointer_byte_size, false)],
            "closure_fn_slot",
        )
    }
    .map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            "gep closure_fn",
            scoop2_base::Span::default(),
        )
    })?;
    // 查找 invoke 函数。
    if let Some(invoke_fv) = fl
        .cg
        .lookup_callable_fn(invoke_fqn)
        .or_else(|| fl.cg.module.get_function(invoke_fqn))
    {
        let invoke_ptr = unsafe { inkwell::values::PointerValue::new(invoke_fv.as_value_ref()) };
        fl.builder.build_store(fn_slot, invoke_ptr).map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "store closure_fn",
                scoop2_base::Span::default(),
            )
        })?;
    } else {
        fl.builder
            .build_store(fn_slot, fl.cg.native_ptr_ty().const_null())
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "store closure_fn null",
                    scoop2_base::Span::default(),
                )
            })?;
    }
    // native → GC ptr。
    let obj_int = fl
        .builder
        .build_ptr_to_int(obj_native, fl.cg.context.i64_type(), "closure_int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int closure",
                scoop2_base::Span::default(),
            )
        })?;
    let obj_gc = fl
        .builder
        .build_int_to_ptr(obj_int, fl.cg.gc_ptr_ty(), "closure_gc")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr closure",
                scoop2_base::Span::default(),
            )
        })?;
    Ok(obj_gc.into())
}

/// 把 env tuple struct 值打包进堆 blob（GC alloc），返回 blob 的 native 指针。
/// blob 布局：object header 之后按 tuple 布局的字段 offset 依次存放字段值
///（与函数入口的 unpack_closure_env 对称）。
fn pack_closure_env<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    env_ty: scoop2_hir::ty::TypeId,
    env: inkwell::values::StructValue<'ctx>,
) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
    let (size, fields) = match fl.layouts.get(env_ty) {
        Some(layout) => {
            let fields = match &layout.kind {
                scoop2_lir::TypeLayoutKind::Struct { fields }
                | scoop2_lir::TypeLayoutKind::Tuple { elements: fields } => fields.clone(),
                _ => Vec::new(),
            };
            (layout.size.max(1), fields)
        }
        None => (1, Vec::new()),
    };
    // alloc_typed 的 size 含 object header；字段写在 header 之后。
    let header_size = fl
        .cg
        .target_data
        .get_store_size(&fl.cg.object_header_type());
    let total_size = header_size + size;
    let type_desc = fl.cg.get_or_create_env_blob_type_descriptor(env_ty, size);
    let alloc = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[
                type_desc.into(),
                fl.cg.context.i64_type().const_int(total_size, false).into(),
            ],
            "env_alloc",
        )
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "alloc env", scoop2_base::Span::default())
        })?;
    let blob = match alloc.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => {
            super::expect_ptr_val(v, "alloc env 返回值", &fl.fqn)?
        }
        _ => {
            return Err(CodegenError::llvm(
                "alloc env 返回非 BasicValue",
                "make_closure",
                scoop2_base::Span::default(),
            ));
        }
    };
    for (i, f) in fields.iter().enumerate() {
        let field_val = fl
            .builder
            .build_extract_value(env, i as u32, "env_pack")
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "extractvalue env",
                    scoop2_base::Span::default(),
                )
            })?;
        let slot = unsafe {
            fl.builder.build_in_bounds_gep(
                fl.cg.context.i8_type(),
                blob,
                &[fl.cg
                    .context
                    .i64_type()
                    .const_int(header_size + f.offset, false)],
                "env_slot_i8",
            )
        }
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "gep env slot", scoop2_base::Span::default())
        })?;
        fl.builder.build_store(slot, field_val).map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "store env field",
                scoop2_base::Span::default(),
            )
        })?;
    }
    Ok(blob)
}

/// `ClassLit { type_fqn }` → `T::class`（返回 type_desc 地址）。
fn lower_class_lit<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    type_fqn: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let type_desc = fl.cg.get_or_create_type_descriptor(type_fqn);
    // 返回 type_desc 地址作为 UIntPtr (i64)。
    let addr = fl
        .builder
        .build_ptr_to_int(type_desc, fl.cg.context.i64_type(), "class_lit_addr")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int class_lit",
                scoop2_base::Span::default(),
            )
        })?;
    Ok(addr.into())
}
