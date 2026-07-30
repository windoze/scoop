//! intrinsic lowering：按 callee FQN 启发式映射到内置 LLVM lowering。
//!
//! 注意：完整的 intrinsic 识别应来自 LIR 携带的 intrinsic name（见 NEW-LLVM-CODEGEN.md
//! 「LIR fix: carry intrinsic name」）。在 LIR 透传 intrinsic name 之前，这里按 FQN
//! 启发式处理最常见的内置运算（int_plus 等），保证 codegen 可对核心算术产出正确 IR。
//!
//! 命名约定（与 sysroot `@Intrinsic("name")` 对齐）：
//! - `scoop.core.Int.plus` → `int_plus`，依此类推（owner 类型 + 方法名）。

use inkwell::values::BasicValueEnum;
use inkwell::{FloatPredicate, IntPredicate};

use scoop2_hir::ty::TypeId;
use scoop2_lir::LirOperand;

use crate::body::FunctionLowerer;
use crate::error::{CodegenError, CodegenResult};

/// 尝试按 FQN lower 一个 intrinsic 调用。命中返回 `Some`，否则 `None`。
pub fn try_lower_intrinsic_by_fqn<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee_fqn: &str,
    args: &[LirOperand],
    result_ty: TypeId,
) -> CodegenResult<Option<BasicValueEnum<'ctx>>> {
    // Array / MutableArray 方法：直接按 FQN 分派到数组 intrinsic。
    if let Some(v) = try_lower_array_intrinsic(fl, callee_fqn, args, result_ty)? {
        return Ok(Some(v));
    }
    // String 字节级 substrate：byteLength / getByte（@Intrinsic("string_byte_length"/"string_get_byte")）。
    if let Some(v) = try_lower_string_intrinsic(fl, callee_fqn, args)? {
        return Ok(Some(v));
    }
    // 用 `@Intrinsic("name")` 注解透传的真实 intrinsic 名（唯一权威来源）。
    // intrinsic_map 从 LirDeclaration.intrinsic_name 构建，覆盖所有 @Intrinsic 方法。
    let name = match fl.cg.lookup_intrinsic_name(callee_fqn) {
        Some(ann_name) => ann_name.to_string(),
        None => return Ok(None),
    };
    // 重载歧义修正：同名方法（如 `Char.minus`）有多个 @Intrinsic 注解
    // （char_minus_int / char_minus_char），intrinsic_map 只保留其一。
    // 按 result_ty / 实参类型精确选名，避免把 `Char - Int` 当成 `Char - Char`。
    let name = disambiguate_overloaded_intrinsic(fl, callee_fqn, &name, args, result_ty);
    lower_named_intrinsic(fl, &name, args).map(Some)
}

/// 修正同名重载 intrinsic 的注解名歧义。
///
/// `intrinsic_map` 以 FQN 为键，重载方法（不同参数类型、不同 @Intrinsic 名）只保留
/// 最后注册的一个。这里按调用点的 result_ty / 实参类型重新精确选名。
fn disambiguate_overloaded_intrinsic(
    fl: &FunctionLowerer,
    callee_fqn: &str,
    name: &str,
    args: &[LirOperand],
    result_ty: TypeId,
) -> String {
    // Char.minus：result_ty=Char → char_minus_int；result_ty=Int → char_minus_char。
    if callee_fqn.ends_with(".Char.minus") && (name == "char_minus_int" || name == "char_minus_char")
    {
        let is_char_result = fl
            .layouts
            .get(result_ty)
            .map(|l| {
                matches!(
                    &l.kind,
                    scoop2_lir::TypeLayoutKind::Scalar {
                        scalar_kind: scoop2_lir::ScalarKind::Char,
                        ..
                    }
                )
            })
            .unwrap_or(false);
        return if is_char_result {
            "char_minus_int".to_string()
        } else {
            "char_minus_char".to_string()
        };
    }
    // 同理 Char.plus 只有 char_plus_int（无歧义），略。
    let _ = args;
    name.to_string()
}

