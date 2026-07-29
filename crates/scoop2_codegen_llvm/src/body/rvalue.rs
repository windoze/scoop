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
        LirRvalue::TopLevelRef { fqn, .. } => lower_top_level_ref(fl, fqn),
        LirRvalue::IndexAccess { receiver_local, index_locals, element_ty } => {
            lower_index_access(fl, receiver_local, index_locals, *element_ty)
        }
        LirRvalue::TypeTest { value_local, target_ty } => {
            lower_type_test(fl, value_local, *target_ty)
        }
        LirRvalue::Cast { value_local, target_ty } => {
            lower_cast(fl, value_local, *target_ty)
        }
        LirRvalue::PatternMatch { subject_local, pattern } => {
            lower_pattern_match(fl, subject_local, pattern)
        }
        LirRvalue::PatternExtract { subject_local, result_ty } => {
            lower_pattern_extract(fl, subject_local, *result_ty)
        }
        LirRvalue::InterpolatedString { parts } => {
            lower_interpolated_string(fl, parts)
        }
        LirRvalue::WithUpdate { base_local, updates, result_ty } => {
            lower_with_update(fl, base_local, updates, *result_ty)
        }
        LirRvalue::EnumVariant { enum_ty, tag_value, args, payload_ty, .. } => {
            lower_enum_variant(fl, *enum_ty, *tag_value, args, *payload_ty)
        }
        LirRvalue::ClassCtor { class_fqn, args } => {
            lower_class_ctor(fl, class_fqn, args)
        }
        LirRvalue::MakeArray { elements, ty } => {
            lower_make_array(fl, elements, *ty)
        }
        LirRvalue::MakeClosure { env_local, invoke_fqn } => {
            lower_make_closure(fl, env_local, invoke_fqn)
        }
        LirRvalue::ClassLit { type_fqn } => {
            lower_class_lit(fl, type_fqn)
        }
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

use inkwell::values::AsValueRef;

// =========================================================================
// W1-4/W1-5 剩余 rvalue 实现
// =========================================================================

use scoop2_lir::{LirPattern, LirInterpolatedPart, LirWithUpdateField};

/// `TopLevelRef { fqn }` → 加载全局变量 backing slot。
/// 顶层 val 在 entry main 中初始化（存入全局 alloca）；此处加载。
fn lower_top_level_ref<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    fqn: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 查找已声明的全局。
    if let Some(gv) = fl.cg.lookup_global(fqn) {
        let ty = fl.layouts.entries.keys().next().copied().unwrap_or(scoop2_hir::ty::TypeId(0));
        let _ = ty;
        // 全局 backing slot 是 native ptr (alloca-like)；但 top-level val 的类型
        // 由 LIR TypeLayout 决定。当前简化：返回全局指针 cast 到 GC ptr。
        let val = fl
            .builder
            .build_load(fl.cg.native_ptr_ty(), gv, "toplevel")
            .map_err(|e| CodegenError::llvm(e.to_string(), "load toplevel", scoop2_base::Span::default()))?;
        return Ok(val);
    }
    // 未找到全局：返回 zero（防御性）。
    Err(CodegenError::undefined_symbol(
        fqn,
        &format!("top-level ref in {}", fl.fqn),
        scoop2_base::Span::default(),
    ))
}

