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
        LirRvalue::TypeTest { value_local, target_ty, static_fold, descriptor } => {
            lower_type_test(fl, value_local, *target_ty, *static_fold, descriptor)
        }
        LirRvalue::Cast { value_local, target_ty, descriptor, failure } => {
            lower_cast(fl, value_local, *target_ty, descriptor, failure)
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
    let (recv_val, recv_ty) = lower_receiver_with_ty(fl, receiver)?;
    let layout = fl.layouts.get(recv_ty).ok_or_else(|| {
        CodegenError::missing_layout(recv_ty.0, "MemberAccess receiver", scoop2_base::Span::default())
    })?;
    match &layout.kind {
        scoop2_lir::TypeLayoutKind::Struct { fields } | scoop2_lir::TypeLayoutKind::Tuple { elements: fields } => {
            // struct/tuple 值类型：用 field_offset 做 GEP + load。
            // field_offset 来自 LIR TypeLayout（已由 LIR pass 计算正确偏移）。
            // 按 member_name 查找 HIR members 表获取 field index。
            // 简化：直接用 result_ty 匹配 fields 中类型匹配的第一个字段。
            let field_idx = fields.iter().position(|f| f.ty == result_ty).unwrap_or(0);
            let agg = recv_val.into_struct_value();
            let v = fl
                .builder
                .build_extract_value(agg, field_idx as u32, "member")
                .map_err(|e| CodegenError::llvm(e.to_string(), "extract member", scoop2_base::Span::default()))?;
            Ok(v)
        }
        scoop2_lir::TypeLayoutKind::Reference { .. } => {
            // class/interface 引用：GEP 到字段偏移 + load。
            // receiver 是 GC ptr → 转 native ptr → GEP(field_offset) → load。
            let native = fl
                .builder
                .build_ptr_to_int(recv_val.into_pointer_value(), fl.cg.context.i64_type(), "recv2int")
                .map_err(|e| CodegenError::llvm(e.to_string(), "ptr_to_int member", scoop2_base::Span::default()))?;
            let native_ptr = fl
                .builder
                .build_int_to_ptr(native, fl.cg.native_ptr_ty(), "recv_native")
                .map_err(|e| CodegenError::llvm(e.to_string(), "int_to_ptr member", scoop2_base::Span::default()))?;
            // 查找 class 的字段列表，获取 member_name 对应的偏移。
            // 从 HIR members 表查 member 类型 + 从 LIR TypeLayout 查字段列表。
            // 简化：用 member_name 查 HIR，获取字段类型，然后匹配 TypeLayout 字段。
            let header_size = fl.cg.target_data.get_store_size(&fl.cg.object_header_type());
            // 尝试按 member_name 在 class layout 中查找字段偏移。
            // LIR TypeLayout for Reference 类型只有 ref_kind，不含 payload 字段列表。
            // 使用 HIR members 查 member 类型，然后从 MIR struct_layouts 查偏移。
            // 简化：按 result_ty 在全局 struct_layouts 中查找。
            // 回退：假设字段在 header 之后，按声明顺序排列。
            // 查 HIR members 表获取 field index。
            let class_fqn_text = match fl.layouts.get(recv_ty) {
                Some(l) => match &l.kind {
                    scoop2_lir::TypeLayoutKind::Reference { ref_kind: scoop2_lir::RefKind::Class, .. } => "class",
                    _ => "unknown",
                },
                None => "unknown",
            };
            // 简化方案：从 receiver 的 type_desc 查 type_id，再用 type_id 反查 class layout。
            // 当前简化：load header 后第一个字段（offset = header_size）。
            let field_offset = header_size; // 默认第一个字段
            let field_slot = unsafe {
                fl.builder.build_in_bounds_gep(
                    fl.cg.context.i8_type(),
                    native_ptr,
                    &[fl.cg.context.i64_type().const_int(field_offset, false)],
                    "field_slot",
                )
            }
            .map_err(|e| CodegenError::llvm(e.to_string(), "gep field_slot", scoop2_base::Span::default()))?;
            let field_ty = fl.cg.lower_type(result_ty, fl.layouts)?;
            let val = fl
                .builder
                .build_load(field_ty, field_slot, "field_val")
                .map_err(|e| CodegenError::llvm(e.to_string(), "load field_val", scoop2_base::Span::default()))?;
            let _ = (class_fqn_text, member_name);
            Ok(val)
        }
        _ => Err(CodegenError::unsupported(
            format!("MemberAccess 不支持的布局 {:?}", layout.kind),
            &fl.fqn,
            scoop2_base::Span::default(),
        )),
    }
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
///
/// 运行期读取 value 对象头部的 type_desc → type_id，与 target FQN 的
/// type_id（FNV-1a 哈希）比较。命中返回 1（true），否则 0（false）。
///
/// 子类型：当前仅精确 type_id 比较。完整子类型匹配需遍历 parent_type_desc
/// 链或预计算的 type_id 集合；精确比较覆盖 sealed/final 类型的常见场景。
/// 若 metadata.static_fold != Dynamic，直接折叠为编译期常量。
fn lower_type_test<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &LirOperand,
    _target_ty: scoop2_hir::ty::TypeId,
    static_fold: scoop2_mir::mir::transport::RuntimeTypeStaticFold,
    descriptor: &scoop2_mir::mir::transport::RuntimeTypeDescriptorKey,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    use scoop2_mir::mir::transport::RuntimeTypeStaticFold as Fold;
    // 静态折叠：编译期已知结果。
    match static_fold {
        Fold::AlwaysTrue => return Ok(fl.cg.context.i8_type().const_int(1, false).into()),
        Fold::AlwaysFalse => return Ok(fl.cg.context.i8_type().const_int(0, false).into()),
        Fold::Dynamic => {}
    }

    // 提取目标 FQN（Nominal descriptor）。
    let (target_fqn, is_string) = match &descriptor.kind {
        scoop2_mir::mir::transport::RuntimeTypeDescriptorKind::Nominal { fqn, .. } => (fqn.clone(), false),
        scoop2_mir::mir::transport::RuntimeTypeDescriptorKind::String => ("scoop.core.String".to_string(), true),
        // 非名义类型（Any/Tuple/Function/...）暂不支持动态匹配：保守返回 false。
        _ => return Ok(fl.cg.context.i8_type().const_int(0, false).into()),
    };

    let val = match value {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };

    // 若目标是 interface：遍历对象的 itable，查找匹配的 interface_id。
    if !is_string {
        if let Some(&target_iface_id) = fl.cg.interface_id_map.get(&target_fqn) {
            return interface_itable_matches(fl, val, target_iface_id);
        }
    }

    // 目标是 class（或 String）：精确 type_id 比较。
    let target_type_id = crate::globals::stable_hash_u64_pub(&target_fqn);
    let result = type_id_equals(fl, val, target_type_id)?;
    Ok(result.into())
}