/// String 字节级 substrate intrinsic。
///
/// - `String.byteLength()` → `scoop_string_byte_length(this)`（i64）。
/// - `String.getByte(i)` → `scoop_string_bytes(this)` 得字节数据指针，GEP i 后 load i8 → zext i64。
fn try_lower_string_intrinsic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee_fqn: &str,
    args: &[LirOperand],
) -> CodegenResult<Option<BasicValueEnum<'ctx>>> {
    let method = match callee_fqn.strip_prefix("scoop.core.String.") {
        Some(m) => m,
        None => return Ok(None),
    };
    if !matches!(method, "byteLength" | "getByte" | "equals" | "notEquals") {
        return Ok(None);
    }
    // equals / notEquals：`scoop_string_equals(a, b)` → i64（1=相等），按运算符映射 Bool(i8)。
    // （String 未声明 equals/notEquals 方法；`==`/`!=` 由 typecheck 记为运算符方法解析到这里。）
    if method == "equals" || method == "notEquals" {
        let lhs = one_arg(fl, args, 0)?;
        let rhs = one_arg(fl, args, 1)?;
        let lhs_gc = native_to_gc_if_ptr(fl, lhs)?;
        let rhs_gc = native_to_gc_if_ptr(fl, rhs)?;
        let call = fl
            .builder
            .build_call(
                fl.rt.string_equals,
                &[lhs_gc.into(), rhs_gc.into()],
                "str_eq",
            )
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "str_eq", scoop2_base::Span::default())
            })?;
        let eq_i64 = match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => {
                crate::body::expect_int_val(v, "string_equals 返回值", &fl.fqn)?
            }
            inkwell::values::ValueKind::Instruction(_) => {
                return Err(CodegenError::llvm(
                    "scoop_string_equals 未返回值",
                    "equals",
                    scoop2_base::Span::default(),
                ));
            }
        };
        let zero = fl.cg.context.i64_type().const_zero();
        let pred = if method == "equals" {
            IntPredicate::NE // 非 0 = 相等
        } else {
            IntPredicate::EQ
        };
        let cmp = fl
            .builder
            .build_int_compare(pred, eq_i64, zero, "str_eq_i1")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "str_eq_i1", scoop2_base::Span::default())
            })?;
        return zext_bool(fl, cmp, "str_eq").map(Some);
    }
    let i64 = fl.cg.context.i64_type();
    let i8 = fl.cg.context.i8_type();
    let receiver = match args.first() {
        Some(LirOperand::Local(id)) => fl.load_local(*id)?,
        Some(LirOperand::Const(c)) => fl.lower_const_value(c)?,
        None => return Ok(None),
    };
    // receiver 是 GC ptr（String 引用）；runtime 的 extern(abi="scoop") 接受 GC ptr。
    let recv_gc = match receiver {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                p
            } else {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "str_recv_int")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "str_recv_int",
                            scoop2_base::Span::default(),
                        )
                    })?;
                fl.builder
                    .build_int_to_ptr(as_int, fl.cg.gc_ptr_ty(), "str_recv_gc")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "str_recv_gc",
                            scoop2_base::Span::default(),
                        )
                    })?
            }
        }
        _ => {
            return Err(CodegenError::llvm(
                "String receiver must be a pointer",
                "try_lower_string_intrinsic",
                scoop2_base::Span::default(),
            ));
        }
    };
    match method {
        "byteLength" => {
            let call = fl
                .builder
                .build_call(fl.rt.string_byte_length, &[recv_gc.into()], "str_byte_len")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "str_byte_len", scoop2_base::Span::default())
                })?;
            match call.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
                inkwell::values::ValueKind::Instruction(_) => Err(CodegenError::llvm(
                    "scoop_string_byte_length 未返回值",
                    "byteLength",
                    scoop2_base::Span::default(),
                )),
            }
        }
        _ => {
            // getByte：bytes = scoop_string_bytes(this)（native const uint8_t*）。
            let index = match args.get(1) {
                Some(LirOperand::Local(id)) => fl.load_local(*id)?,
                Some(LirOperand::Const(c)) => fl.lower_const_value(c)?,
                None => return Ok(None),
            };
            let idx_i = crate::intrinsics::zext_to_i64(
                fl,
                crate::body::expect_int_val(index, "getByte 索引", &fl.fqn)?,
            );
            let call = fl
                .builder
                .build_call(fl.rt.string_bytes, &[recv_gc.into()], "str_bytes")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "str_bytes", scoop2_base::Span::default())
                })?;
            let bytes = match call.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => v,
                inkwell::values::ValueKind::Instruction(_) => {
                    return Err(CodegenError::llvm(
                        "scoop_string_bytes 未返回值",
                        "getByte",
                        scoop2_base::Span::default(),
                    ));
                }
            };
            let bytes_native = gc_to_native(fl, bytes)?;
            let slot = unsafe {
                fl.builder
                    .build_in_bounds_gep(i8, bytes_native, &[idx_i], "str_byte_slot")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "str_byte_slot",
                            scoop2_base::Span::default(),
                        )
                    })?
            };
            let byte = fl
                .builder
                .build_load(i8, slot, "str_byte")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "str_byte", scoop2_base::Span::default())
                })?
                .into_int_value();
            let widened = fl
                .builder
                .build_int_z_extend(byte, i64, "str_byte_i64")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "str_byte_i64", scoop2_base::Span::default())
                })?;
            Ok(Some(widened.into()))
        }
    }
}