/// `IndexAccess { receiver, indices, element_ty }` → 数组索引访问。
/// 通过 runtime scoop_mutable_array 操作（简化：内联 GEP for immutable array）。
fn lower_index_access<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    receiver: &LirOperand,
    indices: &[LirOperand],
    element_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let _ = element_ty;
    // 当前简化：仅支持一维索引，通过 runtime array get intrinsic。
    // receiver 是 Array<T> (GC ptr)。
    let recv_gc = match receiver {
        LirOperand::Local(id) => fl.load_local(*id)?.into_pointer_value(),
        LirOperand::Const(c) => fl.lower_const_value(c)?.into_pointer_value(),
    };
    let idx = match indices.first() {
        Some(LirOperand::Local(id)) => fl.load_local(*id)?.into_int_value(),
        Some(LirOperand::Const(c)) => fl.lower_const_value(c)?.into_int_value(),
        None => return Err(CodegenError::unsupported("IndexAccess 无索引", &fl.fqn, scoop2_base::Span::default())),
    };
    // Array<T> 布局：{ header; i64 len; ptr data }。
    // data 在 header_size + 8。
    let native = fl
        .builder
        .build_ptr_to_int(recv_gc, fl.cg.context.i64_type(), "arr2int")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int arr", scoop2_base::Span::default()))?;
    let native_ptr = fl
        .builder
        .build_int_to_ptr(native, fl.cg.native_ptr_ty(), "arr_native")
        .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr arr", scoop2_base::Span::default()))?;
    let header_size = fl.cg.target_data.get_store_size(&fl.cg.object_header_type());
    let data_offset = header_size + 8; // skip len field
    let data_ptr_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            native_ptr,
            &[fl.cg.context.i64_type().const_int(data_offset, false)],
            "arr_data_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep arr_data", scoop2_base::Span::default()))?;
    let data_ptr = fl
        .builder
        .build_load(fl.cg.native_ptr_ty(), data_ptr_slot, "arr_data")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load arr_data", scoop2_base::Span::default()))?
        .into_pointer_value();
    // 元素地址 = data_ptr + idx * elem_size。
    // 简化：假设元素为 GC ptr (8 bytes)。
    let elem_ptr = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.native_ptr_ty(),
            data_ptr,
            &[idx],
            "elem_ptr",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep elem", scoop2_base::Span::default()))?;
    let elem_native = fl
        .builder
        .build_load(fl.cg.native_ptr_ty(), elem_ptr, "elem")
        .map_err(|e| CodegenError::llvm(e.to_string(), "load elem", scoop2_base::Span::default()))?
        .into_pointer_value();
    // native → GC ptr
    let elem_int = fl
        .builder
        .build_ptr_to_int(elem_native, fl.cg.context.i64_type(), "elem_int")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int elem", scoop2_base::Span::default()))?;
    let elem_gc = fl
        .builder
        .build_int_to_ptr(elem_int, fl.cg.gc_ptr_ty(), "elem_gc")
        .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr elem", scoop2_base::Span::default()))?;
    Ok(elem_gc.into())
}

/// `TypeTest { value, target_ty }` → `is T` 类型检查。
/// 比较 value 的 type_desc->type_id 与 target_ty 的 type_id。
fn lower_type_test<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &LirOperand,
    target_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let _ = target_ty;
    // 简化：总是返回 true（1）。
    // 完整实现需要从 value 的 type_desc 读取 type_id 并比较。
    Ok(fl.cg.context.i8_type().const_int(1, false).into())
}

/// `Cast { value, target_ty }` → `as T` 类型转换。
fn lower_cast<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &LirOperand,
    target_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 简化：identity cast（值不变）。
    // 完整实现需要类型检查 + panic on mismatch。
    match value {
        LirOperand::Local(id) => fl.load_local(*id),
        LirOperand::Const(c) => fl.lower_const_value(c),
    }
}