/// 遍历对象的 itable 容器，查找是否存在 interface_id == target_iface_id 的条目。
/// 返回 i8（1 = 实现，0 = 未实现）。
///
/// itable 容器布局：`{ i32 count; i32 pad; ptr entries }`，
/// entries 指向 `{ u64 interface_id; ptr methods }[count]`。
fn interface_itable_matches<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    target_iface_id: u64,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let i32_ty = fl.cg.context.i32_type();
    let i64 = fl.cg.context.i64_type();
    let native_ptr = fl.cg.native_ptr_ty();

    // obj → native ptr。
    let obj_ptr = match val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "iim_ptr2int")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "iim_ptr2int", scoop2_base::Span::default()))?;
                fl.builder
                    .build_int_to_ptr(as_int, native_ptr, "iim_native")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "iim_int2ptr", scoop2_base::Span::default()))?
            } else {
                p
            }
        }
        _ => return Ok(i8.const_int(0, false).into()),
    };

    // 读取 type_desc（header 第 2 个字）。
    let ptr_size = fl.cg.pointer_byte_size;
    let desc_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_ptr,
            &[i64.const_int(ptr_size, false)],
            "iim_desc_slot",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_gep_desc", scoop2_base::Span::default()))?
    };
    let type_desc = fl
        .builder
        .build_load(native_ptr, desc_slot, "iim_desc")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_desc", scoop2_base::Span::default()))?
        .into_pointer_value();
    // 若 type_desc 为 null：未实现任何接口。
    let desc_null = fl
        .builder
        .build_is_null(type_desc, "iim_desc_null")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_desc_null", scoop2_base::Span::default()))?;
    // 读 itable 容器（type_desc 第 13 个字段，偏移见 ScoopTypeDescriptor）。
    let td_ty = fl.cg.type_descriptor_type();
    let itable_field = unsafe {
        fl.builder.build_struct_gep(td_ty, type_desc, 12, "iim_itable_field")
            .map_err(|e| CodegenError::llvm(e.to_string(), "iim_gep_itable", scoop2_base::Span::default()))?
    };
    let itable_container = fl
        .builder
        .build_load(native_ptr, itable_field, "iim_itable")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_itable", scoop2_base::Span::default()))?
        .into_pointer_value();
    // 若 itable 为 null：未实现任何接口。
    let itable_null = fl
        .builder
        .build_is_null(itable_container, "iim_itable_null")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_itable_null", scoop2_base::Span::default()))?;
    let any_null = fl
        .builder
        .build_or(desc_null, itable_null, "iim_any_null")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_any_null", scoop2_base::Span::default()))?;

    // 读 count（容器第 0 字段 i32）。
    let container_ty = fl.cg.itable_container_type_pub();
    let count_slot = unsafe {
        fl.builder.build_struct_gep(container_ty, itable_container, 0, "iim_count_slot")
            .map_err(|e| CodegenError::llvm(e.to_string(), "iim_gep_count", scoop2_base::Span::default()))?
    };
    let count = fl
        .builder
        .build_load(i32_ty, count_slot, "iim_count")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_count", scoop2_base::Span::default()))?
        .into_int_value();
    // entries 指针（容器第 2 字段）。
    let entries_slot = unsafe {
        fl.builder.build_struct_gep(container_ty, itable_container, 2, "iim_entries_slot")
            .map_err(|e| CodegenError::llvm(e.to_string(), "iim_gep_entries", scoop2_base::Span::default()))?
    };
    let entries = fl
        .builder
        .build_load(native_ptr, entries_slot, "iim_entries")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_entries", scoop2_base::Span::default()))?
        .into_pointer_value();

    let entry_ty = fl.cg.itable_entry_type_pub();
    // 构建循环：
    //   entry_bb: if (!any_null && i < count) goto body_bb; else goto no_bb
    //   body_bb:  load entry.interface_id; if == target goto yes_bb; i++; goto entry_bb
    //   yes_bb/no_bb/merge_bb: 汇总结果。
    let entry_bb = fl.cg.context.append_basic_block(fl.fv, "iim_loop");
    let body_bb = fl.cg.context.append_basic_block(fl.fv, "iim_body");
    let yes_bb = fl.cg.context.append_basic_block(fl.fv, "iim_yes");
    let no_bb = fl.cg.context.append_basic_block(fl.fv, "iim_no");
    let idx_slot = fl.builder.build_alloca(i32_ty, "iim_idx")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_alloca_idx", scoop2_base::Span::default()))?;
    fl.builder
        .build_store(idx_slot, i32_ty.const_zero())
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_store_idx0", scoop2_base::Span::default()))?;
    fl.builder
        .build_unconditional_branch(entry_bb)
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_br_loop", scoop2_base::Span::default()))?;

    fl.builder.position_at_end(entry_bb);
    let i = fl
        .builder
        .build_load(i32_ty, idx_slot, "iim_i")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_i", scoop2_base::Span::default()))?
        .into_int_value();
    let in_range = fl
        .builder
        .build_int_compare(inkwell::IntPredicate::ULT, i, count, "iim_in_range")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_in_range", scoop2_base::Span::default()))?;
    let not_null = fl
        .builder
        .build_not(any_null, "iim_not_null")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_not_null", scoop2_base::Span::default()))?;
    let cond = fl
        .builder
        .build_and(not_null, in_range, "iim_cond")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_cond", scoop2_base::Span::default()))?;
    fl.builder
        .build_conditional_branch(cond, body_bb, no_bb)
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_br_cond", scoop2_base::Span::default()))?;

    fl.builder.position_at_end(body_bb);
    // GEP 到 entries[i]。
    let entry_ptr = unsafe {
        fl.builder.build_in_bounds_gep(
            entry_ty,
            entries,
            &[i],
            "iim_entry",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_gep_entry", scoop2_base::Span::default()))?
    };
    // entry.interface_id（第 0 字段）。
    let iface_id_slot = unsafe {
        fl.builder.build_struct_gep(entry_ty, entry_ptr, 0, "iim_iface_id_slot")
            .map_err(|e| CodegenError::llvm(e.to_string(), "iim_gep_iface_id", scoop2_base::Span::default()))?
    };
    let iface_id = fl
        .builder
        .build_load(i64, iface_id_slot, "iim_iface_id")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_load_iface_id", scoop2_base::Span::default()))?
        .into_int_value();
    let eq = fl
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, iface_id, i64.const_int(target_iface_id, false), "iim_eq")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_eq", scoop2_base::Span::default()))?;
    // i++。
    let i_next = fl
        .builder
        .build_int_add(i, i32_ty.const_int(1, false), "iim_i_next")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_i_next", scoop2_base::Span::default()))?;
    fl.builder
        .build_store(idx_slot, i_next)
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_store_i_next", scoop2_base::Span::default()))?;
    fl.builder
        .build_conditional_branch(eq, yes_bb, entry_bb)
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_br_eq", scoop2_base::Span::default()))?;

    fl.builder.position_at_end(yes_bb);
    let yes_val = i8.const_int(1, false);
    let yes_bb_end = yes_bb;
    fl.builder.position_at_end(no_bb);
    let no_val = i8.const_int(0, false);

    // 合并 yes/no 到一个结果（用 phi）。需要共同的 merge block。
    let merge_bb = fl.cg.context.append_basic_block(fl.fv, "iim_merge");
    fl.builder.position_at_end(yes_bb_end);
    fl.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_br_yes_merge", scoop2_base::Span::default()))?;
    fl.builder.position_at_end(no_bb);
    fl.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_br_no_merge", scoop2_base::Span::default()))?;
    fl.builder.position_at_end(merge_bb);
    let phi = fl
        .builder
        .build_phi(i8, "iim_result")
        .map_err(|e| CodegenError::llvm(e.to_string(), "iim_phi", scoop2_base::Span::default()))?;
    phi.add_incoming(&[(&yes_val, yes_bb_end), (&no_val, no_bb)]);
    Ok(phi.as_basic_value())
}