/// Array / MutableArray intrinsic：size / get / set / __dataPtr。
///
/// 布局（runtime/c/scoop_array_internal.h；按 owner FQN 分派）：
/// - `Array<T>`（ScoopArray，内联 data）：
///   header(32) | len@32 | elem_size_bytes@40 | data_offset_bytes@48 | ...
///   元素地址 = arr + data_offset_bytes + idx * elem_size_bytes。
/// - `MutableArray<T>`（ScoopMutableArray，外置 data 指针）：
///   header(32) | len@32 | cap@40 | elem_size_bytes@48 | elem_align@56 | elem_desc@64
///   | data(ptr)@72 | ...
///   元素地址 = data + idx * elem_size_bytes。
/// get/set 做边界检查：越界（含负 index）调用 scoop_panic。
fn try_lower_array_intrinsic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    callee_fqn: &str,
    args: &[LirOperand],
    result_ty: TypeId,
) -> CodegenResult<Option<BasicValueEnum<'ctx>>> {
    let i64 = fl.cg.context.i64_type();
    let i8 = fl.cg.context.i8_type();
    // 仅处理 Array / MutableArray 的已知方法。
    let is_mutable_owner = callee_fqn.starts_with("scoop.core.MutableArray.");
    let is_array_owner = callee_fqn.starts_with("scoop.core.Array.") || is_mutable_owner;
    if !is_array_owner {
        return Ok(None);
    }
    let method = callee_fqn.rsplit('.').next().unwrap_or("");
    // 第一个参数是 receiver（this），已由 member_call prepend。
    let receiver = match args.get(0) {
        Some(LirOperand::Local(id)) => fl.load_local(*id)?,
        Some(LirOperand::Const(c)) => fl.lower_const_value(c)?,
        None => return Ok(None),
    };
    // receiver 是 GC ptr（Array 引用）→ native ptr。
    let arr_native = gc_to_native(fl, receiver)?;
    let header_size = fl
        .cg
        .target_data
        .get_store_size(&fl.cg.object_header_type());
    let load_u64_at = |byte_off: u64,
                       name: &str|
     -> CodegenResult<inkwell::values::IntValue<'ctx>> {
        let slot = unsafe {
            fl.builder
                .build_in_bounds_gep(i8, arr_native, &[i64.const_int(byte_off, false)], name)
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "arr_gep_meta", scoop2_base::Span::default())
                })?
        };
        Ok(fl
            .builder
            .build_load(i64, slot, name)
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "arr_load_meta", scoop2_base::Span::default())
            })?
            .into_int_value())
    };
    // len 在两种布局里都在 header + 0。
    let len_val = load_u64_at(header_size, "arr_len")?;
    // (data_ptr, elem_size)：按布局分派。
    let (data_ptr, elem_size) = if is_mutable_owner {
        // MutableArray：elem_size_bytes @ header+16，data 外置指针 @ header+40。
        let esz = load_u64_at(header_size + 16, "arr_esz")?;
        let data_slot = unsafe {
            fl.builder
                .build_in_bounds_gep(
                    i8,
                    arr_native,
                    &[i64.const_int(header_size + 40, false)],
                    "arr_data_slot",
                )
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "arr_gep_data", scoop2_base::Span::default())
                })?
        };
        let data = fl
            .builder
            .build_load(fl.cg.native_ptr_ty(), data_slot, "arr_data")
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), "arr_load_data", scoop2_base::Span::default())
            })?
            .into_pointer_value();
        (data, esz)
    } else {
        // Array：elem_size_bytes @ header+8，data_offset_bytes @ header+16。
        let esz = load_u64_at(header_size + 8, "arr_esz")?;
        let doff = load_u64_at(header_size + 16, "arr_doff")?;
        let data = unsafe {
            fl.builder
                .build_in_bounds_gep(i8, arr_native, &[doff], "arr_data")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "arr_gep_data", scoop2_base::Span::default())
                })?
        };
        (data, esz)
    };

    match method {
        "size" => {
            return Ok(Some(len_val.into()));
        }
        "get" => {
            // args[1] = index。
            let index = match args.get(1) {
                Some(LirOperand::Local(id)) => {
                    crate::body::expect_int_val(fl.load_local(*id)?, "intrinsic 整型参数", &fl.fqn)?
                }
                Some(LirOperand::Const(c)) => crate::body::expect_int_val(
                    fl.lower_const_value(c)?,
                    "intrinsic 整型参数",
                    &fl.fqn,
                )?,
                None => return Ok(None),
            };
            let index = crate::body::rvalue::normalize_int_to_i64(fl, index, "arr_get_idx")?;
            // 边界检查：越界（含负 index）panic。
            crate::body::rvalue::build_array_bounds_check(fl, index, len_val)?;
            // elem_offset = index * elem_size
            let elem_off = fl
                .builder
                .build_int_mul(index, elem_size, "arr_elem_off")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "arr_elem_off", scoop2_base::Span::default())
                })?;
            let elem_ptr = unsafe {
                fl.builder
                    .build_in_bounds_gep(i8, data_ptr, &[elem_off], "arr_elem_ptr")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "arr_gep_elem",
                            scoop2_base::Span::default(),
                        )
                    })?
            };
            // 元素加载类型由 result_ty 的布局决定（编译期已知）。
            // result_ty 可能是 Int（值类型，WORD 元素）或 String/Any（引用类型，REF 元素）。
            let result_layout = fl.layouts.get(result_ty);
            let elem_is_ref = result_layout
                .map(|l| {
                    matches!(
                        &l.kind,
                        scoop2_lir::TypeLayoutKind::Reference {
                            gc_traceable: true,
                            ..
                        }
                    )
                })
                .unwrap_or(false);
            if elem_is_ref {
                // REF 元素：load native ptr → GC ptr。
                let raw = fl
                    .builder
                    .build_load(fl.cg.native_ptr_ty(), elem_ptr, "arr_elem_ptr")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "arr_load_elem_ptr",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into_pointer_value();
                let gc = fl
                    .builder
                    .build_ptr_to_int(raw, i64, "arr_ref_int")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "arr_ref_int",
                            scoop2_base::Span::default(),
                        )
                    })?;
                let gc_ptr = fl
                    .builder
                    .build_int_to_ptr(gc, fl.cg.gc_ptr_ty(), "arr_elem_gc")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "arr_elem_gc",
                            scoop2_base::Span::default(),
                        )
                    })?;
                return Ok(Some(gc_ptr.into()));
            } else {
                // WORD 元素：load i64。
                let raw = fl
                    .builder
                    .build_load(i64, elem_ptr, "arr_elem_raw")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "arr_load_elem_raw",
                            scoop2_base::Span::default(),
                        )
                    })?
                    .into_int_value();
                return Ok(Some(raw.into()));
            }
        }
        "set" => {
            // args[1] = index, args[2] = value。
            let index = match args.get(1) {
                Some(LirOperand::Local(id)) => {
                    crate::body::expect_int_val(fl.load_local(*id)?, "intrinsic 整型参数", &fl.fqn)?
                }
                Some(LirOperand::Const(c)) => crate::body::expect_int_val(
                    fl.lower_const_value(c)?,
                    "intrinsic 整型参数",
                    &fl.fqn,
                )?,
                None => return Ok(None),
            };
            let value = match args.get(2) {
                Some(LirOperand::Local(id)) => fl.load_local(*id)?,
                Some(LirOperand::Const(c)) => fl.lower_const_value(c)?,
                None => return Ok(None),
            };
            let index = crate::body::rvalue::normalize_int_to_i64(fl, index, "arr_set_idx")?;
            // 边界检查：越界（含负 index）panic。
            crate::body::rvalue::build_array_bounds_check(fl, index, len_val)?;
            let elem_off = fl
                .builder
                .build_int_mul(index, elem_size, "arr_set_off")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "arr_set_off", scoop2_base::Span::default())
                })?;
            let elem_ptr = unsafe {
                fl.builder
                    .build_in_bounds_gep(i8, data_ptr, &[elem_off], "arr_set_ptr")
                    .map_err(|e| {
                        CodegenError::llvm(
                            e.to_string(),
                            "arr_set_gep_elem",
                            scoop2_base::Span::default(),
                        )
                    })?
            };
            // value 若为 GC ptr → native ptr for store。
            let val_native = gc_to_native_basic(fl, value)?;
            fl.builder.build_store(elem_ptr, val_native).map_err(|e| {
                CodegenError::llvm(e.to_string(), "arr_store", scoop2_base::Span::default())
            })?;
            // 返回 Unit（i8 zero）。
            return Ok(Some(i8.const_zero().into()));
        }
        "__dataPtr" => {
            // 返回 data_ptr 作为 UIntPtr（native ptr → i64）。
            let as_int = fl
                .builder
                .build_ptr_to_int(data_ptr, i64, "arr_dptr_int")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), "arr_dptr_int", scoop2_base::Span::default())
                })?;
            return Ok(Some(as_int.into()));
        }
        _ => return Ok(None),
    }
}

