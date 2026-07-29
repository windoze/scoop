//! call lowering：`LirCall` → LLVM 调用指令。
//!
//! 覆盖：Direct 调用、Interface 分发（itable lookup）、Virtual 分发（vtable slot）、
//! Closure 调用、FunValue 调用。

use inkwell::IntPredicate;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};

use scoop2_lir::{LirCall, LirCallKind, LirOperand};

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 顶层入口：lowering 一个调用，返回其结果值。
pub fn lower_call<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    call: &LirCall,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match &call.kind {
        LirCallKind::Direct { callee_symbol, .. } => {
            super::direct::lower_direct(fl, callee_symbol, &call.args, call.result_ty)
        }
        LirCallKind::Interface {
            receiver_local,
            interface_id,
            itable_slot,
            ..
        } => lower_interface_dispatch(
            fl,
            receiver_local,
            *interface_id,
            *itable_slot,
            &call.args,
            call.result_ty,
        ),
        LirCallKind::Virtual {
            receiver_local,
            vtable_slot,
            ..
        } => lower_virtual_dispatch(fl, receiver_local, *vtable_slot, &call.args, call.result_ty),
        LirCallKind::Closure { callee_local } => {
            lower_closure_call(fl, callee_local, &call.args, call.result_ty)
        }
        LirCallKind::FunValue { callee_local } => {
            lower_funvalue_call(fl, callee_local, &call.args, call.result_ty)
        }
        LirCallKind::Resume {
            continuation,
            resume_value,
        } => lower_resume(fl, continuation, resume_value, call.result_ty),
    }
}

/// Resume continuation：`k.resume(value)`（Direct 调用入口）。
///
/// 当 devirtualize / Direct fallback 把 `Continuation.resume` 解析为 Direct 调用时
/// 由此进入；正常路径是 `LirCallKind::Resume`。两者共用 `lower_resume`。
pub fn lower_resume_direct<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    continuation: &LirOperand,
    resume_value: &LirOperand,
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    lower_resume(fl, continuation, resume_value, result_ty)
}

