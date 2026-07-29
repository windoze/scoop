//! stmt lowering：`LirStmtKind` → LLVM 指令序列。

use scoop2_lir::LirStmt;

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

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
            let target_ty = fl
                .local_types
                .get(target)
                .copied()
                .ok_or_else(|| CodegenError::unsupported(format!("Assign target local {} 类型未知", target), &fl.fqn, scoop2_base::Span::default()))?;
            let v = super::rvalue::lower_rvalue(fl, value, target_ty)?;
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
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_bit_cast panic", scoop2_base::Span::default()))?;
            let _ = fl
                .builder
                .build_call(fl.rt.panic, &[native.into()], "panic")
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_call panic", scoop2_base::Span::default()))?;
            // panic 不返回；后续由 unreachable 兜底（若 block 还有 terminator）。
            Ok(())
        }
        LirStmtKind::StoreMember { .. }
        | LirStmtKind::StoreTupleIndex { .. }
        | LirStmtKind::StoreGlobal { .. } => {
            // 这些 store 类语句依赖成员偏移/全局布局，在 W1-5（access）完善。
            // 当前给出明确错误（而非静默）。
            Err(CodegenError::unsupported(
                format!("Store 类语句暂未实现（{:?}）", stmt.kind),
                &fl.fqn,
                stmt.span,
            ))
        }
    }
}
