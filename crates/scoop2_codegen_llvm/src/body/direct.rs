//! Direct 调用 lowering：解析 callee 符号 → LLVM 调用。
//!
//! 解析顺序：
//! 1. 已在 module 中声明/定义的函数（callables + declarations）；
//! 2. runtime 符号（通过 RuntimeFns / FQN 映射）；
//! 3. intrinsic（按 callee FQN 启发式映射到内置 lowering；W1-5 完善）。
//!
//! 未解析的符号返回 `UndefinedSymbol` 错误。

use inkwell::values::BasicMetadataValueEnum;
use inkwell::values::BasicValueEnum;

use scoop2_hir::ty::TypeId;
use scoop2_lir::LirOperand;

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// lower 一个 Direct 调用。
pub fn lower_direct<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee_symbol: &str,
    args: &[LirOperand],
    result_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 0. Continuation.resume（effect_lower 重写的 resume 调用）。
    //    args = [continuation, resume_value]；调用 continuation 的 step_fn。
    if callee_symbol == "scoop.core.Continuation.resume" {
        let cont = args
            .get(0)
            .cloned()
            .unwrap_or(LirOperand::Const(scoop2_lir::LirConstValue::Null));
        let rv = args
            .get(1)
            .cloned()
            .unwrap_or(LirOperand::Const(scoop2_lir::LirConstValue::Null));
        return super::call::lower_resume_direct(fl, &cont, &rv, result_ty);
    }
    // 1. intrinsic 启发式（按 FQN）— 优先于函数声明，因为 @Intrinsic 方法
    //    虽有声明（body=None）但应内联而非调用。
    if let Some(v) =
        crate::intrinsics::try_lower_intrinsic_by_fqn(fl, callee_symbol, args, result_ty)?
    {
        return Ok(v);
    }
    // 2. 已声明/定义的函数。
    if let Some(fv) = fl
        .cg
        .lookup_callable_fn(callee_symbol)
        .or_else(|| fl.cg.module.get_function(callee_symbol))
    {
        return lower_known_fn(fl, fv, args, result_ty);
    }
    // 3. runtime 符号映射（println/print 等常用符号）。
    if let Some(fv) = resolve_runtime_symbol(fl, callee_symbol) {
        return lower_known_fn(fl, fv, args, result_ty);
    }
    Err(CodegenError::undefined_symbol(
        callee_symbol,
        &format!("direct call in {}", fl.fqn),
        scoop2_base::Span::default(),
    ))
}

/// 调用一个已知的 `FunctionValue`：lowering 实参并 build_call。
fn lower_known_fn<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    fv: inkwell::values::FunctionValue<'ctx>,
    args: &[LirOperand],
    _result_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 实参类型：从函数参数类型推导。
    let param_count = fv.count_params();
    let mut arg_vals: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len());
    for (i, operand) in args.iter().enumerate() {
        let val = lower_call_arg(fl, operand, None)?;
        // 按被调用函数的参数类型转换实参。
        let val = if (i as u32) < param_count {
            let param_ty = fv.get_nth_param(i as u32).map(|p| p.get_type());
            coerce_call_arg(fl, val, param_ty)
        } else {
            coerce_arg_addrspace(fl, val)
        };
        arg_vals.push(val.into());
    }
    let call = fl.builder.build_call(fv, &arg_vals, "call").map_err(|e| {
        CodegenError::llvm(e.to_string(), "build_call", scoop2_base::Span::default())
    })?;
    // 返回值：void 函数返回 unit（i8 zero）。
    if fv.get_type().get_return_type().is_some() {
        let result = match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => v,
            inkwell::values::ValueKind::Instruction(_) => {
                return Err(CodegenError::llvm(
                    "call 返回 InstructionValue 而非 BasicValue",
                    "lower_known_fn",
                    scoop2_base::Span::default(),
                ));
            }
        };
        // 若返回 native ptr，转换为 GC ptr（addrspace 1），与 Scoop 类型系统一致。
        let result = match result {
            BasicValueEnum::PointerValue(p) => {
                if p.get_type().get_address_space() != crate::context::gc_address_space() {
                    // native ptr → GC ptr via inttoptr.
                    let as_int = fl
                        .builder
                        .build_ptr_to_int(p, fl.cg.context.i64_type(), "ret2int")
                        .ok();
                    if let Some(int_val) = as_int {
                        let gc_ptr = fl
                            .builder
                            .build_int_to_ptr(int_val, fl.cg.gc_ptr_ty(), "ret2gc")
                            .ok();
                        if let Some(g) = gc_ptr {
                            return Ok(g.into());
                        }
                    }
                    result
                } else {
                    result
                }
            }
            _ => result,
        };
        Ok(result)
    } else {
        Ok(fl.cg.context.i8_type().const_zero().into())
    }
}

/// lower 一个调用实参。若已知参数 LLVM 类型，按其 load/转换；否则按 operand 本地类型。
fn lower_call_arg<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    operand: &LirOperand,
    param_llvm_ty: Option<inkwell::types::BasicTypeEnum<'ctx>>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match operand {
        LirOperand::Local(id) => {
            // 优先用参数类型（保证 ABI 一致）；回退到 local 声明类型。
            if let Some(_pty) = param_llvm_ty {
                fl.load_local(*id)
            } else {
                fl.load_local(*id)
            }
        }
        LirOperand::Const(c) => fl.lower_const_value(c),
    }
}