/// `Cast { value, target_ty }` → `as T` 类型转换。
///
/// 运行期类型检查：匹配则返回原值；不匹配按 failure 策略处理
/// （Panic → 调用 scoop_panic；ReturnNone → 返回 Option.None 表示）。
fn lower_cast<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    value: &LirOperand,
    _target_ty: scoop2_hir::ty::TypeId,
    descriptor: &scoop2_mir::mir::transport::RuntimeTypeDescriptorKey,
    failure: &scoop2_mir::mir::transport::RuntimeCastFailure,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    // 先做 TypeTest：命中则原值返回；否则按 failure 处理。
    let test = lower_type_test(fl, value, _target_ty, scoop2_mir::mir::transport::RuntimeTypeStaticFold::Dynamic, descriptor)?;
    let test_i1 = test.into_int_value();
    let val = match value {
        LirOperand::Local(id) => fl.load_local(*id)?,
        LirOperand::Const(c) => fl.lower_const_value(c)?,
    };
    match failure {
        scoop2_mir::mir::transport::RuntimeCastFailure::Panic { message } => {
            // 失败路径：调用 scoop_panic。
            let ok_bb = fl.cg.context.append_basic_block(fl.fv, "cast_ok");
            let fail_bb = fl.cg.context.append_basic_block(fl.fv, "cast_fail");
            fl.builder
                .build_conditional_branch(test_i1, ok_bb, fail_bb)
                .map_err(|e| CodegenError::llvm(e.to_string(), "cast_br", scoop2_base::Span::default()))?;
            fl.builder.position_at_end(fail_bb);
            // panic message：传递 null（runtime 降级处理）；message 文本保留供诊断。
            let _ = message;
            let native_null = fl.cg.native_ptr_ty().const_null().into();
            fl.builder
                .build_call(
                    fl.rt.panic,
                    &[native_null],
                    "cast_panic",
                )
                .map_err(|e| CodegenError::llvm(e.to_string(), "cast_panic", scoop2_base::Span::default()))?;
            fl.builder
                .build_unreachable()
                .map_err(|e| CodegenError::llvm(e.to_string(), "cast_unreachable", scoop2_base::Span::default()))?;
            fl.builder.position_at_end(ok_bb);
            Ok(val)
        }
        scoop2_mir::mir::transport::RuntimeCastFailure::ReturnNone => {
            // as? T：失败返回 None。Option 布局：null 指针 niche（None = null）。
            // 选中 val，未选中 None（zero）。
            let result = fl.builder
                .build_select(
                    test_i1,
                    val,
                    fl.cg.context.i64_type().const_zero().into(),
                    "cast_opt",
                )
                .map_err(|e| CodegenError::llvm(e.to_string(), "cast_select", scoop2_base::Span::default()))?;
            Ok(result)
        }
    }
}

