//! intrinsic lowering：按 callee FQN 启发式映射到内置 LLVM lowering。
//!
//! 注意：完整的 intrinsic 识别应来自 LIR 携带的 intrinsic name（见 NEW-LLVM-CODEGEN.md
//! 「LIR fix: carry intrinsic name」）。在 LIR 透传 intrinsic name 之前，这里按 FQN
//! 启发式处理最常见的内置运算（int_plus 等），保证 codegen 可对核心算术产出正确 IR。
//!
//! 命名约定（与 sysroot `@Intrinsic("name")` 对齐）：
//! - `scoop.core.Int.plus` → `int_plus`，依此类推（owner 类型 + 方法名）。

use inkwell::values::BasicValueEnum;
use inkwell::IntPredicate;

use scoop2_hir::ty::TypeId;
use scoop2_lir::LirOperand;

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 尝试按 FQN lower 一个 intrinsic 调用。命中返回 `Some`，否则 `None`。
pub fn try_lower_intrinsic_by_fqn<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee_fqn: &str,
    args: &[LirOperand],
    _result_ty: TypeId,
) -> CodegenResult<Option<BasicValueEnum<'ctx>>> {
    let Some(name) = intrinsic_name_from_fqn(callee_fqn) else {
        return Ok(None);
    };
    lower_named_intrinsic(fl, &name, args).map(Some)
}

/// 从 Scoop FQN 推导 intrinsic name（启发式）。
/// 例如 `scoop.core.Int.plus` → `int_plus`。
/// 仅处理内建标量类型 + 已知运算符方法。
fn intrinsic_name_from_fqn(fqn: &str) -> Option<String> {
    let last_dot = fqn.rfind('.')?;
    let (owner_path, method) = fqn.split_at(last_dot);
    let method = &method[1..]; // 去掉 '.'
    let owner = owner_path.rsplit('.').next()?;
    let type_prefix = match owner {
        "Int" => "int",
        "UInt" => "uint",
        "Int8" => "int8",
        "Int16" => "int16",
        "Int32" => "int32",
        "Int64" => "int64",
        "UInt8" => "uint8",
        "UInt16" => "uint16",
        "UInt32" => "uint32",
        "UInt64" => "uint64",
        "UIntPtr" => "uint",
        "Bool" => "bool",
        "Char" => "char",
        "Float" | "Float64" => "float",
        "Float32" => "float32",
        _ => return None,
    };
    // 只对内建运算符方法映射。
    let mapped = match method {
        "plus" => "plus",
        "minus" => "minus",
        "times" => "times",
        "div" => "div",
        "rem" => "rem",
        "unaryMinus" => "unary_minus",
        "unaryPlus" => "unary_plus",
        "inc" => "inc",
        "dec" => "dec",
        "and" => "and",
        "or" => "or",
        "xor" => "xor",
        "inv" => "inv",
        "shl" => "shl",
        "shr" => "shr",
        "compareTo" => "compare_to",
        "equals" => "equals",
        "notEquals" => "not_equals",
        "lt" => "lt",
        "le" => "le",
        "gt" => "gt",
        "ge" => "ge",
        "hashCode" | "hash" => "hash",
        "toInt" => "to_int",
        _ => return None,
    };
    Some(format!("{type_prefix}_{mapped}"))
}

