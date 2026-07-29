//! rvalue lowering：`LirRvalue` → LLVM 值。
//!
//! 当前实现覆盖最小子集：Use、Const、Call(Direct→intrinsic/runtime)。
//! 其余 rvalue（MemberAccess/构造/模式/调用分发等）在 W1-5/W1-6 完善，
//! 未覆盖的返回明确错误（绝不静默/panic）。

use inkwell::values::BasicValueEnum;

use scoop2_hir::ty::TypeId;
use scoop2_lir::{LirOperand, LirRvalue};

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 顶层入口：lowering 一个 rvalue，返回其 LLVM 值。`target_ty` 提示结果类型。
pub fn lower_rvalue<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    rv: &LirRvalue,
    target_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match rv {
        LirRvalue::Use(id) => fl.load_local(*id),
        LirRvalue::Const(c) => super::consts::lower_const(fl, c, target_ty),
        LirRvalue::Call(call) => super::call::lower_call(fl, call),
        LirRvalue::MakeTuple { elements, ty } => lower_make_tuple(fl, elements, *ty),
        LirRvalue::TupleIndex { receiver_local, index, element_ty } => {
            lower_tuple_index(fl, receiver_local, *index as u32, *element_ty)
        }
        LirRvalue::StructLit { fields, ty, .. } => lower_struct_lit(fl, fields, *ty),
        LirRvalue::IntEq { lhs_local, rhs_local } => lower_int_eq(fl, lhs_local, rhs_local),
        LirRvalue::MemberAccess {
            receiver_local,
            member_name,
            result_ty,
            ..
        } => lower_member_access(fl, receiver_local, member_name, *result_ty),
        LirRvalue::TopLevelRef { .. } => {
            // 顶层 val/var 引用：W1-2 全局层 + entry 初始化后完善。
            Err(CodegenError::unsupported(
                "TopLevelRef（顶层 val/var）尚未实现",
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
        LirRvalue::IndexAccess { .. }
        | LirRvalue::TypeTest { .. }
        | LirRvalue::Cast { .. }
        | LirRvalue::PatternMatch { .. }
        | LirRvalue::PatternExtract { .. }
        | LirRvalue::InterpolatedString { .. }
        | LirRvalue::WithUpdate { .. }
        | LirRvalue::EnumVariant { .. }
        | LirRvalue::ClassCtor { .. }
        | LirRvalue::MakeArray { .. }
        | LirRvalue::MakeClosure { .. }
        | LirRvalue::ClassLit { .. } => Err(CodegenError::unsupported(
            format!("rvalue 变体尚未实现（{:?}）", discriminant(rv)),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// `StructLit { fields, ty }` → LLVM insertvalue 链。
fn lower_struct_lit<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    fields: &[(String, LirOperand)],
    ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let struct_llvm_ty = fl.cg.lower_type(ty, fl.layouts)?;
    let struct_ty = struct_llvm_ty.into_struct_type();
    let mut agg = struct_ty.const_zero();
    // 字段顺序：按 LIR fields 顺序插入。注意：struct 字段索引需按声明顺序；
    // 当前 LIR StructLit 的 fields 已按声明顺序（与 Struct layout 一致）。
    for (i, (_, operand)) in fields.iter().enumerate() {
        let field_ty = match fl.layouts.get(ty) {
            Some(l) => match &l.kind {
                scoop2_lir::TypeLayoutKind::Struct { fields: fs } => fs.get(i).map(|f| f.ty),
                _ => None,
            },
            None => None,
        }
        .ok_or_else(|| CodegenError::missing_layout(ty.0, "StructLit field ty", scoop2_base::Span::default()))?;
        let val = fl.lower_operand(operand, field_ty)?;
        let inserted = fl
            .builder
            .build_insert_value(agg, val, i as u32, &format!("sf{}", i))
            .map_err(|e| CodegenError::llvm(e.to_string(), "build_insert_value(struct)", scoop2_base::Span::default()))?;
        agg = inserted.into_struct_value();
    }
    Ok(agg.into())
}

/// `IntEq { lhs, rhs }` → LLVM icmp eq（返回 Bool i8）。
fn lower_int_eq<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    lhs: &LirOperand,
    rhs: &LirOperand,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let lhs_val = match lhs {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let rhs_val = match rhs {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let eq = fl
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            lhs_val.into_int_value(),
            rhs_val.into_int_value(),
            "eq",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_icmp eq", scoop2_base::Span::default()))?;
    Ok(fl
        .builder
        .build_int_z_extend(eq, fl.cg.context.i8_type(), "eq_i8")
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_z_ext", scoop2_base::Span::default()))?
        .into())
}

/// `MakeTuple { elements, ty }` → LLVM insertvalue 链。
fn lower_make_tuple<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    elements: &[LirOperand],
    ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let tuple_llvm_ty = fl.cg.lower_type(ty, fl.layouts)?;
    let struct_ty = tuple_llvm_ty.into_struct_type();
    let mut agg = struct_ty.const_zero();
    for (i, operand) in elements.iter().enumerate() {
        // 取第 i 个字段的类型。
        let field_ty = match fl.layouts.get(ty) {
            Some(l) => match &l.kind {
                scoop2_lir::TypeLayoutKind::Tuple { elements: fs } => fs.get(i).map(|f| f.ty),
                _ => None,
            },
            None => None,
        }
        .ok_or_else(|| CodegenError::missing_layout(ty.0, "MakeTuple field ty", scoop2_base::Span::default()))?;
        let val = fl.lower_operand(operand, field_ty)?;
        let inserted = fl
            .builder
            .build_insert_value(agg, val, i as u32, &format!("tup{}", i))
            .map_err(|e| CodegenError::llvm(e.to_string(), "build_insert_value", scoop2_base::Span::default()))?;
        // build_insert_value 返回 AggregateValueEnum；StructValue 可恢复。
        agg = inserted.into_struct_value();
    }
    Ok(agg.into())
}

/// `TupleIndex { receiver, index }` → LLVM extractvalue。
fn lower_tuple_index<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    index: u32,
    element_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // receiver 的 tuple 值（StructValue）。
    let agg = match receiver {
        LirOperand::Local(id) => fl.load_local(*id)?.into_struct_value(),
        LirOperand::Const(c) => fl.lower_const_value(c)?.into_struct_value(),
    };
    let _ = element_ty;
    let v = fl
        .builder
        .build_extract_value(agg, index, &format!("ti{}", index))
        .map_err(|e| CodegenError::llvm(e.to_string(), "build_extract_value", scoop2_base::Span::default()))?;
    Ok(v)
}

/// `MemberAccess { receiver, member }` → struct extractvalue / class GEP+load。
fn lower_member_access<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    member_name: &str,
    result_ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 取 receiver 的值与其类型。
    let (recv_val, recv_ty) = lower_receiver_with_ty(fl, receiver)?;
    let layout = fl.layouts.get(recv_ty).ok_or_else(|| {
        CodegenError::missing_layout(recv_ty.0, "MemberAccess receiver", scoop2_base::Span::default())
    })?;
    // 仅处理 struct 值类型（extractvalue）；class 引用访问需 GEP+load（W1-5 完善）。
    let fields = match &layout.kind {
        scoop2_lir::TypeLayoutKind::Struct { fields } => fields,
        scoop2_lir::TypeLayoutKind::Tuple { elements } => elements,
        _ => {
            return Err(CodegenError::unsupported(
                format!("MemberAccess 仅支持 struct/tuple 值类型，实际布局 {:?}", layout.kind),
                &fl.fqn,
                scoop2_base::Span::default(),
            ))
        }
    };
    let _ = member_name;
    // struct 字段索引：LIR 的 MemberAccess 未直接携带字段索引，需按 member_name 在 fields 中查找。
    // 当前 LIR 字段命名信息不足（FieldLayout 只有 offset/size/ty），无法按名匹配。
    // 这是 LIR 缺口（§0.1-2 的 field_offset/result_ty 问题）的一部分；
    // 这里返回明确错误，待 LIR 补充 member→field_index 后完善。
    let _ = (recv_val, result_ty, fields);
    Err(CodegenError::unsupported(
        "MemberAccess 需要 LIR 提供 member→field_index（当前 LIR 仅有 field_offset，缺字段名映射）",
        &fl.fqn,
        scoop2_base::Span::default(),
    ))
}

/// 取 receiver 的值 + 类型。
fn lower_receiver_with_ty<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
) -> CodegenResult<(BasicValueEnum<'ctx>, TypeId)> {
    match receiver {
        LirOperand::Local(id) => {
            let ty = fl.local_types.get(id).copied().ok_or_else(|| {
                CodegenError::unsupported(
                    format!("receiver local {} 类型未知", id),
                    &fl.fqn,
                    scoop2_base::Span::default(),
                )
            })?;
            Ok((fl.load_local_typed(*id, ty)?, ty))
        }
        LirOperand::Const(_) => Err(CodegenError::unsupported(
            "MemberAccess 的 receiver 不能是 Const",
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
}

/// 取 rvalue 变体名（用于诊断）。
fn discriminant(rv: &LirRvalue) -> &'static str {
    match rv {
        LirRvalue::Use(_) => "Use",
        LirRvalue::Const(_) => "Const",
        LirRvalue::Call(_) => "Call",
        LirRvalue::TopLevelRef { .. } => "TopLevelRef",
        LirRvalue::MemberAccess { .. } => "MemberAccess",
        LirRvalue::TupleIndex { .. } => "TupleIndex",
        LirRvalue::IndexAccess { .. } => "IndexAccess",
        LirRvalue::TypeTest { .. } => "TypeTest",
        LirRvalue::Cast { .. } => "Cast",
        LirRvalue::PatternMatch { .. } => "PatternMatch",
        LirRvalue::PatternExtract { .. } => "PatternExtract",
        LirRvalue::IntEq { .. } => "IntEq",
        LirRvalue::InterpolatedString { .. } => "InterpolatedString",
        LirRvalue::WithUpdate { .. } => "WithUpdate",
        LirRvalue::EnumVariant { .. } => "EnumVariant",
        LirRvalue::ClassCtor { .. } => "ClassCtor",
        LirRvalue::MakeTuple { .. } => "MakeTuple",
        LirRvalue::MakeArray { .. } => "MakeArray",
        LirRvalue::StructLit { .. } => "StructLit",
        LirRvalue::MakeClosure { .. } => "MakeClosure",
        LirRvalue::ClassLit { .. } => "ClassLit",
    }
}