/// GC ptr (addrspace 1) → native ptr (addrspace 0)。
fn gc_to_native<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
    let i64 = fl.cg.context.i64_type();
    match val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() == crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "a2n_int")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), "a2n_int", scoop2_base::Span::default())
                    })?;
                fl.builder
                    .build_int_to_ptr(as_int, fl.cg.native_ptr_ty(), "a2n_ptr")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), "a2n_ptr", scoop2_base::Span::default())
                    })
            } else {
                Ok(p)
            }
        }
        _ => Err(CodegenError::llvm(
            &format!("expected pointer for array receiver, got {:?}", val),
            "gc_to_native",
            scoop2_base::Span::default(),
        )),
    }
}

/// 同 gc_to_native 但返回 BasicValueEnum（用于 store value）。
fn gc_to_native_basic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match val {
        BasicValueEnum::PointerValue(_) => Ok(gc_to_native(fl, val)?.into()),
        _ => Ok(val),
    }
}

/// 若值是 native ptr 但目标期望 GC ptr（引用类型元素），转回 GC ptr。
fn native_to_gc_if_ptr<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    val: BasicValueEnum<'ctx>,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let i64 = fl.cg.context.i64_type();
    match val {
        BasicValueEnum::PointerValue(p) => {
            if p.get_type().get_address_space() != crate::context::gc_address_space() {
                let as_int = fl
                    .builder
                    .build_ptr_to_int(p, i64, "n2g_int")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), "n2g_int", scoop2_base::Span::default())
                    })?;
                let gc = fl
                    .builder
                    .build_int_to_ptr(as_int, fl.cg.gc_ptr_ty(), "n2g_ptr")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), "n2g_ptr", scoop2_base::Span::default())
                    })?;
                Ok(gc.into())
            } else {
                Ok(val)
            }
        }
        _ => Ok(val),
    }
}

/// 按已知 intrinsic name lower（公开接口，供 codegen direct.rs 调用）。
pub fn try_lower_named_intrinsic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    name: &str,
    args: &[LirOperand],
    result_ty: TypeId,
) -> CodegenResult<Option<BasicValueEnum<'ctx>>> {
    lower_named_intrinsic(fl, name, args).map(Some)
}