/// 按已知 intrinsic name lower。未实现的返回错误。
fn lower_named_intrinsic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    name: &str,
    args: &[LirOperand],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let ctx = fl.cg.context;
    // 整数二元运算（plus/minus/times/div/rem/and/or/xor/shl/shr）。
    if let Some(op) = int_binary_op(name) {
        let lhs = one_arg(fl, args, 0)?;
        let rhs = one_arg(fl, args, 1)?;
        let lhs_i = lhs.into_int_value();
        let rhs_i = rhs.into_int_value();
        let res = match op {
            IntBin::Add => fl.builder.build_int_add(lhs_i, rhs_i, "add"),
            IntBin::Sub => fl.builder.build_int_sub(lhs_i, rhs_i, "sub"),
            IntBin::Mul => fl.builder.build_int_mul(lhs_i, rhs_i, "mul"),
            IntBin::And => fl.builder.build_and(lhs_i, rhs_i, "and"),
            IntBin::Or => fl.builder.build_or(lhs_i, rhs_i, "or"),
            IntBin::Xor => fl.builder.build_xor(lhs_i, rhs_i, "xor"),
            IntBin::Shl => fl.builder.build_left_shift(lhs_i, rhs_i, "shl"),
            IntBin::LShr => fl.builder.build_right_shift(lhs_i, rhs_i, false, "lshr"),
            IntBin::AShr => fl.builder.build_right_shift(lhs_i, rhs_i, true, "ashr"),
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
        return Ok(res.into());
    }
    // 整数除法/取余（需符号性；当前按有符号——完整需按类型符号性）。
    match name {
        n if n.ends_with("_div") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_signed_div(lhs, rhs, "div")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        n if n.ends_with("_rem") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_signed_rem(lhs, rhs, "rem")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        n if n.ends_with("_unary_minus") => {
            let v = one_arg(fl, args, 0)?.into_int_value();
            let r = fl
                .builder
                .build_int_neg(v, "neg")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        n if n.ends_with("_inv") => {
            let v = one_arg(fl, args, 0)?.into_int_value();
            let r = fl
                .builder
                .build_not(v, "not")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        n if n.ends_with("_to_int") => {
            let v = one_arg(fl, args, 0)?;
            let result: BasicValueEnum = match v {
                BasicValueEnum::IntValue(iv) => {
                    let src_width = iv.get_type().get_bit_width();
                    if src_width < 64 {
                        fl.builder
                            .build_int_z_extend(iv, ctx.i64_type(), "to_int_ext")
                            .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?
                            .into()
                    } else {
                        iv.into()
                    }
                }
                BasicValueEnum::FloatValue(fv) => {
                    fl.builder
                        .build_float_to_signed_int(fv, ctx.i64_type(), "f2i")
                        .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?
                        .into()
                }
                _ => return Err(CodegenError::unsupported(
                    format!("to_int 不支持的类型: {name}"),
                    &fl.fqn,
                    scoop2_base::Span::default(),
                )),
            };
            return Ok(result);
        }
        n if n.ends_with("_compare_to") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            // 返回 -1/0/1。
            let lt = fl
                .builder
                .build_int_compare(IntPredicate::SLT, lhs, rhs, "lt")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            let gt = fl
                .builder
                .build_int_compare(IntPredicate::SGT, lhs, rhs, "gt")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            let i64_ty = ctx.i64_type();
            let lt_i = fl
                .builder
                .build_int_z_extend(lt, i64_ty, "lt_i")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            let gt_i = fl
                .builder
                .build_int_z_extend(gt, i64_ty, "gt_i")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            let r = fl
                .builder
                .build_int_sub(gt_i, lt_i, "cmp")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        n if n.ends_with("_equals") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_compare(IntPredicate::EQ, lhs, rhs, "eq")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_not_equals") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_compare(IntPredicate::NE, lhs, rhs, "ne")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_lt") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_compare(IntPredicate::SLT, lhs, rhs, "lt")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_le") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_compare(IntPredicate::SLE, lhs, rhs, "le")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_gt") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_compare(IntPredicate::SGT, lhs, rhs, "gt")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_ge") => {
            let lhs = one_arg(fl, args, 0)?.into_int_value();
            let rhs = one_arg(fl, args, 1)?.into_int_value();
            let r = fl
                .builder
                .build_int_compare(IntPredicate::SGE, lhs, rhs, "ge")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(zext_bool(fl, r, name)?);
        }
        _ => {}
    }
    Err(CodegenError::unknown_intrinsic(
        name,
        &format!("intrinsic lower in {}", fl.fqn),
        scoop2_base::Span::default(),
    ))
}

#[derive(Clone, Copy)]
#[allow(dead_code)] // LShr 预留给 UInt 的逻辑右移（按类型符号性选择，当前默认算术右移）。
enum IntBin {
    Add,
    Sub,
    Mul,
    And,
    Or,
    Xor,
    Shl,
    LShr,
    AShr,
}

fn int_binary_op(name: &str) -> Option<IntBin> {
    match name {
        n if n.ends_with("_unary_minus") || n.ends_with("_unary_plus") => None,
        n if n.ends_with("_plus") => Some(IntBin::Add),
        n if n.ends_with("_minus") => Some(IntBin::Sub),
        n if n.ends_with("_times") => Some(IntBin::Mul),
        n if n.ends_with("_and") => Some(IntBin::And),
        n if n.ends_with("_or") => Some(IntBin::Or),
        n if n.ends_with("_xor") => Some(IntBin::Xor),
        n if n.ends_with("_shl") => Some(IntBin::Shl),
        n if n.ends_with("_shr") => Some(IntBin::AShr), // Int 用算术右移
        _ => None,
    }
}

/// 取第 i 个实参的值（按其本地类型）。
fn one_arg<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    args: &[LirOperand],
    i: usize,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match args.get(i) {
        Some(LirOperand::Local(id)) => fl.load_local(*id),
        Some(LirOperand::Const(c)) => fl.lower_const_value(c),
        None => Err(CodegenError::unsupported(
            format!("intrinsic 实参不足（需第 {} 个）", i),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 把 i1 比较结果 zext 为 Bool（i8）。
fn zext_bool<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    i1: inkwell::values::IntValue<'ctx>,
    name: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    Ok(fl
        .builder
        .build_int_z_extend(i1, fl.cg.context.i8_type(), &format!("{name}_i8"))
        .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?
        .into())
}
