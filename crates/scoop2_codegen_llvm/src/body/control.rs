//! terminator lowering：`LirTerminator` → LLVM terminator 指令。

use inkwell::values::BasicValueEnum;
use scoop2_lir::{LirOperand, LirTerminator};

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 顶层入口：lowering terminator。
pub fn lower_terminator<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    term: &LirTerminator,
) -> CodegenResult<()> {
    match term {
        LirTerminator::Return { value } => {
            // 先 load 返回值（GC local 从 frame slot 读取），再 pop root frame。
            // 若先 pop，frame slot 被清零，GC local 读取得到 NULL。
            let ret_val = match value {
                Some(operand) => {
                    let v = fl.lower_operand(operand, fl.return_ty)?;
                    // Option 表示归一：`return None()` 的静态类型是 Option(Nothing)
                    // （Pointer niche），与声明返回类型 Option<T>（可能 Tagged）表示
                    // 不同，需要显式转换，否则函数签名与返回值类型不一致。
                    let from_ty = match operand {
                        LirOperand::Local(id) => fl.local_types.get(id).copied(),
                        LirOperand::Const(_) => None,
                    };
                    let coerced = match from_ty {
                        Some(ft) if ft != fl.return_ty => {
                            let both_option = [ft, fl.return_ty].iter().all(|t| {
                                matches!(
                                    fl.layouts.get(*t).map(|l| &l.kind),
                                    Some(scoop2_lir::TypeLayoutKind::Option { .. })
                                )
                            });
                            if both_option {
                                super::rvalue::coerce_option_value(fl, v, ft, fl.return_ty)?
                            } else {
                                v
                            }
                        }
                        _ => v,
                    };
                    // 归一到函数返回类型的 LLVM 表示宽度（修复 Float32 函数返回 Float64
                    // 计算结果、或窄整数函数返回宽整数等类型不匹配导致的 LLVM 验证失败）。
                    let coerced = coerce_to_return_type(fl, coerced, fl.return_ty)?;
                    Some(coerced)
                }
                None => None,
            };
            fl.emit_root_frame_pop()?;
            match ret_val {
                Some(v) => {
                    fl.builder.build_return(Some(&v)).map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "build_return",
                            scoop2_base::Span::default(),
                        )
                    })?;
                }
                None => {
                    // 隐式 Unit return。若函数返回类型非 Unit（通常出现在不可达的冗余 block，
                    // 如 MIR 产生的尾部 Return(Unit) 块），返回该类型的零值以满足 LLVM 返回类型约束。
                    let unit_zero = fl.cg.context.i8_type().const_zero();
                    let is_unit = fl.layouts.get(fl.return_ty).is_some_and(|l| {
                        matches!(
                            l.kind,
                            scoop2_lir::TypeLayoutKind::Scalar {
                                scalar_kind: scoop2_lir::ScalarKind::Unit
                            }
                        )
                    });
                    if is_unit {
                        // Unit 返回类型降级为 i8；返回 i8 zero（与函数 i8 返回类型一致）。
                        fl.builder
                            .build_return(Some(&fl.cg.context.i8_type().const_zero()))
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    "build_return(unit)",
                                    scoop2_base::Span::default(),
                                )
                            })?;
                    } else {
                        // 不可达冗余 block：返回零值（实际不会执行到）。
                        let ret_llvm = fl.cg.lower_type(fl.return_ty, fl.layouts)?;
                        let zero = ret_llvm.const_zero();
                        fl.builder.build_return(Some(&zero)).map_err(|e| {
                            CodegenError::llvm(
                                e.to_string(),
                                "build_return(zero fallback)",
                                scoop2_base::Span::default(),
                            )
                        })?;
                    }
                    let _ = unit_zero;
                }
            }
        }
        LirTerminator::Goto { target } => {
            let bb = lookup_block(fl, *target)?;
            fl.builder.build_unconditional_branch(bb).map_err(|e| {
                CodegenError::llvm(e.to_string(), "build_br", scoop2_base::Span::default())
            })?;
        }
        LirTerminator::CondBr {
            cond,
            then_target,
            else_target,
        } => {
            // cond 是 Bool（i8），按其声明类型 load 后转 i1。
            let cond_val = fl.lower_operand(cond, bool_ty())?;
            // 若 cond 是指针（GC ref，effect/continuation 路径可能产生），转 i64 后比较。
            let cond_i = match cond_val {
                BasicValueEnum::IntValue(i) => i,
                BasicValueEnum::PointerValue(p) => fl
                    .builder
                    .build_ptr_to_int(p, fl.cg.context.i64_type(), "cond_ptr2int")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "cond_ptr2int",
                            scoop2_base::Span::default(),
                        )
                    })?,
                _ => fl.cg.context.i8_type().const_zero(),
            };
            // 与同类型的 zero 比较（cond_i 可能是 i8 或 i64）。
            let zero = cond_i.get_type().const_zero();
            let i1 = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::NE, cond_i, zero, "condbr")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "build_icmp", scoop2_base::Span::default())
                })?;
            let then_bb = lookup_block(fl, *then_target)?;
            let else_bb = lookup_block(fl, *else_target)?;
            fl.builder
                .build_conditional_branch(i1, then_bb, else_bb)
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "build_condbr", scoop2_base::Span::default())
                })?;
        }
        LirTerminator::Unreachable => {
            fl.emit_root_frame_pop()?;
            fl.builder.build_unreachable().map_err(|e| {
                CodegenError::llvm(
                    e.to_string(),
                    "build_unreachable",
                    scoop2_base::Span::default(),
                )
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

/// 把返回值归一到函数返回类型的 LLVM 表示宽度。
///
/// 修复 Float32 函数返回 Float64 计算结果（或窄整数返回宽整数）等情形：
/// LLVM 要求 `ret` 指令的操作数类型与函数返回类型严格一致。Scoop 的 expected-type
/// 吸收让 `1.25 + 2.75`（期望 Float32）按 Float64 计算（intrinsic 不按 expected 宽度
/// 选择），导致 ret f64 与声明 f32 不匹配。这里按返回类型把标量截断/扩展。
fn coerce_to_return_type<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    return_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use inkwell::types::BasicTypeEnum;
    let Some(layout) = fl.layouts.get(return_ty) else {
        return Ok(val);
    };
    let target = fl.cg.lower_type(return_ty, fl.layouts)?;
    match (val, target, &layout.kind) {
        // 浮点：f64 → f32 截断（fptrunc）；f32 → f64 扩展（fpext）。
        (BasicValueEnum::FloatValue(fv), BasicTypeEnum::FloatType(dst_ty), _) => {
            if fv.get_type() == dst_ty {
                Ok(fv.into())
            } else {
                // 判断方向：目标是 f32（窄）→ trunc；否则 ext。
                let dst_is_f32 = dst_ty == fl.cg.context.f32_type();
                if dst_is_f32 {
                    fl.builder
                        .build_float_trunc(fv, dst_ty, "ret_fptrunc")
                        .map(|v| v.into())
                        .map_err(|e| {
                            CodegenError::llvm(e.to_string(), "coerce return fptrunc", scoop2_base::Span::default())
                        })
                } else {
                    fl.builder
                        .build_float_ext(fv, dst_ty, "ret_fpext")
                        .map(|v| v.into())
                        .map_err(|e| {
                            CodegenError::llvm(e.to_string(), "coerce return fpext", scoop2_base::Span::default())
                        })
                }
            }
        }
        // 整数：宽度不匹配时截断/扩展。
        (BasicValueEnum::IntValue(iv), BasicTypeEnum::IntType(dst_ty), _) => {
            let sw = iv.get_type().get_bit_width();
            let dw = dst_ty.get_bit_width();
            if sw == dw {
                Ok(iv.into())
            } else if sw > dw {
                fl.builder
                    .build_int_truncate(iv, dst_ty, "ret_trunc")
                    .map(|v| v.into())
                    .map_err(|e| CodegenError::llvm(e.to_string(), "coerce return trunc", scoop2_base::Span::default()))
            } else {
                fl.builder
                    .build_int_z_extend(iv, dst_ty, "ret_zext")
                    .map(|v| v.into())
                    .map_err(|e| CodegenError::llvm(e.to_string(), "coerce return zext", scoop2_base::Span::default()))
            }
        }
        (v, _, _) => Ok(v),
    }
}