/// Resume continuation：`k.resume(value)`。
///
/// continuation 对象布局取 canonical 布局常量（`scoop2_lir::effect::CONT_OFFSET_*`），
/// 与 LIR `prepare_effect_abi` / synthetic_types 严格一致：
///   header(0..32) | resumed(32, i8) | state(40, i64) | frame(48) | step_fn(56) | resume_value(64)
///
/// resume 语义：
/// 1. 加载 continuation（GC ptr → native ptr）。
/// 2. 读 resumed_flag → 若已 resumed，`scoop_panic("ContinuationAlreadyResumed")`
///    （continuation 单发，不可重入，对应 RuntimeError::ContinuationAlreadyResumed）。
/// 3. 置 resumed_flag = true。
/// 4. resume_value 归一为 8 字节 word，写入 continuation 的 resume_value 字段。
/// 5. 读 step_fn_ptr + frame_ptr，间接调用 `step_fn(frame, resume_word)` → 返回 Step。
///    （Step 的具体 ABI 由 EffectStep callable lowering 定义；resume 透传其返回值。）
fn lower_resume<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    continuation: &LirOperand,
    resume_value: &LirOperand,
    _result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use scoop2_lir::effect::{
        CONT_OFFSET_FRAME, CONT_OFFSET_RESUME_VALUE, CONT_OFFSET_RESUMED, CONT_OFFSET_STEP_FN,
    };
    let i64 = fl.cg.context.i64_type();
    let i8 = fl.cg.context.i8_type();
    let native_ptr = fl.cg.native_ptr_ty();
    let llvm = |e: inkwell::builder::BuilderError, what: &str| {
        CodegenError::llvm(e.to_string(), what, scoop2_base::Span::default())
    };
    // 加载 continuation 值（GC ptr → native ptr）。
    let cont_val = match continuation {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let cont_native = match cont_val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "res_cont_int")
                    .map_err(|e| llvm(e, "res_cont_int"))?;
                fl.builder
                    .build_int_to_ptr(as_int, native_ptr, "res_cont_native")
                    .map_err(|e| llvm(e, "res_cont_native"))?
            } else {
                p
            }
        }
        _ => {
            return Err(CodegenError::llvm(
                "resume: continuation must be a pointer",
                "lower_resume",
                scoop2_base::Span::default(),
            ));
        }
    };

    // 1. 读 resumed_flag → 若 true，panic。
    let resumed_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                i8,
                cont_native,
                &[i64.const_int(CONT_OFFSET_RESUMED, false)],
                "res_resumed_slot",
            )
            .map_err(|e| llvm(e, "res_gep_resumed"))?
    };
    let resumed = fl
        .builder
        .build_load(i8, resumed_slot, "res_resumed")
        .map_err(|e| llvm(e, "res_load_resumed"))?
        .into_int_value();
    let already = fl
        .builder
        .build_int_compare(
            inkwell::IntPredicate::NE,
            resumed,
            i8.const_zero(),
            "res_already",
        )
        .map_err(|e| llvm(e, "res_already"))?;
    let ok_bb = fl.cg.context.append_basic_block(fl.fv, "resume_ok");
    let panic_bb = fl.cg.context.append_basic_block(fl.fv, "resume_panic");
    fl.builder
        .build_conditional_branch(already, panic_bb, ok_bb)
        .map_err(|e| llvm(e, "res_br"))?;
    fl.builder.position_at_end(panic_bb);
    let msg = fl
        .cg
        .get_or_create_string_literal("ContinuationAlreadyResumed")?;
    let msg_native = fl
        .builder
        .build_bit_cast(msg, native_ptr, "res_panic_msg")
        .map_err(|e| llvm(e, "res_panic_msg"))?;
    fl.builder
        .build_call(fl.rt.panic, &[msg_native.into()], "resume_panic_call")
        .map_err(|e| llvm(e, "resume_panic_call"))?;
    fl.builder
        .build_unreachable()
        .map_err(|e| llvm(e, "resume_unreachable"))?;
    fl.builder.position_at_end(ok_bb);

    // 2. 置 resumed_flag = true。
    fl.builder
        .build_store(resumed_slot, i8.const_int(1, false))
        .map_err(|e| llvm(e, "res_store_resumed"))?;

    // 3. resume_value 归一为 8 字节 word。
    let rv = match resume_value {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let rv_word = match rv {
        BasicValueEnum::PointerValue(p) => fl
            .builder
            .build_ptr_to_int(p, i64, "res_rv_word")
            .map_err(|e| llvm(e, "res_rv_word"))?,
        BasicValueEnum::IntValue(iv) => crate::intrinsics::zext_to_i64(fl, iv),
        BasicValueEnum::FloatValue(fv) => {
            let float_ty = fv.get_type();
            let bits = if float_ty == fl.cg.context.f32_type() {
                let b32 = fl
                    .builder
                    .build_bit_cast(fv, fl.cg.context.i32_type(), "res_rv_bits32")
                    .map_err(|e| llvm(e, "res_rv_bits32"))?
                    .into_int_value();
                fl.builder
                    .build_int_z_extend(b32, i64, "res_rv_zext")
                    .map_err(|e| llvm(e, "res_rv_zext"))?
            } else {
                fl.builder
                    .build_bit_cast(fv, i64, "res_rv_bits")
                    .map_err(|e| llvm(e, "res_rv_bits"))?
                    .into_int_value()
            };
            bits
        }
        other => {
            return Err(CodegenError::unsupported(
                format!(
                    "resume value 类型不支持按 word 传递：{:?}（复合值需装箱）",
                    other.get_type()
                ),
                &fl.fqn,
                scoop2_base::Span::default(),
            ));
        }
    };

    // 4. 写 resume_value word 到 continuation 的 resume_value 字段。
    let rv_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                i8,
                cont_native,
                &[i64.const_int(CONT_OFFSET_RESUME_VALUE, false)],
                "res_rv_slot",
            )
            .map_err(|e| llvm(e, "res_gep_rv"))?
    };
    fl.builder
        .build_store(rv_slot, rv_word)
        .map_err(|e| llvm(e, "res_store_rv"))?;

    // 5. 读 step_fn_ptr + frame_ptr，间接调用 step_fn(frame, resume_word)。
    let step_fn_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                i8,
                cont_native,
                &[i64.const_int(CONT_OFFSET_STEP_FN, false)],
                "res_stepfn_slot",
            )
            .map_err(|e| llvm(e, "res_gep_stepfn"))?
    };
    let step_fn = fl
        .builder
        .build_load(native_ptr, step_fn_slot, "res_stepfn")
        .map_err(|e| llvm(e, "res_load_stepfn"))?
        .into_pointer_value();
    let frame_slot = unsafe {
        fl.builder
            .build_in_bounds_gep(
                i8,
                cont_native,
                &[i64.const_int(CONT_OFFSET_FRAME, false)],
                "res_frame_slot",
            )
            .map_err(|e| llvm(e, "res_gep_frame"))?
    };
    let frame_ptr = fl
        .builder
        .build_load(native_ptr, frame_slot, "res_frame")
        .map_err(|e| llvm(e, "res_load_frame"))?
        .into_pointer_value();
    // step_fn 签名：ptr step_fn(ptr frame, i64 resume_word) → 返回 Step。
    let step_fn_ty = native_ptr.fn_type(&[native_ptr.into(), i64.into()], false);
    let call = fl
        .builder
        .build_indirect_call(
            step_fn_ty,
            step_fn,
            &[frame_ptr.into(), rv_word.into()],
            "res_step_call",
        )
        .map_err(|e| llvm(e, "res_step_call"))?;
    match call.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => Ok(v),
        inkwell::values::ValueKind::Instruction(_) => Err(CodegenError::llvm(
            "resume: step_fn 未返回值",
            "lower_resume",
            scoop2_base::Span::default(),
        )),
    }
}

