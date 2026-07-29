//! call lowering：`LirCall` → LLVM 调用指令。
//!
//! 覆盖：Direct 调用、Interface 分发（itable lookup）、Virtual 分发（vtable slot）、
//! Closure 调用、FunValue 调用。

use inkwell::values::{BasicValueEnum, BasicMetadataValueEnum};
use inkwell::IntPredicate;

use scoop2_lir::{LirCall, LirCallKind, LirOperand};

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 顶层入口：lowering 一个调用，返回其结果值。
pub fn lower_call<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    call: &LirCall,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match &call.kind {
        LirCallKind::Direct { callee_symbol } => {
            super::direct::lower_direct(fl, callee_symbol, &call.args, call.result_ty)
        }
        LirCallKind::Interface {
            receiver_local,
            interface_id,
            itable_slot,
            ..
        } => lower_interface_dispatch(fl, receiver_local, *interface_id, *itable_slot, &call.args, call.result_ty),
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
    let itable = load_struct_field_ptr(fl, recv_native, fl.cg.type_descriptor_type(), 12, "itable")?;

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
        .map_err(|e| CodegenError::llvm(e.to_string(), "icmp loop", scoop2_base::Span::default()))?;
    let body_bb = fl.cg.context.append_basic_block(fl.fv, "itable_body");
    let _ = fl.builder.build_conditional_branch(cond, body_bb, not_found_bb);

    // body: load entry[i].interface_id, compare.
    fl.builder.position_at_end(body_bb);
    let entry_ptr = unsafe {
        fl.builder.build_in_bounds_gep(entry_ty, entries_ptr, &[i_val], "entry_ptr")
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
        .map_err(|e| CodegenError::llvm(e.to_string(), "icmp iface", scoop2_base::Span::default()))?;
    let _ = fl.builder.build_conditional_branch(match_cond, found_bb, inc_bb);

    // found: load entry.methods → methods[slot] → fn ptr.
    fl.builder.position_at_end(found_bb);
    let methods_arr = load_struct_field_ptr(fl, entry_ptr, entry_ty, 1, "methods_arr")?;
    let fn_ptr_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            native_ptr,
            methods_arr,
            &[fl.cg.context.i32_type().const_int(itable_slot as u64, false)],
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
        .build_int_add(i_val, fl.cg.context.i32_type().const_int(1, false), "i_next")
        .map_err(|e| CodegenError::llvm(e.to_string(), "add i", scoop2_base::Span::default()))?;
    fl.builder
        .build_store(i_slot, i_next)
        .map_err(|e| CodegenError::llvm(e.to_string(), "store i_next", scoop2_base::Span::default()))?;
    let _ = fl.builder.build_unconditional_branch(loop_bb);

    // not_found: 调用 scoop_runtime_error_fatal（不能返回 null，否则 LLVM 会把后续代码视为 UB）。
    fl.builder.position_at_end(not_found_bb);
    let panic_msg = fl.cg.get_or_create_string_literal("interface dispatch failed: method not found")?;
    let panic_native = fl
        .builder
        .build_ptr_to_int(panic_msg, fl.cg.context.i64_type(), "panic_msg_int")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int panic", scoop2_base::Span::default()))?;
    let panic_native_ptr = fl
        .builder
        .build_int_to_ptr(panic_native, fl.cg.native_ptr_ty(), "panic_msg_ptr")
        .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr panic", scoop2_base::Span::default()))?;
    let _ = fl
        .builder
        .build_call(fl.rt.runtime_error_fatal, &[panic_native_ptr.into()], "fatal")
        .map_err(|e| CodegenError::llvm(e.to_string(), "call fatal", scoop2_base::Span::default()))?;
    // fatal is noreturn; add unreachable to prevent LLVM from treating
    // the not_found → merge edge as a valid path (which would make
    // the phi have a null incoming and cause UB).
    let _ = fl.builder.build_unreachable();
    // not_found does NOT branch to merge_bb (it's unreachable after fatal).
    fl.builder.position_at_end(merge_bb);
    let phi = fl
        .builder
        .build_phi(native_ptr, "resolved_fn")
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_phi", scoop2_base::Span::default()))?;
    // not_found 分支调用了 fatal（noreturn），但 phi 仍需要一个 incoming value。
    // 用 native_ptr.const_null() 作为占位（not_found 分支实际不会到达 merge_bb，
    // 因为 fatal 是 noreturn；但 LLVM 需要 phi 的 incoming 类型一致）。
    // not_found 分支以 unreachable 结束，不到达 merge_bb。
    // phi 只有 found_bb 一个 incoming。
    phi.add_incoming(&[(&fn_val, found_bb)]);
    let resolved_fn = phi.as_basic_value().into_pointer_value();

    // 5. 间接调用。
    call_fn_ptr(fl, resolved_fn, receiver, args, result_ty, true)
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
            &[fl.cg.context.i32_type().const_int(vtable_slot as u64, false)],
            "vfn_ptr_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep vfn", scoop2_base::Span::default()))?;
    let resolved_fn = fl
        .builder
        .build_load(native_ptr, fn_ptr_slot, "vfn_val")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load vfn", scoop2_base::Span::default()))?
        .into_pointer_value();
    call_fn_ptr(fl, resolved_fn, receiver, args, result_ty, true)
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
    let header_size = fl.cg.target_data.get_store_size(&fl.cg.object_header_type());
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
    call_fn_ptr(fl, invoke_fn, callee, args, result_ty, false)
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
        LirOperand::Local(id) => fl.load_local(*id)?.into_pointer_value(),
        LirOperand::Const(c) => fl.lower_const_value(c)?.into_pointer_value(),
    };
    let native_ptr = fl.cg.native_ptr_ty();
    let as_int = fl
        .builder
        .build_ptr_to_int(gc_ptr, fl.cg.context.i64_type(), "gc2int")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int", scoop2_base::Span::default()))?;
    let native = fl
        .builder
        .build_int_to_ptr(as_int, native_ptr, "int2native")
        .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr", scoop2_base::Span::default()))?;
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
    .map_err(|e| CodegenError::llvm(e.to_string(), format!("gep {name}"), scoop2_base::Span::default()))?;
    let val = fl
        .builder
        .build_load(native_ptr, slot, name)
        .map_err(|e| CodegenError::llvm(e.to_string(), format!("load {name}"), scoop2_base::Span::default()))?
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
    .map_err(|e| CodegenError::llvm(e.to_string(), format!("gep {name}"), scoop2_base::Span::default()))?;
    let val = fl
        .builder
        .build_load(fl.cg.context.i32_type(), slot, name)
        .map_err(|e| CodegenError::llvm(e.to_string(), format!("load {name}"), scoop2_base::Span::default()))?
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
    .map_err(|e| CodegenError::llvm(e.to_string(), format!("gep {name}"), scoop2_base::Span::default()))?;
    let val = fl
        .builder
        .build_load(fl.cg.context.i64_type(), slot, name)
        .map_err(|e| CodegenError::llvm(e.to_string(), format!("load {name}"), scoop2_base::Span::default()))?
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
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let ret_llvm = fl.cg.lower_type(result_ty, fl.layouts)?;
    let native_ptr = fl.cg.native_ptr_ty();

    // 构造参数列表。
    let gc_ptr_ty = fl.cg.gc_ptr_ty();
    let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
    if prepend_receiver {
        // receiver 作为 GC ptr（addrspace 1）首参——与成员函数 this 参数类型一致。
        let recv_gc = match receiver {
            LirOperand::Local(id) => fl.load_local(*id)?.into_pointer_value(),
            LirOperand::Const(c) => fl.lower_const_value(c)?.into_pointer_value(),
        };
        call_args.push(recv_gc.into());
    }
    for operand in args {
        let arg_val = fl.lower_operand(operand, result_ty)?;
        call_args.push(arg_val.into());
    }

    // 函数类型：用返回类型 + 参数类型构造。
    let param_tys: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = if prepend_receiver {
        let mut v: Vec<_> = vec![gc_ptr_ty.into()]; // receiver = GC ptr
        for _ in args {
            v.push(ret_llvm.into());
        }
        v
    } else {
        let mut v: Vec<_> = vec![gc_ptr_ty.into()]; // env_ptr = GC ptr
        for _ in args {
            v.push(ret_llvm.into());
        }
        v
    };
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
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_indirect_call", scoop2_base::Span::default()))?;

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