/// 按已知 intrinsic name lower。未实现的返回错误。
fn lower_named_intrinsic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    name: &str,
    args: &[LirOperand],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let ctx = fl.cg.context;
    // to_string：按类型 dispatch 到 runtime scoop_*_to_string（runtime 接受 i64 标量 / f64）。
    // 注意：string_to_string 不是 intrinsic（String.toString 有真实 body，返回 this）。
    // 不匹配 _to_string 后缀，避免误将 String.toString 方法调用当 intrinsic 内联。
    if name.ends_with("_to_string") && !name.starts_with("string") {
        let gc_ptr_ty = fl.cg.gc_ptr_ty();
        let arg0 = one_arg(fl, args, 0)?;
        let call = if name.starts_with("float") {
            // 按实际位宽 dispatch：f32 → scoop_float32_to_string(f32)，
            // f64 → scoop_float64_to_string(f64)（与旧管线/runtime ABI 对齐）。
            let fv = crate::body::expect_float_val(arg0, "toString 参数", &fl.fqn)?;
            if fv.get_type() == ctx.f32_type() {
                fl.builder
                    .build_call(fl.rt.float32_to_string, &[fv.into()], "f2s")
            } else {
                fl.builder
                    .build_call(fl.rt.float64_to_string, &[fv.into()], "f2s")
            }
        } else {
            // 标量值扩展到 i64（runtime scoop_*_to_string 接受 int64_t）。
            let arg_int = crate::body::expect_int_val(arg0, "toString 参数", &fl.fqn)?;
            let arg_int_64 = zext_to_i64(fl, arg_int);
            if name.starts_with("int") {
                fl.builder
                    .build_call(fl.rt.int_to_string, &[arg_int_64.into()], "i2s")
            } else if name.starts_with("bool") {
                fl.builder
                    .build_call(fl.rt.bool_to_string, &[arg_int_64.into()], "b2s")
            } else if name.starts_with("char") {
                fl.builder
                    .build_call(fl.rt.char_to_string, &[arg_int_64.into()], "c2s")
            } else {
                return Err(CodegenError::unsupported(
                    format!("unsupported to_string intrinsic: {name}"),
                    &fl.fqn,
                    scoop2_base::Span::default(),
                ));
            }
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
        let ptr = match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => v.into_pointer_value(),
            _ => gc_ptr_ty.const_null(),
        };
        return Ok(ptr.into());
    }
    // 浮点 intrinsic（Float32/Float64 共用 `float_*` 名；按操作数实际位宽 lowering）。
    // 必须先于 int_binary_op——"float_plus" 等同样以 "_plus"/"_minus" 结尾，会被整型路径误匹配。
    if name.starts_with("float") {
        return lower_float_intrinsic(fl, name, args);
    }
    // 整数二元运算（plus/minus/times/div/rem/and/or/xor/shl/shr）。
    if let Some(op) = int_binary_op(name) {
        let lhs = one_arg(fl, args, 0)?;
        let rhs = one_arg(fl, args, 1)?;
        let lhs_i = crate::body::expect_int_val(lhs, "string_compare 参数", &fl.fqn)?;
        let rhs_i = crate::body::expect_int_val(rhs, "string_compare 参数", &fl.fqn)?;
        // 移位单独处理：保持 lhs 原始位宽（结果类型 = receiver 类型），
        // 只把移位量归一到 lhs 宽度并按位宽掩码。unify 到 i64 会让结果错位存储
        // 到窄 receiver 槽（store i64 → load i8）。
        if matches!(op, IntBin::Shl | IntBin::LShr | IntBin::AShr) {
            let width = lhs_i.get_type().get_bit_width();
            let lhs_ty = lhs_i.get_type();
            let rhs_typed = if rhs_i.get_type().get_bit_width() == width {
                rhs_i
            } else if rhs_i.get_type().get_bit_width() < width {
                fl.builder
                    .build_int_z_extend(rhs_i, lhs_ty, "shift_amt_ext")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                    })?
            } else {
                fl.builder
                    .build_int_truncate(rhs_i, lhs_ty, "shift_amt_trunc")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                    })?
            };
            // Scoop 语义要求移位量按位宽取模（`1 << 64` == `1 << 0` == 1）。
            // LLVM 的 shl/lshr/ashr 在 shift >= bitwidth 时是 UB，必须先 `& (width-1)`。
            let mask = lhs_ty.const_int((width - 1) as u64, false);
            let masked = fl
                .builder
                .build_and(rhs_typed, mask, "shift_masked")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let res = match op {
                IntBin::Shl => fl.builder.build_left_shift(lhs_i, masked, "shl"),
                IntBin::LShr => fl.builder.build_right_shift(lhs_i, masked, false, "lshr"),
                // `int_shr`：有符号类型算术右移；无符号类型（UInt*）逻辑右移。
                IntBin::AShr => {
                    let recv_unsigned =
                        args.first().is_some_and(|a| operand_is_unsigned(fl, a));
                    if recv_unsigned {
                        fl.builder.build_right_shift(lhs_i, masked, false, "lshr")
                    } else {
                        fl.builder.build_right_shift(lhs_i, masked, true, "ashr")
                    }
                }
                _ => fl.builder.build_left_shift(lhs_i, masked, "shl"),
            }
            .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(res.into());
        }
        let (lhs_i, rhs_i) = unify_int_pair(fl, lhs_i, rhs_i);
        // 算术结果宽度 = unify 后宽度；但存储回 receiver 类型 slot 时需截断回
        // receiver 原始宽度（unify 可能把 UInt8 扩到 i64，store i64 到 i8 slot 会越界）。
        // 此处保留 unify 后结果，由 store 路径按 receiver 类型截断（见 store_local）。
        let res = match op {
            IntBin::Add => fl.builder.build_int_add(lhs_i, rhs_i, "add"),
            IntBin::Sub => fl.builder.build_int_sub(lhs_i, rhs_i, "sub"),
            IntBin::Mul => fl.builder.build_int_mul(lhs_i, rhs_i, "mul"),
            IntBin::And => fl.builder.build_and(lhs_i, rhs_i, "and"),
            IntBin::Or => fl.builder.build_or(lhs_i, rhs_i, "or"),
            IntBin::Xor => fl.builder.build_xor(lhs_i, rhs_i, "xor"),
            // 移位已在上方单独处理（保持 lhs 原始位宽）；此分支不可达。
            IntBin::Shl | IntBin::LShr | IntBin::AShr => {
                fl.builder.build_left_shift(lhs_i, rhs_i, "shl_unreachable")
            }
        }
        .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
        return Ok(res.into());
    }
    // 整数除法/取余（符号性按 intrinsic 名前缀：`uint_*` 无符号，`int_*` 等有符号）。
    match name {
        n if n.ends_with("_div") => {
            let lhs =
                crate::body::expect_int_val(one_arg(fl, args, 0)?, "intrinsic 整型参数", &fl.fqn)?;
            let rhs =
                crate::body::expect_int_val(one_arg(fl, args, 1)?, "intrinsic 整型参数", &fl.fqn)?;
            let (lhs, rhs) = unify_int_pair(fl, lhs, rhs);
            // 符号性：sysroot 的 UInt*/UIntPtr.div 复用 int_div 注解，故以 receiver
            // 类型符号性为准（而非注解名前缀）。
            let is_unsigned = args.first().is_some_and(|a| operand_is_unsigned(fl, a));
            let r = if is_unsigned {
                fl.builder.build_int_unsigned_div(lhs, rhs, "udiv")
            } else {
                fl.builder.build_int_signed_div(lhs, rhs, "sdiv")
            }
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
            return Ok(r.into());
        }
        n if n.ends_with("_rem") => {
            let lhs =
                crate::body::expect_int_val(one_arg(fl, args, 0)?, "intrinsic 整型参数", &fl.fqn)?;
            let rhs =
                crate::body::expect_int_val(one_arg(fl, args, 1)?, "intrinsic 整型参数", &fl.fqn)?;
            let (lhs, rhs) = unify_int_pair(fl, lhs, rhs);
            let is_unsigned = args.first().is_some_and(|a| operand_is_unsigned(fl, a));
            let r = if is_unsigned {
                fl.builder.build_int_unsigned_rem(lhs, rhs, "urem")
            } else {
                fl.builder.build_int_signed_rem(lhs, rhs, "srem")
            }
            .map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
            return Ok(r.into());
        }
        // Char 算术（Char 在 LLVM 中是 i32）：
        //   char_plus_int  : Char + Int  -> Char（i32 + i64 trunc 回 i32）
        //   char_minus_int : Char - Int  -> Char
        //   char_minus_char: Char - Char -> Int（i32 - i32，结果 zext 到 i64）
        "char_plus_int" | "char_minus_int" => {
            let lhs = crate::body::expect_int_val(
                one_arg(fl, args, 0)?,
                "char 算术参数",
                &fl.fqn,
            )?;
            let rhs = crate::body::expect_int_val(
                one_arg(fl, args, 1)?,
                "char 算术参数",
                &fl.fqn,
            )?;
            // rhs (Int/i64) 截到 Char 的 i32 宽度后运算，结果保持 i32（Char 类型）。
            let i32_ty = ctx.i32_type();
            let rhs32 = if rhs.get_type().get_bit_width() == 32 {
                rhs
            } else {
                fl.builder
                    .build_int_truncate(rhs, i32_ty, "char_rhs_trunc")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                    })?
            };
            let r = if name == "char_plus_int" {
                fl.builder.build_int_add(lhs, rhs32, "char_add")
            } else {
                fl.builder.build_int_sub(lhs, rhs32, "char_sub")
            }
            .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        "char_minus_char" => {
            let lhs = crate::body::expect_int_val(
                one_arg(fl, args, 0)?,
                "char_minus_char 参数",
                &fl.fqn,
            )?;
            let rhs = crate::body::expect_int_val(
                one_arg(fl, args, 1)?,
                "char_minus_char 参数",
                &fl.fqn,
            )?;
            // 两 Char (i32) 相减得 i32，结果 zext 到 Int (i64)。
            let diff = fl
                .builder
                .build_int_sub(lhs, rhs, "char_diff")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            let i64 = if diff.get_type().get_bit_width() == 64 {
                diff
            } else {
                fl.builder
                    .build_int_z_extend(diff, ctx.i64_type(), "char_diff_zext")
                    .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?
            };
            return Ok(i64.into());
        }
        // 整数 hash（sysroot int_hash/char_hash via toInt；运行时无对应符号，
        // 简单返回值本身作为 hash——一致性满足，分布非最优但语义正确）。
        n if n.ends_with("_hash") => {
            let v = one_arg(fl, args, 0)?;
            // 数值类型的 hash 就是其整数值（zext 到 i64）。
            if let BasicValueEnum::IntValue(iv) = v {
                let i64 = if iv.get_type().get_bit_width() == 64 {
                    iv
                } else {
                    fl.builder
                        .build_int_z_extend(iv, ctx.i64_type(), "hash_zext")
                        .map_err(|e| {
                            CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                        })?
                };
                return Ok(i64.into());
            }
            return Err(CodegenError::unknown_intrinsic(
                name,
                &format!("hash 不支持的类型 in {}", fl.fqn),
                scoop2_base::Span::default(),
            ));
        }
        n if n.ends_with("_unary_minus") => {
            let v =
                crate::body::expect_int_val(one_arg(fl, args, 0)?, "intrinsic 整型参数", &fl.fqn)?;
            let r = fl.builder.build_int_neg(v, "neg").map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
            return Ok(r.into());
        }
        n if n.ends_with("_inv") => {
            let v =
                crate::body::expect_int_val(one_arg(fl, args, 0)?, "intrinsic 整型参数", &fl.fqn)?;
            let r = fl.builder.build_not(v, "not").map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
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
                            .map_err(|e| {
                                CodegenError::llvm(
                                    e.to_string(),
                                    name,
                                    scoop2_base::Span::default(),
                                )
                            })?
                            .into()
                    } else {
                        iv.into()
                    }
                }
                BasicValueEnum::FloatValue(fv) => fl
                    .builder
                    .build_float_to_signed_int(fv, ctx.i64_type(), "f2i")
                    .map_err(|e| {
                        CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                    })?
                    .into(),
                _ => {
                    return Err(CodegenError::unsupported(
                        format!("to_int 不支持的类型: {name}"),
                        &fl.fqn,
                        scoop2_base::Span::default(),
                    ));
                }
            };
            return Ok(result);
        }
        n if n.ends_with("_compare_to") => {
            let lhs =
                crate::body::expect_int_val(one_arg(fl, args, 0)?, "intrinsic 整型参数", &fl.fqn)?;
            let rhs =
                crate::body::expect_int_val(one_arg(fl, args, 1)?, "intrinsic 整型参数", &fl.fqn)?;
            let (lhs, rhs) = unify_int_pair(fl, lhs, rhs);
            let is_unsigned = args.first().is_some_and(|a| operand_is_unsigned(fl, a));
            let lt_pred = if is_unsigned {
                IntPredicate::ULT
            } else {
                IntPredicate::SLT
            };
            let gt_pred = if is_unsigned {
                IntPredicate::UGT
            } else {
                IntPredicate::SGT
            };
            // 返回 -1/0/1。
            let lt = fl
                .builder
                .build_int_compare(lt_pred, lhs, rhs, "lt")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let gt = fl
                .builder
                .build_int_compare(gt_pred, lhs, rhs, "gt")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let i64_ty = ctx.i64_type();
            let lt_i = fl
                .builder
                .build_int_z_extend(lt, i64_ty, "lt_i")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let gt_i = fl
                .builder
                .build_int_z_extend(gt, i64_ty, "gt_i")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let r = fl.builder.build_int_sub(gt_i, lt_i, "cmp").map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
            return Ok(r.into());
        }
        // 注意：`_not_equals` 必须先于 `_equals`——"int_not_equals" 同时以 "_equals" 结尾，
        // 顺序颠倒会把 `!=` 错误地 lowering 成 EQ。
        n if n.ends_with("_not_equals") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Ne)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_equals") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Eq)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_lt") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Lt)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_le") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Le)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_gt") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Gt)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_ge") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Ge)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        // `bool_ne` 双重含义（sysroot）：作为 `Bool.not()`（一元，1 实参）时是逻辑非；
        // 作为二元 NE（2 实参）时走下方 `_ne` 分支。这里先处理一元情形。
        "bool_ne" if args.len() == 1 => {
            let v = crate::body::expect_int_val(
                one_arg(fl, args, 0)?,
                "bool_ne(not) 参数",
                &fl.fqn,
            )?;
            // Bool 在 LLVM 是 i8；与 1 异或 = 逻辑取反（0→1, 1→0）。
            let one = v.get_type().const_int(1, false);
            let r = fl
                .builder
                .build_xor(v, one, "bool_not")
                .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            return Ok(r.into());
        }
        // 短形式 `_eq` / `_ne`（sysroot `@Intrinsic("int_eq"/"int_ne")` / `bool_eq`/`bool_ne`）。
        // 注意顺序：必须晚于 `_ge`/`_le`（它们不以 `_eq`/`_ne` 结尾，故无冲突），
        // 但 `_ne` 不能被 `_le` 误匹配（"int_ne" 不以 "_le" 结尾）。
        n if n.ends_with("_ne") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Ne)?;
            return Ok(zext_bool(fl, r, name)?);
        }
        n if n.ends_with("_eq") => {
            let r = lower_int_cmp(fl, name, args, IntCmp::Eq)?;
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

/// 整数比较种类。
#[derive(Clone, Copy)]
enum IntCmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// 整数比较 intrinsic lowering：统一操作数位宽 + 按类型符号性选择谓词。
///
/// 符号性由 intrinsic 名前缀决定：`uint_*` 用无符号谓词（ULT/ULE/UGT/UGE），
/// 其它（`int_*`/`bool_*`/`char_*`）用有符号谓词。EQ/NE 与符号无关。
/// 统一位宽避免 LLVM ICmp 操作数不同宽的验证失败（如 UInt8 == Int 字面量）。
fn lower_int_cmp<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    name: &str,
    args: &[LirOperand],
    cmp: IntCmp,
) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
    let lhs = crate::body::expect_int_val(one_arg(fl, args, 0)?, "intrinsic 整型参数", &fl.fqn)?;
    let rhs = crate::body::expect_int_val(one_arg(fl, args, 1)?, "intrinsic 整型参数", &fl.fqn)?;
    let (lhs, rhs) = unify_int_pair(fl, lhs, rhs);
    // 符号性：sysroot UInt*/UIntPtr 的比较复用 int_* 注解，故以 receiver 类型为准。
    let is_unsigned = args.first().is_some_and(|a| operand_is_unsigned(fl, a));
    let pred = match cmp {
        IntCmp::Eq => IntPredicate::EQ,
        IntCmp::Ne => IntPredicate::NE,
        IntCmp::Lt => {
            if is_unsigned {
                IntPredicate::ULT
            } else {
                IntPredicate::SLT
            }
        }
        IntCmp::Le => {
            if is_unsigned {
                IntPredicate::ULE
            } else {
                IntPredicate::SLE
            }
        }
        IntCmp::Gt => {
            if is_unsigned {
                IntPredicate::UGT
            } else {
                IntPredicate::SGT
            }
        }
        IntCmp::Ge => {
            if is_unsigned {
                IntPredicate::UGE
            } else {
                IntPredicate::SGE
            }
        }
    };
    let label = match cmp {
        IntCmp::Eq => "eq",
        IntCmp::Ne => "ne",
        IntCmp::Lt => "lt",
        IntCmp::Le => "le",
        IntCmp::Gt => "gt",
        IntCmp::Ge => "ge",
    };
    fl.builder
        .build_int_compare(pred, lhs, rhs, label)
        .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))
}

