//! Direct 调用 lowering：解析 callee 符号 → LLVM 调用。
//!
//! 解析顺序：
//! 1. 已在 module 中声明/定义的函数（callables + declarations）；
//! 2. runtime 符号（通过 RuntimeFns / FQN 映射）；
//! 3. intrinsic（按 callee FQN 启发式映射到内置 lowering；W1-5 完善）。
//!
//! 未解析的符号返回 `UndefinedSymbol` 错误。

use inkwell::values::BasicValueEnum;
use inkwell::values::BasicMetadataValueEnum;

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
    // 1. intrinsic 启发式（按 FQN）— 优先于函数声明，因为 @Intrinsic 方法
    //    虽有声明（body=None）但应内联而非调用。
    if let Some(v) = crate::intrinsics::try_lower_intrinsic_by_fqn(fl, callee_symbol, args, result_ty)? {
        return Ok(v);
    }
    // 2. 已声明/定义的函数。
    if let Some(fv) = fl.cg.lookup_callable_fn(callee_symbol).or_else(|| fl.cg.module.get_function(callee_symbol)) {
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
    let call = fl
        .builder
        .build_call(fv, &arg_vals, "call")
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_call", scoop2_base::Span::default()))?;
    // 返回值：void 函数返回 unit（i8 zero）。
    if fv.get_type().get_return_type().is_some() {
        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => Err(CodegenError::llvm(
                "call 返回 InstructionValue 而非 BasicValue",
                "lower_known_fn",
                scoop2_base::Span::default(),
            )),
        }
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
        // 指针：addrspace 转换。
        (BasicValueEnum::PointerValue(ptr_val), _) => {
            coerce_arg_addrspace(fl, ptr_val.into())
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