/// Interface 分发：receiver → header.type_desc → itable → 按 interface_id 查找 → methods[slot] → call。
fn lower_interface_dispatch<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    interface_id: u64,
    itable_slot: u32,
    args: &[LirOperand],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let native_ptr = fl.cg.native_ptr_ty();

    // 1. receiver → type_desc（header field 1）。
    let recv_native = get_type_desc_ptr(fl, receiver)?;

    // 2. type_desc.itable（field index 12）。
    let itable =
        load_struct_field_ptr(fl, recv_native, fl.cg.type_descriptor_type(), 12, "itable")?;

    // 3. itable 容器: { i32 count; i32 _pad; ptr entries }。count=field 0, entries=field 2。
    let container_ty = fl.cg.itable_container_type_pub();
    let count = load_struct_field_int(fl, itable, container_ty, 0, "itable_count")?;
    let entries_ptr = load_struct_field_ptr(fl, itable, container_ty, 2, "entries_ptr")?;

    // 4. 线性扫描 entries（entry: { u64 interface_id; ptr methods }），找匹配。
    let entry_ty = fl.cg.itable_entry_type_pub();
    let i64_ty = fl.cg.context.i64_type();
    let loop_bb = fl.cg.context.append_basic_block(fl.fv, "itable_loop");
    let found_bb = fl.cg.context.append_basic_block(fl.fv, "itable_found");
    let inc_bb = fl.cg.context.append_basic_block(fl.fv, "itable_inc");
    let not_found_bb = fl.cg.context.append_basic_block(fl.fv, "itable_notfound");
    let merge_bb = fl.cg.context.append_basic_block(fl.fv, "itable_merge");

    let i_slot = fl
        .builder
        .build_alloca(fl.cg.context.i32_type(), "i")
        .map_err(|e| CodegenError::llvm(e.to_string(), "alloca i", scoop2_base::Span::default()))?;
    fl.builder
        .build_store(i_slot, fl.cg.context.i32_type().const_zero())
        .map_err(|e| CodegenError::llvm(e.to_string(), "store i", scoop2_base::Span::default()))?;
    let _ = fl.builder.build_unconditional_branch(loop_bb);

    // loop: if i < count → body; else → not_found.
    fl.builder.position_at_end(loop_bb);
    let i_val = fl
        .builder
        .build_load(fl.cg.context.i32_type(), i_slot, "i_val")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load i", scoop2_base::Span::default()))?
        .into_int_value();
    let cond = fl
        .builder
        .build_int_compare(IntPredicate::SLT, i_val, count, "loop_cond")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "icmp loop", scoop2_base::Span::default())
        })?;
    let body_bb = fl.cg.context.append_basic_block(fl.fv, "itable_body");
    let _ = fl
        .builder
        .build_conditional_branch(cond, body_bb, not_found_bb);

    // body: load entry[i].interface_id, compare.
    fl.builder.position_at_end(body_bb);
    let entry_ptr = unsafe {
        fl.builder
            .build_in_bounds_gep(entry_ty, entries_ptr, &[i_val], "entry_ptr")
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep entry", scoop2_base::Span::default()))?;
    let entry_iface_id = load_struct_field_int_raw(fl, entry_ptr, entry_ty, 0, "entry_iface_id")?;
    let match_cond = fl
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            entry_iface_id,
            i64_ty.const_int(interface_id, false),
            "iface_match",
        )
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "icmp iface", scoop2_base::Span::default())
        })?;
    let _ = fl
        .builder
        .build_conditional_branch(match_cond, found_bb, inc_bb);

    // found: load entry.methods → methods[slot] → fn ptr.
    fl.builder.position_at_end(found_bb);
    let methods_arr = load_struct_field_ptr(fl, entry_ptr, entry_ty, 1, "methods_arr")?;
    let fn_ptr_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            native_ptr,
            methods_arr,
            &[fl.cg
                .context
                .i32_type()
                .const_int(itable_slot as u64, false)],
            "fn_ptr_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep fn_ptr", scoop2_base::Span::default()))?;
    let fn_val = fl
        .builder
        .build_load(native_ptr, fn_ptr_slot, "fn_val")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load fn", scoop2_base::Span::default()))?
        .into_pointer_value();
    let _ = fl.builder.build_unconditional_branch(merge_bb);

    // inc: i++ → loop.
    fl.builder.position_at_end(inc_bb);
    let i_next = fl
        .builder
        .build_int_add(
            i_val,
            fl.cg.context.i32_type().const_int(1, false),
            "i_next",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "add i", scoop2_base::Span::default()))?;
    fl.builder.build_store(i_slot, i_next).map_err(|e| {
        CodegenError::llvm(e.to_string(), "store i_next", scoop2_base::Span::default())
    })?;
    let _ = fl.builder.build_unconditional_branch(loop_bb);

    // not_found: 调用 scoop_runtime_error_fatal（不能返回 null，否则 LLVM 会把后续代码视为 UB）。
    fl.builder.position_at_end(not_found_bb);
    let panic_msg = fl
        .cg
        .get_or_create_string_literal("interface dispatch failed: method not found")?;
    let panic_native = fl
        .builder
        .build_ptr_to_int(panic_msg, fl.cg.context.i64_type(), "panic_msg_int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int panic",
                scoop2_base::Span::default(),
            )
        })?;
    let panic_native_ptr = fl
        .builder
        .build_int_to_ptr(panic_native, fl.cg.native_ptr_ty(), "panic_msg_ptr")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr panic",
                scoop2_base::Span::default(),
            )
        })?;
    let _ = fl
        .builder
        .build_call(
            fl.rt.runtime_error_fatal,
            &[panic_native_ptr.into()],
            "fatal",
        )
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "call fatal", scoop2_base::Span::default())
        })?;
    // fatal is noreturn; add unreachable to prevent LLVM from treating
    // the not_found → merge edge as a valid path (which would make
    // the phi have a null incoming and cause UB).
    let _ = fl.builder.build_unreachable();
    // not_found does NOT branch to merge_bb (it's unreachable after fatal).
    fl.builder.position_at_end(merge_bb);
    let phi = fl
        .builder
        .build_phi(native_ptr, "resolved_fn")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "build_phi", scoop2_base::Span::default())
        })?;
    // not_found 分支调用了 fatal（noreturn），但 phi 仍需要一个 incoming value。
    // 用 native_ptr.const_null() 作为占位（not_found 分支实际不会到达 merge_bb，
    // 因为 fatal 是 noreturn；但 LLVM 需要 phi 的 incoming 类型一致）。
    // not_found 分支以 unreachable 结束，不到达 merge_bb。
    // phi 只有 found_bb 一个 incoming。
    phi.add_incoming(&[(&fn_val, found_bb)]);
    let resolved_fn = phi.as_basic_value().into_pointer_value();

    // 5. 间接调用。
    call_fn_ptr(fl, resolved_fn, receiver, args, result_ty, true, None)
}