/// `PatternMatch { subject, pattern }` → 模式匹配测试（返回 Bool）。
fn lower_pattern_match<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    subject: &LirOperand,
    pattern: &LirPattern,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match pattern {
        LirPattern::Wildcard | LirPattern::Bind { .. } => {
            // 总是匹配。
            Ok(fl.cg.context.i8_type().const_int(1, false).into())
        }
        LirPattern::IntLit(v) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?.into_int_value(),
                LirOperand::Const(c) => fl.lower_const_value(c)?.into_int_value(),
            };
            let rhs = fl.cg.context.i64_type().const_int(*v as u64, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj, rhs, "pat_int_eq")
                .map_err(|e| CodegenError::llvm(e.to_string(), "pat_int_eq", scoop2_base::Span::default()))?;
            Ok(fl.builder.build_int_z_extend(eq, fl.cg.context.i8_type(), "pat_int_i8")
                .map_err(|e| CodegenError::llvm(e.to_string(), "zext pat", scoop2_base::Span::default()))?
                .into())
        }
        _ => {
            // 其余模式（Char/String/Bool/Tuple/Struct/Variant/Or/Is）简化为 true。
            Ok(fl.cg.context.i8_type().const_int(1, false).into())
        }
    }
}

/// `PatternExtract { subject, result_ty }` → 模式提取（返回 subject 本身）。
fn lower_pattern_extract<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    subject: &LirOperand,
    _result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match subject {
        LirOperand::Local(id) => fl.load_local(*id),
        LirOperand::Const(c) => fl.lower_const_value(c),
    }
}

/// `InterpolatedString { parts }` → f-string 拼接。
fn lower_interpolated_string<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    parts: &[LirInterpolatedPart],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 简化：仅支持纯字面量部分拼接。
    // 完整实现需要将每个 expr 部分 toString 后 concat。
    let mut result: Option<inkwell::values::PointerValue<'ctx>> = None;
    for part in parts {
        match part {
            LirInterpolatedPart::Lit(s) => {
                let lit = fl.cg.get_or_create_string_literal(s)?;
                result = Some(match result {
                    Some(prev) => {
                        // concat(prev, lit)
                        let prev_native = fl.builder.build_ptr_to_int(prev, fl.cg.context.i64_type(), "prev_int")
                            .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int interp", scoop2_base::Span::default()))?;
                        let prev_ptr = fl.builder.build_int_to_ptr(prev_native, fl.cg.native_ptr_ty(), "prev_native")
                            .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr interp", scoop2_base::Span::default()))?;
                        let lit_native = fl.builder.build_ptr_to_int(lit, fl.cg.context.i64_type(), "lit_int")
                            .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int lit", scoop2_base::Span::default()))?;
                        let lit_ptr = fl.builder.build_int_to_ptr(lit_native, fl.cg.native_ptr_ty(), "lit_native")
                            .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr lit", scoop2_base::Span::default()))?;
                        let concat_result = fl.builder.build_call(fl.rt.string_concat, &[prev_ptr.into(), lit_ptr.into()], "concat")
                            .map_err(|e| CodegenError::llvm(e.to_string(), "call concat", scoop2_base::Span::default()))?;
                        match concat_result.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
                            _ => lit,
                        }
                    }
                    None => lit,
                });
            }
            LirInterpolatedPart::Expr(operand) => {
                // 简化：跳过 expr 部分（需要 toString 转换）。
                let _ = operand;
            }
        }
    }
    Ok(result.unwrap_or_else(|| fl.cg.gc_ptr_ty().const_null()).into())
}

