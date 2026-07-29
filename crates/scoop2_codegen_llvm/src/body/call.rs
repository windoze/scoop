//! call lowering：`LirCall` → LLVM 调用指令。
//!
//! 当前覆盖：Direct 调用（解析为已声明的 callable 或 runtime 符号）。
//! Virtual/Interface/Closure/FunValue 在 W1-6 完善。

use inkwell::values::BasicValueEnum;

use scoop2_lir::{LirCall, LirCallKind};

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
        LirCallKind::Virtual { .. }
        | LirCallKind::Interface { .. }
        | LirCallKind::Closure { .. }
        | LirCallKind::FunValue { .. } => Err(CodegenError::unsupported(
            format!("调用种类尚未实现（{:?}）", call.kind),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}