/// Virtual 分发：receiver → type_desc → vtable → slot → call。
fn lower_virtual_dispatch<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    vtable_slot: u32,
    args: &[LirOperand],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let native_ptr = fl.cg.native_ptr_ty();
    let td_ptr = get_type_desc_ptr(fl, receiver)?;
    // type_desc.vtable（field index 13）。
    let vtable = load_struct_field_ptr(fl, td_ptr, fl.cg.type_descriptor_type(), 13, "vtable")?;
    let fn_ptr_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            native_ptr,
            vtable,
            &[fl.cg
                .context
                .i32_type()
                .const_int(vtable_slot as u64, false)],
            "vfn_ptr_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep vfn", scoop2_base::Span::default()))?;
    let resolved_fn = fl
        .builder
        .build_load(native_ptr, fn_ptr_slot, "vfn_val")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load vfn", scoop2_base::Span::default()))?
        .into_pointer_value();
    call_fn_ptr(fl, resolved_fn, receiver, args, result_ty, true, None)
}

/// Closure 调用：closure 对象 = { header; env_ptr; invoke_fn_ptr }。
fn lower_closure_call<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee: &LirOperand,
    args: &[LirOperand],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let native_ptr = fl.cg.native_ptr_ty();
    let closure_native = get_native_ptr_from_operand(fl, callee)?;
    let header_size = fl
        .cg
        .target_data
        .get_store_size(&fl.cg.object_header_type());
    let fn_offset = header_size + fl.cg.pointer_byte_size;
    let fn_ptr_i8 = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            closure_native,
            &[fl.cg.context.i64_type().const_int(fn_offset, false)],
            "cl_fn_i8",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep cl_fn", scoop2_base::Span::default()))?;
    let invoke_fn = fl
        .builder
        .build_load(native_ptr, fn_ptr_i8, "cl_invoke_fn")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load cl_fn", scoop2_base::Span::default()))?
        .into_pointer_value();
    // 统一闭包 ABI：首参传 env blob 指针（从 closure 对象的 env_ptr 槽加载）。
    let env_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            closure_native,
            &[fl.cg.context.i64_type().const_int(header_size, false)],
            "cl_env_i8",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep cl_env", scoop2_base::Span::default()))?;
    let env_native = fl
        .builder
        .build_load(native_ptr, env_slot, "cl_env")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "load cl_env", scoop2_base::Span::default())
        })?
        .into_pointer_value();
    let env_int = fl
        .builder
        .build_ptr_to_int(env_native, fl.cg.context.i64_type(), "cl_env_int")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "ptr_to_int cl_env",
                scoop2_base::Span::default(),
            )
        })?;
    let env_gc = fl
        .builder
        .build_int_to_ptr(env_int, fl.cg.gc_ptr_ty(), "cl_env_gc")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "int_to_ptr cl_env",
                scoop2_base::Span::default(),
            )
        })?;
    call_fn_ptr(fl, invoke_fn, callee, args, result_ty, false, Some(env_gc))
}