/// 浮点 intrinsic lowering（Float32/Float64；按操作数实际位宽产出 fadd/fcmp 等）。
///
/// 覆盖 sysroot `scoop.core` 中 Float64/Float32 声明的 `float_*` 注解：
/// plus/minus/times/div/rem、unary_minus/unary_plus、compare_to、equals、to_int。
fn lower_float_intrinsic<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    name: &str,
    args: &[LirOperand],
) -> CodegenResult<BasicValueEnum<'ctx>> {
    let ctx = fl.cg.context;
    let float_arg = |fl: &mut FunctionLowerer<'a, 'ctx>, i: usize| {
        crate::body::expect_float_val(one_arg(fl, args, i)?, "float intrinsic 参数", &fl.fqn)
    };
    match name {
        "float_plus" | "float_minus" | "float_times" | "float_div" | "float_rem" => {
            let lhs = float_arg(fl, 0)?;
            let rhs = float_arg(fl, 1)?;
            let res = match name {
                "float_plus" => fl.builder.build_float_add(lhs, rhs, "fadd"),
                "float_minus" => fl.builder.build_float_sub(lhs, rhs, "fsub"),
                "float_times" => fl.builder.build_float_mul(lhs, rhs, "fmul"),
                "float_div" => fl.builder.build_float_div(lhs, rhs, "fdiv"),
                _ => fl.builder.build_float_rem(lhs, rhs, "frem"),
            }
            .map_err(|e| CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default()))?;
            Ok(res.into())
        }
        "float_unary_minus" => {
            let v = float_arg(fl, 0)?;
            let r = fl.builder.build_float_neg(v, "fneg").map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
            Ok(r.into())
        }
        "float_unary_plus" => Ok(float_arg(fl, 0)?.into()),
        "float_compare_to" => {
            // 三路比较：lt → -1，gt → 1，否则 0（与整型 compare_to 同构，返回 i64）。
            let lhs = float_arg(fl, 0)?;
            let rhs = float_arg(fl, 1)?;
            let lt = fl
                .builder
                .build_float_compare(FloatPredicate::OLT, lhs, rhs, "flt")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let gt = fl
                .builder
                .build_float_compare(FloatPredicate::OGT, lhs, rhs, "fgt")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let i64_ty = ctx.i64_type();
            let lt_i = fl
                .builder
                .build_int_z_extend(lt, i64_ty, "lt_i")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let gt_i = fl
                .builder
                .build_int_z_extend(gt, i64_ty, "gt_i")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            let r = fl.builder.build_int_sub(gt_i, lt_i, "fcmp").map_err(|e| {
                CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
            })?;
            Ok(r.into())
        }
        "float_equals" => {
            let lhs = float_arg(fl, 0)?;
            let rhs = float_arg(fl, 1)?;
            let r = fl
                .builder
                .build_float_compare(FloatPredicate::OEQ, lhs, rhs, "feq")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            zext_bool(fl, r, name)
        }
        "float_not_equals" | "float_lt" | "float_le" | "float_gt" | "float_ge" => {
            let lhs = float_arg(fl, 0)?;
            let rhs = float_arg(fl, 1)?;
            let pred = match name {
                "float_not_equals" => FloatPredicate::ONE,
                "float_lt" => FloatPredicate::OLT,
                "float_le" => FloatPredicate::OLE,
                "float_gt" => FloatPredicate::OGT,
                _ => FloatPredicate::OGE,
            };
            let r = fl
                .builder
                .build_float_compare(pred, lhs, rhs, "fcmp")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            zext_bool(fl, r, name)
        }
        "float_to_int" | "float64_to_int" | "float32_to_int" => {
            let v = float_arg(fl, 0)?;
            let r = fl
                .builder
                .build_float_to_signed_int(v, ctx.i64_type(), "f2i")
                .map_err(|e| {
                    CodegenError::llvm(e.to_string(), name, scoop2_base::Span::default())
                })?;
            Ok(r.into())
        }
        _ => Err(CodegenError::unknown_intrinsic(
            name,
            &format!("float intrinsic lower in {}", fl.fqn),
            scoop2_base::Span::default(),
        )),
    }
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
        // `ushr`（逻辑右移）必须先于 `shr`——"int_ushr" 同样以 "_shr" 结尾。
        n if n.ends_with("_ushr") => Some(IntBin::LShr),
        // 有符号 `int_shr`（算术右移）；无符号 `uint_shr` 也走此分支，
        // 但其运算数恒非负，算术右移 == 逻辑右移，结果正确。
        n if n.ends_with("_shr") => Some(IntBin::AShr),
        _ => None,
    }
}