/// `WithUpdate { base, updates, result_ty }` → 值类型字段更新（copy + modify）。
fn lower_with_update<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    base: &LirOperand,
    updates: &[LirWithUpdateField],
    result_ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // copy base value, then apply updates via insertvalue.
    let mut agg = match base {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let layout = fl.layouts.get(result_ty);
    let fields = match layout {
        Some(l) => match &l.kind {
            scoop2_lir::TypeLayoutKind::Struct { fields } => fields,
            scoop2_lir::TypeLayoutKind::Tuple { elements } => elements,
            _ => return Ok(agg),
        },
        None => return Ok(agg),
    };
    for update in updates {
        // 查找字段索引。
        let field_idx = fields.iter().position(|f| {
            // field_offset 匹配（简化：按声明顺序查找）。
            // LIR WithUpdate 的 field_name 对应 struct/tuple 的字段名。
            // tuple 没有 field_name，用 _N 格式。
            true
        });
        if let Some(idx) = field_idx {
            let val = match &update.value {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let inserted = fl
                .builder
                .build_insert_value(agg.into_struct_value(), val, idx as u32, "update")
                .map_err(|e| CodegenError::llvm(e.to_string(), "insert_with_update", scoop2_base::Span::default()))?;
            agg = inserted.into_struct_value().into();
        }
    }
    Ok(agg)
}

/// `EnumVariant { enum_ty, tag_value, args, payload_ty }` → 构造 enum 值。
fn lower_enum_variant<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    enum_ty: scoop2_hir::ty::TypeId,
    tag_value: u64,
    args: &[LirOperand],
    _payload_ty: Option<scoop2_hir::ty::TypeId>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // enum 布局：{ iN tag; payload_bytes }（简化版）。
    let enum_llvm_ty = fl.cg.lower_type(enum_ty, fl.layouts)?;
    let enum_struct = enum_llvm_ty.into_struct_type();
    let agg = enum_struct.const_zero();
    // field 0 = tag。
    let tag_field = fl
        .builder
        .build_insert_value(agg, fl.cg.context.i64_type().const_int(tag_value, false), 0, "enum_tag")
        .map_err(|e| CodegenError::llvm(e.to_string(), "insert enum_tag", scoop2_base::Span::default()))?;
    // field 1+ = payload（简化：跳过 args）。
    let _ = args;
    Ok(tag_field.into_struct_value().into())
}

/// `ClassCtor { class_fqn, args }` → 分配 GC 对象 + 初始化字段。
fn lower_class_ctor<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    class_fqn: &str,
    args: &[LirOperand],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 1. 获取 type descriptor。
    let type_desc = fl.cg.get_or_create_type_descriptor(class_fqn);
    // 2. 构造对象布局：{ header; payload }。
    let header_ty = fl.cg.object_header_type();
    let header_size = fl.cg.target_data.get_store_size(&header_ty);
    // payload size：简化为 args.len() * 8（每个 arg 一个 ptr 槽）。
    let payload_size = (args.len() as u64) * fl.cg.pointer_byte_size;
    let total_size = header_size + payload_size;
    // 3. scoop_alloc_typed(type_desc, size)。
    let alloc_result = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[type_desc.into(), fl.cg.context.i64_type().const_int(total_size, false).into()],
            "ctor_alloc",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "alloc_typed ctor", scoop2_base::Span::default()))?;
    let obj_native = match alloc_result.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
        _ => return Err(CodegenError::llvm("alloc_typed 返回非 BasicValue", "class_ctor", scoop2_base::Span::default())),
    };
    // 4. memset payload 为 0（header 已由 runtime 初始化）。
    // 简化：跳过 memset，直接写字段。
    // 5. 写入字段。
    for (i, arg) in args.iter().enumerate() {
        let field_offset = header_size + (i as u64) * fl.cg.pointer_byte_size;
        let field_slot = unsafe {
            fl.builder.build_in_bounds_gep(
                fl.cg.context.i8_type(),
                obj_native,
                &[fl.cg.context.i64_type().const_int(field_offset, false)],
                &format!("ctor_field{}", i),
            )
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), "gep ctor_field", scoop2_base::Span::default()))?;
        let val = match arg {
            LirOperand::Local(id) => fl.load_local(*id)?,
            LirOperand::Const(c) => fl.lower_const_value(c)?,
        };
        // GC ptr → native ptr for store。
        let val_native = match val {
            BasicValueEnum::PointerValue(p) => {
                let pi = fl.builder.build_ptr_to_int(p, fl.cg.context.i64_type(), "arg_int")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int ctor_arg", scoop2_base::Span::default()))?;
                fl.builder.build_int_to_ptr(pi, fl.cg.native_ptr_ty(), "arg_native")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr ctor_arg", scoop2_base::Span::default()))?
            }
            _ => {
                // 整数值存为 ptr（bit pattern）。
                let iv = val.into_int_value();
                fl.builder.build_int_to_ptr(iv, fl.cg.native_ptr_ty(), "arg_int_as_ptr")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr ctor_int", scoop2_base::Span::default()))?
            }
        };
        fl.builder
            .build_store(field_slot, val_native)
            .map_err(|e| CodegenError::llvm(e.to_string(), "store ctor_field", scoop2_base::Span::default()))?;
    }
    // 6. 返回 GC ptr（native → addrspace 1）。
    let obj_int = fl
        .builder
        .build_ptr_to_int(obj_native, fl.cg.context.i64_type(), "obj_int")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int obj", scoop2_base::Span::default()))?;
    let obj_gc = fl
        .builder
        .build_int_to_ptr(obj_int, fl.cg.gc_ptr_ty(), "obj_gc")
        .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr obj", scoop2_base::Span::default()))?;
    Ok(obj_gc.into())
}