/// 把已知的 runtime 符号名（Scoop FQN）映射到 `FunctionValue`。
/// 当前覆盖：`scoop.core.println`/`scoop.core.print`（→ scoop_println/print）。
fn resolve_runtime_symbol<'a, 'ctx>(
    _fl: &FunctionLowerer<'a, 'ctx>,
    _fqn: &str,
) -> Option<inkwell::values::FunctionValue<'ctx>> {
    // println/print are now properly monomorphized user functions (println<String> etc.)
    // that call __scoop_println internally. Do NOT short-circuit them to runtime symbols.
    None
}

/// 按被调用函数参数类型转换实参（标量宽度扩展/收缩 + addrspace 转换）。
fn coerce_call_arg<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    param_ty: Option<inkwell::types::BasicTypeEnum<'ctx>>,
) -> BasicValueEnum<'ctx> {
    let Some(target_ty) = param_ty else {
        return coerce_arg_addrspace(fl, val);
    };
    match (val, target_ty) {
        // 整数：宽度不匹配时 zext/sext/trunc。
        (BasicValueEnum::IntValue(src), inkwell::types::BasicTypeEnum::IntType(dst_ty)) => {
            let src_bits = src.get_type().get_bit_width();
            let dst_bits = dst_ty.get_bit_width();
            if src_bits == dst_bits {
                src.into()
            } else if src_bits < dst_bits {
                fl.builder
                    .build_int_z_extend(src, dst_ty, "arg_ext")
                    .ok()
                    .map(|v| v.into())
                    .unwrap_or_else(|| dst_ty.const_zero().into())
            } else {
                fl.builder
                    .build_int_truncate(src, dst_ty, "arg_trunc")
                    .ok()
                    .map(|v| v.into())
                    .unwrap_or_else(|| dst_ty.const_zero().into())
            }
        }
        // 指针：按目标 addrspace 转换（目标 GC ptr → 保留/转 GC；目标 native → 转 native）。
        (BasicValueEnum::PointerValue(ptr_val), target) => {
            coerce_ptr_to_target_addrspace(fl, ptr_val, target)
        }
        // 浮点：bitcast 若宽度不同。
        (BasicValueEnum::FloatValue(fv), inkwell::types::BasicTypeEnum::FloatType(dst_ty)) => {
            if fv.get_type() == dst_ty {
                fv.into()
            } else {
                fl.builder
                    .build_bit_cast(fv, dst_ty, "arg_fcast")
                    .ok()
                    .map(|v| v.into())
                    .unwrap_or_else(|| fv.into())
            }
        }
        (v, _) => v,
    }
}

/// 按目标类型对指针做 addrspace 转换。
///
/// - 目标是 GC ptr（addrspace 1）：源 GC ptr 直接保留；源 native ptr 经 inttoptr 转 GC。
/// - 目标是 native ptr（addrspace 0）或未知：源 GC ptr 经 ptrtoint→inttoptr 转 native；其它保留。
fn coerce_ptr_to_target_addrspace<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    ptr_val: inkwell::values::PointerValue<'ctx>,
    target: inkwell::types::BasicTypeEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    let src_as = ptr_val.get_type().get_address_space();
    let gc_as = crate::context::gc_address_space();
    let target_is_gc_ptr = match target {
        inkwell::types::BasicTypeEnum::PointerType(pt) => pt.get_address_space() == gc_as,
        _ => false,
    };
    if target_is_gc_ptr {
        if src_as == gc_as {
            // 源、目标都是 GC ptr：直接保留。
            return ptr_val.into();
        }
        // 源 native → 目标 GC：经整数中转。
        if let Some(int_val) = fl
            .builder
            .build_ptr_to_int(ptr_val, fl.cg.context.i64_type(), "arg2int")
            .ok()
        {
            if let Some(g) = fl
                .builder
                .build_int_to_ptr(int_val, fl.cg.gc_ptr_ty(), "arg2gc")
                .ok()
            {
                return g.into();
            }
        }
        return ptr_val.into();
    }
    // 目标是 native ptr（或非指针：理论上不该发生，回退保留原值）。
    if src_as == gc_as {
        coerce_arg_addrspace(fl, ptr_val.into())
    } else {
        ptr_val.into()
    }
}

/// 若值是 GC 指针（addrspace 1），转换为 native 指针（addrspace 0）。
/// 用于 extern / runtime 函数调用（期望 native ptr）。
fn coerce_arg_addrspace<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
) -> BasicValueEnum<'ctx> {
    match val {
        BasicValueEnum::PointerValue(ptr_val) => {
            if ptr_val.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(ptr_val, fl.cg.context.i64_type(), "arg2int")
                    .ok();
                if let Some(int_val) = as_int {
                    let native = fl
                        .builder
                        .build_int_to_ptr(int_val, fl.cg.native_ptr_ty(), "arg2native")
                        .ok();
                    if let Some(n) = native {
                        return n.into();
                    }
                }
                val
            } else {
                val
            }
        }
        _ => val,
    }
}