/// FunValue 调用：与 Closure 相同。
fn lower_funvalue_call<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee: &LirOperand,
    args: &[LirOperand],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    lower_closure_call(fl, callee, args, result_ty)
}

// ---- 辅助函数 ----

/// 从 receiver operand 加载 type_desc 的 native 指针。
/// receiver → GC ptr → native ptr → header.type_desc (field 1)。
fn get_type_desc_ptr<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
    let recv_native = get_native_ptr_from_operand(fl, receiver)?;
    load_struct_field_ptr(fl, recv_native, fl.cg.object_header_type(), 1, "type_desc")
}

/// 把 operand load 为 native 指针（GC ptr 经 i64 中转）。
fn get_native_ptr_from_operand<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    operand: &LirOperand,
) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
    let gc_ptr = match operand {
        LirOperand::Local(id) => {
            super::expect_ptr_val(fl.load_local(*id)?, "call receiver/closure", &fl.fqn)?
        }
        LirOperand::Const(c) => {
            super::expect_ptr_val(fl.lower_const_value(c)?, "call receiver/closure", &fl.fqn)?
        }
    };
    let native_ptr = fl.cg.native_ptr_ty();
    let as_int = fl
        .builder
        .build_ptr_to_int(gc_ptr, fl.cg.context.i64_type(), "gc2int")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "ptr_to_int", scoop2_base::Span::default())
        })?;
    let native = fl
        .builder
        .build_int_to_ptr(as_int, native_ptr, "int2native")
        .map_err(|e| {
            CodegenError::llvm(e.to_string(), "int_to_ptr", scoop2_base::Span::default())
        })?;
    Ok(native)
}