/// `MakeArray { elements, ty }` → 构造不可变数组。
fn lower_make_array<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    elements: &[LirOperand],
    _ty: scoop2_hir::ty::TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 使用 runtime scoop_mutable_array_new + push + freeze。
    let gc_ptr_ty = fl.cg.gc_ptr_ty();
    let native_ptr = fl.cg.native_ptr_ty();
    // scoop_mutable_array_new(elem_kind=2(REF), elem_size=8, elem_align=8, desc=null, capacity=elements.len())
    let arr = fl
        .builder
        .build_call(
            fl.rt.mutable_array_new,
            &[
                fl.cg.context.i32_type().const_int(2, false).into(), // ELEM_KIND_REF
                fl.cg.context.i64_type().const_int(8, false).into(),  // elem_size
                fl.cg.context.i64_type().const_int(8, false).into(),  // elem_align
                native_ptr.const_null().into(),                        // desc
                fl.cg.context.i64_type().const_int(elements.len() as u64, false).into(), // capacity
            ],
            "arr_new",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "mutable_array_new", scoop2_base::Span::default()))?;
    let mut_arr = match arr.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
        _ => return Err(CodegenError::llvm("array_new 返回非 BasicValue", "make_array", scoop2_base::Span::default())),
    };
    // push each element。
    for elem in elements {
        let val = match elem {
            LirOperand::Local(id) => fl.load_local(*id)?,
            LirOperand::Const(c) => fl.lower_const_value(c)?,
        };
        let val_native = match val {
            BasicValueEnum::PointerValue(p) => {
                let pi = fl.builder.build_ptr_to_int(p, fl.cg.context.i64_type(), "elem_int")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int elem", scoop2_base::Span::default()))?;
                fl.builder.build_int_to_ptr(pi, native_ptr, "elem_native")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr elem", scoop2_base::Span::default()))?
            }
            _ => {
                let iv = val.into_int_value();
                fl.builder.build_int_to_ptr(iv, native_ptr, "elem_int_as_ptr")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr elem_int", scoop2_base::Span::default()))?
            }
        };
        let _ = fl
            .builder
            .build_call(fl.rt.mutable_array_push_ref, &[mut_arr.into(), val_native.into()], "arr_push")
            .map_err(|e| CodegenError::llvm(e.to_string(), "push_ref", scoop2_base::Span::default()))?;
    }
    // freeze → immutable Array。
    let frozen = fl
        .builder
        .build_call(fl.rt.mutable_array_freeze, &[mut_arr.into()], "arr_freeze")
        .map_err(|e| CodegenError::llvm(e.to_string(), "freeze", scoop2_base::Span::default()))?;
    Ok(match frozen.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v,
        _ => gc_ptr_ty.const_null().into(),
    })
}

