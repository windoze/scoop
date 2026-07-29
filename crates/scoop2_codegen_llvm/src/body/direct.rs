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
    // 1. 已声明/定义的函数。
    if let Some(fv) = fl.cg.lookup_callable_fn(callee_symbol).or_else(|| fl.cg.module.get_function(callee_symbol)) {
        return lower_known_fn(fl, fv, args, result_ty);
    }
    // 2. runtime 符号映射（println/print 等常用符号）。
    if let Some(fv) = resolve_runtime_symbol(fl, callee_symbol) {
        return lower_known_fn(fl, fv, args, result_ty);
    }
    // 3. intrinsic 启发式（按 FQN）。
    if let Some(v) = crate::intrinsics::try_lower_intrinsic_by_fqn(fl, callee_symbol, args, result_ty)? {
        return Ok(v);
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
        let param_ty = if (i as u32) < param_count {
            fv.get_nth_param(i as u32).map(|p| p.get_type())
        } else {
            None
        };
        let val = lower_call_arg(fl, operand, param_ty)?;
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
    fl: &FunctionLowerer<'a, 'ctx>,
    fqn: &str,
) -> Option<inkwell::values::FunctionValue<'ctx>> {
    match fqn {
        "scoop.core.println" | "scoop.core.__scoop_println" => Some(fl.rt.println),
        "scoop.core.print" | "scoop.core.__scoop_print" => Some(fl.rt.print),
        _ => None,
    }
}