/// 从 struct 指针的指定 field index load 一个 native 指针。
fn load_struct_field_ptr<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    struct_ptr: inkwell::values::PointerValue<'ctx>,
    struct_ty: inkwell::types::StructType<'ctx>,
    field_index: u32,
    name: &str,
) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
    let native_ptr = fl.cg.native_ptr_ty();
    let slot = unsafe {
        fl.builder
            .build_struct_gep(struct_ty, struct_ptr, field_index, name)
    }
    .map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            format!("gep {name}"),
            scoop2_base::Span::default(),
        )
    })?;
    let val = fl
        .builder
        .build_load(native_ptr, slot, name)
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                format!("load {name}"),
                scoop2_base::Span::default(),
            )
        })?
        .into_pointer_value();
    Ok(val)
}

/// 从 struct 指针的指定 field index load 一个 i32 值。
fn load_struct_field_int<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    struct_ptr: inkwell::values::PointerValue<'ctx>,
    struct_ty: inkwell::types::StructType<'ctx>,
    field_index: u32,
    name: &str,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let slot = unsafe {
        fl.builder
            .build_struct_gep(struct_ty, struct_ptr, field_index, name)
    }
    .map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            format!("gep {name}"),
            scoop2_base::Span::default(),
        )
    })?;
    let val = fl
        .builder
        .build_load(fl.cg.context.i32_type(), slot, name)
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                format!("load {name}"),
                scoop2_base::Span::default(),
            )
        })?
        .into_int_value();
    Ok(val)
}

/// 从 struct 指针的指定 field index load 一个 i64 值（itable entry interface_id）。
fn load_struct_field_int_raw<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    struct_ptr: inkwell::values::PointerValue<'ctx>,
    struct_ty: inkwell::types::StructType<'ctx>,
    field_index: u32,
    name: &str,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let slot = unsafe {
        fl.builder
            .build_struct_gep(struct_ty, struct_ptr, field_index, name)
    }
    .map_err(|e| {
        CodegenError::llvm(
            e.to_string(),
            format!("gep {name}"),
            scoop2_base::Span::default(),
        )
    })?;
    let val = fl
        .builder
        .build_load(fl.cg.context.i64_type(), slot, name)
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                format!("load {name}"),
                scoop2_base::Span::default(),
            )
        })?
        .into_int_value();
    Ok(val)
}

