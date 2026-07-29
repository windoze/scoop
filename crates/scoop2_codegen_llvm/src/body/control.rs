//! terminator lowering：`LirTerminator` → LLVM terminator 指令。

use scoop2_lir::LirTerminator;

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 顶层入口：lowering terminator。
pub fn lower_terminator<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    term: &LirTerminator,
) -> CodegenResult<()> {
    match term {
        LirTerminator::Return { value } => {
            // pop root frame（在 ret 之前）。
            fl.emit_root_frame_pop()?;
            match value {
                Some(operand) => {
                    let v = fl.lower_operand(operand, fl.return_ty)?;
                    fl.builder
                        .build_return(Some(&v))
                        .map_err(|e| CodegenError::llvm(e.to_string(), "build_return", scoop2_base::Span::default()))?;
                }
                None => {
                    // 隐式 Unit return。若函数返回类型非 Unit（通常出现在不可达的冗余 block，
                    // 如 MIR 产生的尾部 Return(Unit) 块），返回该类型的零值以满足 LLVM 返回类型约束。
                    let unit_zero = fl.cg.context.i8_type().const_zero();
                    let is_unit = fl
                        .layouts
                        .get(fl.return_ty)
                        .is_some_and(|l| matches!(l.kind, scoop2_lir::TypeLayoutKind::Scalar { scalar_kind: scoop2_lir::ScalarKind::Unit }));
                    if is_unit {
                        fl.builder
                            .build_return(None)
                            .map_err(|e| CodegenError::llvm(e.to_string(), "build_return(unit)", scoop2_base::Span::default()))?;
                    } else {
                        // 不可达冗余 block：返回零值（实际不会执行到）。
                        let ret_llvm = fl.cg.lower_type(fl.return_ty, fl.layouts)?;
                        let zero = ret_llvm.const_zero();
                        fl.builder
                            .build_return(Some(&zero))
                            .map_err(|e| CodegenError::llvm(e.to_string(), "build_return(zero fallback)", scoop2_base::Span::default()))?;
                    }
                    let _ = unit_zero;
                }
            }
        }
        LirTerminator::Goto { target } => {
            let bb = lookup_block(fl, *target)?;
            fl.builder
                .build_unconditional_branch(bb)
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_br", scoop2_base::Span::default()))?;
        }
        LirTerminator::CondBr {
            cond,
            then_target,
            else_target,
        } => {
            // cond 是 Bool（i8），按其声明类型 load 后转 i1。
            let cond_val = fl.lower_operand(cond, bool_ty())?;
            let i1 = fl
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    cond_val.into_int_value(),
                    fl.cg.context.i8_type().const_zero(),
                    "condbr",
                )
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_icmp", scoop2_base::Span::default()))?;
            let then_bb = lookup_block(fl, *then_target)?;
            let else_bb = lookup_block(fl, *else_target)?;
            fl.builder
                .build_conditional_branch(i1, then_bb, else_bb)
                .map_err(|e| CodegenError::llvm(e.to_string(), "build_condbr", scoop2_base::Span::default()))?;
        }
        LirTerminator::Unreachable => {
            fl.emit_root_frame_pop()?;
            fl.builder.build_unreachable().map_err(|e| {
                CodegenError::llvm(e.to_string(), "build_unreachable", scoop2_base::Span::default())
            })?;
        }
    }
    Ok(())
}

fn lookup_block<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    id: u32,
) -> CodegenResult<inkwell::basic_block::BasicBlock<'ctx>> {
    fl.blocks.get(&id).copied().ok_or_else(|| {
        CodegenError::unsupported(
            format!("terminator 引用的 block {} 缺失", id),
            &fl.fqn,
            scoop2_base::Span::default(),
        )
    })
}

/// Bool 的 TypeId 哨兵。LIR 不提供全局 Bool TypeId；
/// `lower_operand` 对 `Const` 直接物化（i8），对 `Local` 走 `load_local`（按声明类型）。
/// 此 TypeId 仅在 operand 是 Const 时被 lower_const 忽略，在 Local 时由 load_local 覆盖。
fn bool_ty() -> scoop2_hir::ty::TypeId {
    scoop2_hir::ty::TypeId(0)
}
