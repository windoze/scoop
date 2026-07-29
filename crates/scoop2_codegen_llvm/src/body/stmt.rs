//! stmt lowering：`LirStmtKind` → LLVM 指令序列。

use inkwell::values::{BasicValue, BasicValueEnum};
use scoop2_lir::LirStmt;

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 若 rvalue 的静态类型与 target 类型是不同的 Option 表示，把值转换到
/// target 的 niche 表示；否则原样返回。
fn maybe_coerce_option_assign<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &scoop2_lir::LirRvalue,
    target_ty: scoop2_hir::ty::TypeId,
    v: inkwell::values::BasicValueEnum<'ctx>,
) -> CodegenResult<inkwell::values::BasicValueEnum<'ctx>> {
    let static_ty = match value {
        scoop2_lir::LirRvalue::Use(id) => fl.local_types.get(id).copied(),
        scoop2_lir::LirRvalue::EnumVariant { enum_ty, .. } => Some(*enum_ty),
        scoop2_lir::LirRvalue::Call(c) => Some(c.result_ty),
        scoop2_lir::LirRvalue::PatternExtract { result_ty, .. } => Some(*result_ty),
        _ => None,
    };
    let Some(st) = static_ty else {
        return Ok(v);
    };
    if st == target_ty {
        return Ok(v);
    }
    let both_option = [st, target_ty].iter().all(|t| {
        matches!(
            fl.layouts.get(*t).map(|l| &l.kind),
            Some(scoop2_lir::TypeLayoutKind::Option { .. })
        )
    });
    if both_option {
        super::rvalue::coerce_option_value(fl, v, st, target_ty)
    } else {
        Ok(v)
    }
}