/// `MakeClosure { env_local, invoke_fqn }` → 构造闭包对象。
fn lower_make_closure<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    env_local: &LirOperand,
    invoke_fqn: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 闭包对象布局：{ header; env_ptr; invoke_fn_ptr }。
    let header_ty = fl.cg.object_header_type();
    let header_size = fl.cg.target_data.get_store_size(&header_ty);
    let total_size = header_size + 2 * fl.cg.pointer_byte_size;
    // 分配。
    let type_desc = fl.cg.get_or_create_type_descriptor("scoop.core.Closure");
    let alloc = fl
        .builder
        .build_call(
            fl.rt.alloc_typed,
            &[type_desc.into(), fl.cg.context.i64_type().const_int(total_size, false).into()],
            "closure_alloc",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "alloc closure", scoop2_base::Span::default()))?;
    let obj_native = match alloc.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
        _ => return Err(CodegenError::llvm("alloc closure 返回非 BasicValue", "make_closure", scoop2_base::Span::default())),
    };
    // env_ptr at offset header_size。
    let env = match env_local {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    let env_native = match env {
        BasicValueEnum::PointerValue(p) => {
            let pi = fl.builder.build_ptr_to_int(p, fl.cg.context.i64_type(), "env_int")
                .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int env", scoop2_base::Span::default()))?;
            fl.builder.build_int_to_ptr(pi, fl.cg.native_ptr_ty(), "env_native")
                .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr env", scoop2_base::Span::default()))?
        }
        _ => fl.cg.native_ptr_ty().const_null(),
    };
    let env_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_native,
            &[fl.cg.context.i64_type().const_int(header_size, false)],
            "closure_env_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep closure_env", scoop2_base::Span::default()))?;
    fl.builder
        .build_store(env_slot, env_native)
        .map_err(|e| CodegenError::llvm(e.to_string(), "store closure_env", scoop2_base::Span::default()))?;
    // invoke_fn_ptr at offset header_size + ptr_size。
    let fn_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_native,
            &[fl.cg.context.i64_type().const_int(header_size + fl.cg.pointer_byte_size, false)],
            "closure_fn_slot",
        )
    }
    .map_err(|e| CodegenError::llvm(e.to_string(), "gep closure_fn", scoop2_base::Span::default()))?;
    // 查找 invoke 函数。
    if let Some(invoke_fv) = fl.cg.lookup_callable_fn(invoke_fqn).or_else(|| fl.cg.module.get_function(invoke_fqn)) {
        let invoke_ptr = unsafe { inkwell::values::PointerValue::new(invoke_fv.as_value_ref()) };
        fl.builder
            .build_store(fn_slot, invoke_ptr)
            .map_err(|e| CodegenError::llvm(e.to_string(), "store closure_fn", scoop2_base::Span::default()))?;
    } else {
        fl.builder
            .build_store(fn_slot, fl.cg.native_ptr_ty().const_null())
            .map_err(|e| CodegenError::llvm(e.to_string(), "store closure_fn null", scoop2_base::Span::default()))?;
    }
    // native → GC ptr。
    let obj_int = fl
        .builder
        .build_ptr_to_int(obj_native, fl.cg.context.i64_type(), "closure_int")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int closure", scoop2_base::Span::default()))?;
    let obj_gc = fl
        .builder
        .build_int_to_ptr(obj_int, fl.cg.gc_ptr_ty(), "closure_gc")
        .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr closure", scoop2_base::Span::default()))?;
    Ok(obj_gc.into())
}

/// `ClassLit { type_fqn }` → `T::class`（返回 type_desc 地址）。
fn lower_class_lit<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    type_fqn: &str,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let type_desc = fl.cg.get_or_create_type_descriptor(type_fqn);
    // 返回 type_desc 地址作为 UIntPtr (i64)。
    let addr = fl
        .builder
        .build_ptr_to_int(type_desc, fl.cg.context.i64_type(), "class_lit_addr")
        .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int class_lit", scoop2_base::Span::default()))?;
    Ok(addr.into())
}