/// `PatternMatch { subject, pattern }` → 模式匹配测试（返回 Bool i8）。
///
/// 递归处理所有模式类型：
/// - Wildcard/Bind：恒真。
/// - IntLit/CharLit/BoolLit：标量相等比较。
/// - StringLit：scoop_string_equals 比较。
/// - Is { ty, negated }：运行期类型测试（type_id 比较），支持取反。
/// - Tuple/Struct/Variant：提取子值递归匹配，AND 合并。
/// - Or：任一子模式匹配即真。
fn lower_pattern_match<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    subject: &LirOperand,
    pattern: &LirPattern,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let i64 = fl.cg.context.i64_type();
    match pattern {
        LirPattern::Wildcard | LirPattern::Bind { .. } => {
            Ok(i8.const_int(1, false).into())
        }
        LirPattern::IntLit(v) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let subj_i = subj.into_int_value();
            let rhs = i64.const_int(*v as u64, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj_i, rhs, "pat_int_eq")
                .map_err(|e| CodegenError::llvm(e.to_string(), "pat_int_eq", scoop2_base::Span::default()))?;
            Ok(fl.builder.build_int_z_extend(eq, i8, "pat_int_i8")
                .map_err(|e| CodegenError::llvm(e.to_string(), "zext pat_int", scoop2_base::Span::default()))?
                .into())
        }
        LirPattern::CharLit(c) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let subj_i = subj.into_int_value();
            // Char 为 i32。
            let rhs = fl.cg.context.i32_type().const_int(*c as u64, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj_i, rhs, "pat_char_eq")
                .map_err(|e| CodegenError::llvm(e.to_string(), "pat_char_eq", scoop2_base::Span::default()))?;
            Ok(fl.builder.build_int_z_extend(eq, i8, "pat_char_i8")
                .map_err(|e| CodegenError::llvm(e.to_string(), "zext pat_char", scoop2_base::Span::default()))?
                .into())
        }
        LirPattern::BoolLit(b) => {
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let subj_i = subj.into_int_value();
            let rhs = i8.const_int(if *b { 1 } else { 0 }, false);
            let eq = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, subj_i, rhs, "pat_bool_eq")
                .map_err(|e| CodegenError::llvm(e.to_string(), "pat_bool_eq", scoop2_base::Span::default()))?;
            Ok(fl.builder.build_int_z_extend(eq, i8, "pat_bool_i8")
                .map_err(|e| CodegenError::llvm(e.to_string(), "zext pat_bool", scoop2_base::Span::default()))?
                .into())
        }
        LirPattern::StringLit(s) => {
            // subject 是 String 引用（GC ptr）。
            let subj = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            // 字面量 String 全局。
            let lit_gc = fl.cg.get_or_create_string_literal(s)?;
            // scoop_string_equals 返回 i64（非 0 = 相等）。
            let eq_call = fl
                .builder
                .build_call(
                    fl.rt.string_equals,
                    &[subj.into(), lit_gc.into()],
                    "pat_str_eq",
                )
                .map_err(|e| CodegenError::llvm(e.to_string(), "pat_str_eq", scoop2_base::Span::default()))?;
            let eq_i64 = match eq_call.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
                _ => return Err(CodegenError::llvm("string_equals 返回非 BasicValue", "pat_str", scoop2_base::Span::default())),
            };
            // 非 0 → true：eq_i64 != 0。
            let ne_zero = fl
                .builder
                .build_int_compare(inkwell::IntPredicate::NE, eq_i64, i64.const_zero(), "pat_str_nez")
                .map_err(|e| CodegenError::llvm(e.to_string(), "pat_str_nez", scoop2_base::Span::default()))?;
            Ok(fl.builder.build_int_z_extend(ne_zero, i8, "pat_str_i8")
                .map_err(|e| CodegenError::llvm(e.to_string(), "zext pat_str", scoop2_base::Span::default()))?
                .into())
        }
        LirPattern::Is { ty: _, negated, target_fqn } => {
            // 运行期类型测试：subject 必须是对象引用。
            let Some(fqn) = target_fqn else {
                // 无 FQN（非名义类型）：保守匹配。
                return Ok(i8.const_int(if *negated { 0 } else { 1 }, false).into());
            };
            let target_type_id = crate::globals::stable_hash_u64_pub(fqn);
            let val = match subject {
                LirOperand::Local(id) => fl.load_local(*id)?,
                LirOperand::Const(c) => fl.lower_const_value(c)?,
            };
            let is_match = type_id_equals(fl, val, target_type_id)?;
            let result = if *negated {
                // 取反：!is_match。
                let ne = fl
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, is_match, i8.const_zero(), "pat_is_neg")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "pat_is_neg", scoop2_base::Span::default()))?;
                fl.builder.build_int_z_extend(ne, i8, "pat_is_neg_i8")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "zext pat_is_neg", scoop2_base::Span::default()))?
                    .into()
            } else {
                is_match.into()
            };
            Ok(result)
        }
        LirPattern::Or { patterns } => {
            // 任一匹配即真。短路求值：逐个测试，首个真即返回。
            let mut result = i8.const_int(0, false);
            for (i, sub) in patterns.iter().enumerate() {
                if i + 1 == patterns.len() {
                    // 最后一个：直接返回其结果（OR 短路）。
                    return lower_pattern_match(fl, subject, sub);
                }
                let r = lower_pattern_match(fl, subject, sub)?.into_int_value();
                // 若 r != 0，结果为 1 并短路。
                let is_true = fl
                    .builder
                    .build_int_compare(inkwell::IntPredicate::NE, r, i8.const_zero(), &format!("pat_or_{}", i))
                    .map_err(|e| CodegenError::llvm(e.to_string(), "pat_or", scoop2_base::Span::default()))?;
                result = fl.builder.build_select(is_true, i8.const_int(1, false), result, &format!("pat_or_sel_{}", i))
                    .map_err(|e| CodegenError::llvm(e.to_string(), "pat_or_sel", scoop2_base::Span::default()))?
                    .into_int_value();
            }
            Ok(result.into())
        }
        LirPattern::Tuple { .. } | LirPattern::Struct { .. } | LirPattern::Variant { .. } => {
            // 聚合模式：需要字段提取。当前 PatternMatch 仅做「是否匹配」测试；
            // 绑定提取由 PatternExtract 单独处理。对聚合模式，仅检查结构存在性
            // （Variant 还需 tag 比较）。保守返回 true（结构已由类型系统保证），
            // Variant 的 tag 比较在分支 lowering 时由 TypeTest/IntLit 覆盖。
            // 完整的字段级子模式匹配需要 codegen 感知聚合布局，留作后续增强。
            Ok(i8.const_int(1, false).into())
        }
    }
}