/// 通过函数指针间接调用。
/// `prepend_receiver`：是否把 receiver 作为首参（interface/virtual 需要传 receiver；closure 不需要）。
fn call_fn_ptr<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    fn_ptr: inkwell::values::PointerValue<'ctx>,
    receiver: &LirOperand,
    args: &[LirOperand],
    result_ty: scoop2_hir::ty::TypeId,
    prepend_receiver: bool,
    closure_env: Option<inkwell::values::PointerValue<'ctx>>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let ret_llvm = fl.cg.lower_type(result_ty, fl.layouts)?;

    // 构造参数列表（先收集为 BasicValueEnum，参数类型随后从实际值推导）。
    let mut call_arg_vals: Vec<BasicValueEnum<'ctx>> = Vec::new();
    if prepend_receiver {
        // receiver 作为 GC ptr（addrspace 1）首参——与成员函数 this 参数类型一致。
        let recv_gc = match receiver {
            LirOperand::Local(id) => {
                super::expect_ptr_val(fl.load_local(*id)?, "call receiver", &fl.fqn)?
            }
            LirOperand::Const(c) => {
                super::expect_ptr_val(fl.lower_const_value(c)?, "call receiver", &fl.fqn)?
            }
        };
        call_arg_vals.push(recv_gc.into());
    }
    if let Some(env_gc) = closure_env {
        // 统一闭包 ABI：env blob 指针作为首参（invoke 函数的 `$env` 形参为 GC ptr）。
        call_arg_vals.push(env_gc.into());
    }
    for operand in args {
        let arg_val = fl.lower_operand(operand, result_ty)?;
        call_arg_vals.push(arg_val);
    }
    let call_args: Vec<BasicMetadataValueEnum<'ctx>> =
        call_arg_vals.iter().map(|&v| v.into()).collect();

    // 函数类型：返回类型 + 从实际参数值推导的参数类型（保证 call site 与 fn_ty 一致；
    // 被调方的声明签名由静态类型一致性保证——receiver/env 均为 GC ptr，
    // 其余参数按各自静态类型 lower，与 invoke/成员函数的形参 lower 结果相同）。
    let param_tys: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
        call_arg_vals.iter().map(|v| v.get_type().into()).collect();
    let fn_ty = match ret_llvm {
        inkwell::types::BasicTypeEnum::IntType(t) => t.fn_type(&param_tys, false),
        inkwell::types::BasicTypeEnum::FloatType(t) => t.fn_type(&param_tys, false),
        inkwell::types::BasicTypeEnum::PointerType(t) => t.fn_type(&param_tys, false),
        inkwell::types::BasicTypeEnum::StructType(t) => t.fn_type(&param_tys, false),
        inkwell::types::BasicTypeEnum::ArrayType(t) => t.fn_type(&param_tys, false),
        inkwell::types::BasicTypeEnum::VectorType(t) => t.fn_type(&param_tys, false),
        inkwell::types::BasicTypeEnum::ScalableVectorType(_) => {
            fl.cg.context.i8_type().fn_type(&param_tys, false)
        }
    };

    // 用 build_indirect_call（inkwell 0.8 正式 API）。
    let call_site = fl
        .builder
        .build_indirect_call(fn_ty, fn_ptr, &call_args, "dispatch")
        .map_err(|e| {
            CodegenError::llvm(
                e.to_string(),
                "build_indirect_call",
                scoop2_base::Span::default(),
            )
        })?;

    if fn_ty.get_return_type().is_some() {
        match call_site.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Ok(fl.cg.context.i8_type().const_zero().into())
            }
        }
    } else {
        Ok(fl.cg.context.i8_type().const_zero().into())
    }
}