/// 顶层入口：lowering 一条语句。
pub fn lower_stmt<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    stmt: &LirStmt,
) -> CodegenResult<()> {
    use scoop2_lir::LirStmtKind;
    match &stmt.kind {
        LirStmtKind::Nop => Ok(()),
        LirStmtKind::Assign { target, value } => {
            // target 的类型从 local_types 取。
            let target_ty = fl.local_types.get(target).copied().ok_or_else(|| {
                CodegenError::unsupported(
                    format!("Assign target local {} 类型未知", target),
                    &fl.fqn,
                    scoop2_base::Span::default(),
                )
            })?;
            let v = super::rvalue::lower_rvalue(fl, value, target_ty)?;
            // Option 表示归一：rvalue 的静态类型与 target 类型可能都是 Option 但
            // niche 表示不同（典型：inline 后 `result = Use(none_tmp)`，
            // none_tmp: Option(Nothing) 是 Pointer niche，result: Option<T> 是
            // Tagged）。opaque pointer 下类型不符的 store 不会报验证错误而是
            // 静默写坏内存，必须显式转换。
            let v = maybe_coerce_option_assign(fl, value, target_ty, v)?;
            fl.store_local(*target, v)?;
            Ok(())
        }
        LirStmtKind::Panic { message } => {
            // 调用 scoop_panic(string)。
            let s = fl.cg.get_or_create_string_literal(message)?;
            // scoop_panic 是 native void*；把 GC 指针 cast 到 native。
            let native = fl
                .builder
                .build_bit_cast(s, fl.cg.native_ptr_ty(), "panic_msg")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "build_bit_cast panic",
                        scoop2_base::Span::default(),
                    )
                })?;
            let _ = fl
                .builder
                .build_call(fl.rt.panic, &[native.into()], "panic")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "build_call panic",
                        scoop2_base::Span::default(),
                    )
                })?;
            // panic 不返回；后续由 unreachable 兜底（若 block 还有 terminator）。
            Ok(())
        }
        LirStmtKind::StoreMember {
            receiver_local,
            receiver_ty,
            field_offset,
            value_local,
            value_ty,
            ..
        } => {
            // class/struct 字段写：GEP(field_offset) + store + write barrier。
            let recv_val = fl.lower_operand(receiver_local, *receiver_ty)?;
            let val = fl.lower_operand(value_local, *value_ty)?;
            let native = fl
                .builder
                .build_ptr_to_int(
                    super::expect_ptr_val(recv_val, "StoreMember receiver", &fl.fqn)?,
                    fl.cg.context.i64_type(),
                    "sm_recv2int",
                )
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "ptr_to_int store_member",
                        scoop2_base::Span::default(),
                    )
                })?;
            let native_ptr = fl
                .builder
                .build_int_to_ptr(native, fl.cg.native_ptr_ty(), "sm_recv_native")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "int_to_ptr store_member",
                        scoop2_base::Span::default(),
                    )
                })?;
            let field_slot = unsafe {
                fl.builder.build_in_bounds_gep(
                    fl.cg.context.i8_type(),
                    native_ptr,
                    &[fl.cg.context.i64_type().const_int(*field_offset, false)],
                    "sm_field_slot",
                )
            }
            .map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "gep store_member",
                    scoop2_base::Span::default(),
                )
            })?;
            // GC ptr → native ptr for store。
            let val_native = match val {
                BasicValueEnum::PointerValue(p) => {
                    let pi = fl
                        .builder
                        .build_ptr_to_int(p, fl.cg.context.i64_type(), "sm_val_int")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "ptr_to_int store_member_val",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    fl.builder
                        .build_int_to_ptr(pi, fl.cg.native_ptr_ty(), "sm_val_native")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "int_to_ptr store_member_val",
                                scoop2_base::Span::default(),
                            )
                        })?
                }
                _ => {
                    // For non-pointer values, store via inttoptr conversion.
                    let val_int = super::expect_int_val(val, "StoreMember 非标量字段值", &fl.fqn)?;
                    let val_ptr = fl
                        .builder
                        .build_int_to_ptr(val_int, fl.cg.native_ptr_ty(), "sm_intval_ptr")
                        .map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "int_to_ptr store_member_intval",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    fl.builder.build_store(field_slot, val_ptr).map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "store member intval",
                            scoop2_base::Span::default(),
                        )
                    })?;
                    return Ok(());
                }
            };
            fl.builder
                .build_store(field_slot, val_native)
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "store member", scoop2_base::Span::default())
                })?;
            Ok(())
        }
        LirStmtKind::StoreTupleIndex {
            receiver_local,
            index,
            value_local,
            value_ty,
        } => {
            let recv_val = fl.lower_operand(receiver_local, *value_ty)?;
            let val = fl.lower_operand(value_local, *value_ty)?;
            let agg = super::expect_struct_val(recv_val, "StoreTupleIndex receiver", &fl.fqn)?;
            let _ = fl
                .builder
                .build_insert_value(agg, val, *index as u32, "store_ti")
                .map_err(|e| {
                    CodegenError::llvm(
                        e.to_string(),
                        "insert store_ti",
                        scoop2_base::Span::default(),
                    )
                })?;
            Ok(())
        }
        LirStmtKind::StoreGlobal {
            global_fqn,
            value_local,
            value_ty,
        } => {
            // 顶层 var 赋值：store 到全局 backing slot。
            let val = fl.lower_operand(value_local, *value_ty)?;
            if let Some(gv) = fl.cg.lookup_global(&global_fqn) {
                let val_native = match val {
                    BasicValueEnum::PointerValue(p) => {
                        let pi = fl
                            .builder
                            .build_ptr_to_int(p, fl.cg.context.i64_type(), "sg_val_int")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "ptr_to_int store_global",
                                    scoop2_base::Span::default(),
                                )
                            })?;
                        fl.builder
                            .build_int_to_ptr(pi, fl.cg.native_ptr_ty(), "sg_val_native")
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "int_to_ptr store_global",
                                    scoop2_base::Span::default(),
                                )
                            })?
                    }
                    _ => fl.cg.native_ptr_ty().const_null(),
                };
                fl.builder.build_store(gv, val_native).map_err(|e| {
                    CodegenError::llvm(e.to_string(), "store global", scoop2_base::Span::default())
                })?;
            }
            Ok(())
        }
    }
}