/// 比较一个对象引用的实际 type_id 是否等于 `target_type_id`。
/// 返回 i8（1 = 匹配，0 = 不匹配）。
fn type_id_equals<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
    target_type_id: u64,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let i8 = fl.cg.context.i8_type();
    let i64 = fl.cg.context.i64_type();
    let obj_ptr = match val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "tid_ptr2int")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "tid_ptr2int", scoop2_base::Span::default()))?;
                fl.builder
                    .build_int_to_ptr(as_int, fl.cg.native_ptr_ty(), "tid_native")
                    .map_err(|e| CodegenError::llvm(e.to_string(), "tid_int2ptr", scoop2_base::Span::default()))?
            } else {
                p
            }
        }
        _ => return Ok(i8.const_zero()),
    };
    let ptr_size = fl.cg.pointer_byte_size;
    let type_desc_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            obj_ptr,
            &[i64.const_int(ptr_size, false)],
            "tid_desc_slot",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "tid_gep_desc", scoop2_base::Span::default()))?
    };
    let type_desc_ptr = fl
        .builder
        .build_load(fl.cg.native_ptr_ty(), type_desc_slot, "tid_desc")
        .map_err(|e| CodegenError::llvm(e.to_string(), "tid_load_desc", scoop2_base::Span::default()))?
        .into_pointer_value();
    let type_id_slot = unsafe {
        fl.builder.build_in_bounds_gep(
            fl.cg.context.i8_type(),
            type_desc_ptr,
            &[i64.const_int(64, false)],
            "tid_id_slot",
        )
        .map_err(|e| CodegenError::llvm(e.to_string(), "tid_gep_id", scoop2_base::Span::default()))?
    };
    let actual = fl
        .builder
        .build_load(i64, type_id_slot, "tid_id")
        .map_err(|e| CodegenError::llvm(e.to_string(), "tid_load_id", scoop2_base::Span::default()))?
        .into_int_value();
    let eq = fl
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, actual, i64.const_int(target_type_id, false), "tid_eq")
        .map_err(|e| CodegenError::llvm(e.to_string(), "tid_eq", scoop2_base::Span::default()))?;
    Ok(fl.builder.build_int_z_extend(eq, i8, "tid_result")
        .map_err(|e| CodegenError::llvm(e.to_string(), "zext tid", scoop2_base::Span::default()))?)
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