/// 判断一个 operand 的类型是否为无符号整数（UInt*/UIntPtr）。
///
/// 用于 `shr` 等与符号性相关的 intrinsic：sysroot 的 `UInt*.shr` 复用 `int_shr`
/// 注解名，但其语义是无符号逻辑右移。codegen 无法从 LLVM i8/i64 位模式判断符号性，
/// 需回查 operand 的 Scoop 类型布局。
fn operand_is_unsigned(
    fl: &FunctionLowerer,
    operand: &LirOperand,
) -> bool {
    let ty = match operand {
        LirOperand::Local(id) => fl.local_types.get(id).copied(),
        _ => None,
    };
    let Some(ty) = ty else { return false };
    let Some(layout) = fl.layouts.get(ty) else {
        return false;
    };
    matches!(
        layout.kind,
        scoop2_lir::TypeLayoutKind::Scalar {
            scalar_kind: scoop2_lir::ScalarKind::Int { unsigned: true, .. },
            ..
        }
    )
}

/// 把两个整数值归一到相同位宽（取较宽者；窄侧 zext）。
///
/// Scoop 的定宽整数（Int8..UInt64）之间运算/比较要求 LLVM 操作数同宽。
/// 类型检查保证语义合法（如 UInt8 == Int 字面量），但 LLVM 指令拒绝不同宽度的
/// 操作数；这里按较宽的一侧 zext 较窄侧（无符号语义由具体指令/谓词决定）。
pub fn unify_int_pair<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
) -> (inkwell::values::IntValue<'ctx>, inkwell::values::IntValue<'ctx>) {
    let lw = lhs.get_type().get_bit_width();
    let rw = rhs.get_type().get_bit_width();
    if lw == rw {
        (lhs, rhs)
    } else if lw > rw {
        let rhs2 = fl
            .builder
            .build_int_z_extend(rhs, lhs.get_type(), "rhs_zext")
            .unwrap_or_else(|_| lhs.get_type().const_zero());
        (lhs, rhs2)
    } else {
        let lhs2 = fl
            .builder
            .build_int_z_extend(lhs, rhs.get_type(), "lhs_zext")
            .unwrap_or_else(|_| rhs.get_type().const_zero());
        (lhs2, rhs)
    }
}

/// 取第 i 个实参的值（按其本地类型）。
/// 把整数值 zext 到 i64（若已是 i64 则原样返回）。
pub fn zext_to_i64<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    v: inkwell::values::IntValue<'ctx>,
) -> inkwell::values::IntValue<'ctx> {
    if v.get_type().get_bit_width() == 64 {
        v
    } else {
        fl.builder
            .build_int_z_extend(v, fl.cg.context.i64_type(), "zext_i64")
            .unwrap_or_else(|_| fl.cg.context.i64_type().const_zero())
    }
}

fn one_arg<'a, 'ctx>(
    fl: &mut FunctionLowerer<'a, 'ctx>,
    args: &[LirOperand],
    i: usize,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    match args.get(i) {
        Some(LirOperand::Local(id)) => fl.load_local(*id),
        Some(LirOperand::Const(c)) => fl.lower_const_value(c),
        None => Err(CodegenError::unsupported(
            format!("intrinsic 实参不足（需第 {} 个，实有 {}）", i, args.len()),
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
