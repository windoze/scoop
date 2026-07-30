//! 类型布局计算：TypeId → TypeLayout。
//!
//! 对所有在 MIR 模块中出现的类型（locals / params / 返回类型）以及 BackendContracts
//! 引用的类型（struct 字段 / enum variant payload）和 effect lowering 产生的合成类型
//! 计算 size/align/kind，写入 `program.type_layouts`。

use std::collections::HashSet;

use scoop2_base::Interner;
use scoop2_hir::hir::TypedHir;
use scoop2_hir::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use scoop2_mir::mir::materialize::MaterializedMir;

use crate::*;

/// 主入口：为 LirProgram 计算并填充 type_layouts。
pub fn compute_type_layouts(
    program: &mut LirProgram,
    mir: &MaterializedMir,
    hir: &TypedHir,
    interner: &Interner,
) {
    let types = &mir.module.types;
    // 收集所有需要计算布局的 TypeId。
    let mut worklist: Vec<TypeId> = Vec::new();
    let mut seen: HashSet<TypeId> = HashSet::new();

    let enqueue = |ty: TypeId, worklist: &mut Vec<TypeId>, seen: &mut HashSet<TypeId>| {
        if seen.insert(ty) {
            worklist.push(ty);
        }
    };

    // 1. 所有 callable 的参数 + 返回类型 + body locals。
    for item in &mir.module.items {
        match item {
            scoop2_mir::mir::Item::Fun(fd) => {
                enqueue(fd.return_ty, &mut worklist, &mut seen);
                for p in &fd.params {
                    enqueue(p.ty, &mut worklist, &mut seen);
                }
                if let Some(body) = &fd.body {
                    for d in &body.locals {
                        enqueue(d.ty, &mut worklist, &mut seen);
                    }
                    // 收集 body 中引用到的其他 TypeId（rvalue/terminator 中的 TypeId）。
                    collect_body_type_ids(body, &mut |t| enqueue(t, &mut worklist, &mut seen));
                }
            }
            scoop2_mir::mir::Item::Initializer(ir) => {
                enqueue(ir.ty, &mut worklist, &mut seen);
                for d in &ir.body.locals {
                    enqueue(d.ty, &mut worklist, &mut seen);
                }
                collect_body_type_ids(&ir.body, &mut |t| enqueue(t, &mut worklist, &mut seen));
            }
            _ => {}
        }
    }

    // 2. 类型存储中的全部类型（nominal/aggregate 也需要布局）。
    //    按 id 顺序遍历，确保所有 struct/enum/tuple/option 都进入布局表。
    //    TypeStore 没有公开的 iter，但我们已通过 body 扫描收集了主要类型；
    //    再补一次 BackendContracts 中出现的 FQN 对应类型。

    // 处理 worklist：对每个 id 计算布局（递归处理子类型）。
    // 用类似迭代收敛的方式：循环直到 worklist 空。
    // 由于 TypeId 是 u32 下标，我们可以按 id 升序处理，子类型 id 一定更小（intern 顺序）。
    // 但更安全的做法是用栈 + 已有布局表查缓存。
    // （interner 在下方 compute_layout 调用中使用。）

    // 处理 BackendContracts：把 struct/enum 的 nominal value 类型 id 也入队。
    // 我们遍历 types，找到所有 value_nominal 类型入队。
    collect_nominal_value_types(types, &mut |t| enqueue(t, &mut worklist, &mut seen));

    // 按处理顺序计算布局。布局计算本身会递归地确保子类型存在（通过 ensure_layout 入队）。
    // 为避免借用问题，把 worklist 取出处理。
    while let Some(ty) = worklist.pop() {
        if program.type_layouts.get(ty).is_some() {
            continue;
        }
        let layout = compute_layout(ty, types, hir, interner, 0, &mut |t| {
            enqueue(t, &mut worklist, &mut seen)
        });
        program.type_layouts.insert(ty, layout);
    }

    // 3. 合成类型（effect lowering 的 Step enum / continuation struct / frame tuple）
    //    的布局。这些类型的 FQN 不存在于 type store，需要单独构建 SyntheticTypeDecl。
    for item in &mir.module.items {
        if let scoop2_mir::mir::Item::Fun(fd) = item {
            if let Some(eff_abi) = &fd.effect_abi {
                // 先确保 frame_ty / step_ty 的布局存在（step variant payload 类型递归处理）。
                let mut pending: Vec<TypeId> = Vec::new();
                pending.push(eff_abi.frame_ty);
                pending.push(eff_abi.step_ty);
                for v in &eff_abi.step_variants {
                    pending.push(v.payload_ty);
                }
                while let Some(t) = pending.pop() {
                    if program.type_layouts.get(t).is_none() {
                        let mut sub_pending: Vec<TypeId> = Vec::new();
                        let l = compute_layout(t, types, hir, interner, 0, &mut |st| {
                            sub_pending.push(st)
                        });
                        program.type_layouts.insert(t, l);
                        pending.extend(sub_pending);
                    }
                }
                prepare_effect_synthetic_layouts(program, types, hir, interner, fd, eff_abi);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 布局计算核心
// ---------------------------------------------------------------------------

/// 计算单个 TypeId 的布局。`enqueue` 用于把未处理的子类型入队（递归保证）。
/// `depth` 为 nominal 值类型的嵌套深度，用于给 `sub_size_align` 的就地递归
/// 兜底（值类型自引用如 `enum A { V(val a: A) }` 会无限递归）。
fn compute_layout(
    ty: TypeId,
    types: &TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    depth: u32,
    enqueue: &mut impl FnMut(TypeId),
) -> TypeLayout {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Unit) => TypeLayout {
            size: 0,
            align: 1,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Unit,
            },
        },
        TypeKind::Value(ValueTypeKind::Bool) => TypeLayout {
            size: 1,
            align: 1,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Bool,
            },
        },
        TypeKind::Value(ValueTypeKind::Char) => TypeLayout {
            size: 4,
            align: 4,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Char,
            },
        },
        TypeKind::Value(ValueTypeKind::Int) => TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Int {
                    bits: 64,
                    unsigned: false,
                },
            },
        },
        TypeKind::Value(ValueTypeKind::UInt) => TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Int {
                    bits: 64,
                    unsigned: true,
                },
            },
        },
        TypeKind::Value(ValueTypeKind::IntN(bits)) => {
            let b = *bits as u64;
            TypeLayout {
                size: (b + 7) / 8,
                align: align_up_pow2(b),
                kind: TypeLayoutKind::Scalar {
                    scalar_kind: ScalarKind::Int {
                        bits: *bits,
                        unsigned: false,
                    },
                },
            }
        }
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => {
            let b = *bits as u64;
            TypeLayout {
                size: (b + 7) / 8,
                align: align_up_pow2(b),
                kind: TypeLayoutKind::Scalar {
                    scalar_kind: ScalarKind::Int {
                        bits: *bits,
                        unsigned: true,
                    },
                },
            }
        }
        TypeKind::Value(ValueTypeKind::Float64) => TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Float { bits: 64 },
            },
        },
        TypeKind::Value(ValueTypeKind::Float32) => TypeLayout {
            size: 4,
            align: 4,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Float { bits: 32 },
            },
        },
        TypeKind::Value(ValueTypeKind::Tuple(elems)) => {
            let mut offset: u64 = 0;
            let mut max_align: u64 = 1;
            let mut fields: Vec<FieldLayout> = Vec::with_capacity(elems.len());
            for &elem in elems {
                // 先确保子类型布局存在（入队）。
                enqueue(elem);
                let (esize, ealign) = sub_size_align(elem, types, hir, interner, depth);
                offset = align_to(offset, ealign);
                fields.push(FieldLayout {
                    offset,
                    size: esize,
                    ty: elem,
                });
                offset += esize;
                if ealign > max_align {
                    max_align = ealign;
                }
            }
            let size = align_to(offset, max_align).max(0);
            TypeLayout {
                size,
                align: max_align,
                kind: TypeLayoutKind::Tuple { elements: fields },
            }
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            // 内建标量别名（@Intrinsic struct Bool/Char/Int/UInt/Int8../Float64/Float32/Unit）：
            // FQN 尾段匹配内建标量名时，使用标量布局而非 struct 布局。
            let fqn_text = interner.resolve(n.fqn);
            let simple = fqn_text.rsplit('.').next().unwrap_or("");
            if let Some(scalar) = builtin_scalar_kind(simple) {
                return scalar_layout(scalar);
            }
            // Option<T>：走 Option 专属布局（Option 现为 value nominal，按 FQN 判定）。
            if n.fqn == types.option_fqn()
                && let Some(inner) = n.args.first().copied()
            {
                enqueue(inner);
                return compute_option_layout(inner, types, hir, interner, depth);
            }
            // struct / enum 值类型。查询 HIR 获取字段列表。
            // 尝试 struct 布局：查 HIR members（按 member_order 声明序迭代——
            // HashMap 迭代序不确定，直接用会破坏字段布局确定性）。
            if hir.members.contains_key(&n.fqn) {
                let ordered = hir.ordered_members(&n.fqn);
                let mut offset: u64 = 0;
                let mut max_align: u64 = 1;
                let mut fields: Vec<FieldLayout> = Vec::new();
                for (_member_name_sym, member_ty) in ordered {
                    enqueue(member_ty);
                    let (fsize, falign) = sub_size_align(member_ty, types, hir, interner, depth);
                    offset = align_to(offset, falign);
                    fields.push(FieldLayout {
                        offset,
                        size: fsize,
                        ty: member_ty,
                    });
                    offset += fsize;
                    if falign > max_align {
                        max_align = falign;
                    }
                }
                let size = align_to(offset, max_align).max(1);
                return TypeLayout {
                    size,
                    align: max_align,
                    kind: TypeLayoutKind::Struct { fields },
                };
            }
            // 尝试 enum 布局：查 HIR enum_variants。
            if let Some(variants) = hir.enum_variants.get(&n.fqn) {
                // 布局规则（NEW-LLVM-CODEGEN.md §3.1 Enum）：
                // - 不含 ref 的 variant（深层判定）共用一个 scalar union slot；
                // - 每个含 ref 的 variant 各占独立 slot（其 payload 的自然 struct
                //   布局），trace_offsets 取全部 ref variant 区域 ref 叶子偏移并集。
                // 约束根源：GC 按静态位置 trace，同一偏移在所有 variant 下的
                // "ref 性"必须唯一。
                // variant payload 字段：`<enum_fqn>.<variant>` 在 hir.members 中登记
                // （注册见 typecheck/env.rs），按声明序做 struct 式布局。
                // 单字段 variant 记录 payload_ty（构造/提取的具类型读写依据）；
                // 多字段 variant 记录 payload_fields（相对本 variant slot 起点）。
                struct VariantPayload {
                    name: String,
                    fields: Vec<FieldLayout>,
                    size: u64,
                    align: u64,
                    has_ref: bool,
                }
                let mut payloads: Vec<VariantPayload> = Vec::new();
                for &variant_name_sym in variants {
                    let variant_name = interner.resolve(variant_name_sym).to_string();
                    let variant_fqn_text = format!("{fqn_text}.{variant_name}");
                    let ordered = interner
                        .get(&variant_fqn_text)
                        .map(|vf| hir.ordered_members(&vf))
                        .unwrap_or_default();
                    let mut offset: u64 = 0;
                    let mut align: u64 = 1;
                    let mut has_ref = false;
                    let mut fields: Vec<FieldLayout> = Vec::new();
                    for (_field_name_sym, field_ty) in &ordered {
                        enqueue(*field_ty);
                        let (fsize, falign) =
                            sub_size_align(*field_ty, types, hir, interner, depth);
                        let field_offset = align_to(offset, falign);
                        fields.push(FieldLayout {
                            offset: field_offset,
                            size: fsize,
                            ty: *field_ty,
                        });
                        offset = field_offset + fsize;
                        if falign > align {
                            align = falign;
                        }
                        if !has_ref
                            && type_contains_ref(
                                *field_ty,
                                types,
                                hir,
                                interner,
                                &mut std::collections::HashSet::new(),
                            )
                        {
                            has_ref = true;
                        }
                    }
                    payloads.push(VariantPayload {
                        name: variant_name,
                        fields,
                        size: align_to(offset, align),
                        align,
                        has_ref,
                    });
                }
                let tag_size: u64 = if variants.len() <= 256 { 1 } else { 4 };
                // scalar union slot：所有无 ref variant 共用。
                let scalar_size: u64 = payloads
                    .iter()
                    .filter(|p| !p.has_ref)
                    .map(|p| p.size)
                    .max()
                    .unwrap_or(0);
                let scalar_align: u64 = payloads
                    .iter()
                    .filter(|p| !p.has_ref)
                    .map(|p| p.align)
                    .max()
                    .unwrap_or(1);
                let scalar_slot = align_to(tag_size, scalar_align);
                // ref variant 独立 slot：紧跟 scalar slot 顺序排布。
                let mut cursor = scalar_slot + scalar_size;
                let mut variant_layouts: Vec<EnumVariantLayout> = Vec::new();
                for (i, p) in payloads.iter().enumerate() {
                    let slot_offset = if p.has_ref {
                        let off = align_to(cursor, p.align);
                        cursor = off + p.size;
                        off
                    } else {
                        scalar_slot
                    };
                    variant_layouts.push(EnumVariantLayout {
                        name: p.name.clone(),
                        tag_value: i as u64,
                        slot_offset,
                        payload_ty: if p.fields.len() == 1 {
                            Some(p.fields[0].ty)
                        } else {
                            None
                        },
                        payload_fields: if p.fields.len() > 1 {
                            p.fields.clone()
                        } else {
                            Vec::new()
                        },
                    });
                }
                let max_align = payloads
                    .iter()
                    .map(|p| p.align)
                    .max()
                    .unwrap_or(1)
                    .max(tag_size);
                let total = if cursor > scalar_slot || scalar_size > 0 {
                    align_to(cursor.max(scalar_slot + scalar_size), max_align)
                } else {
                    tag_size
                };
                return TypeLayout {
                    size: total,
                    align: max_align,
                    kind: TypeLayoutKind::Enum {
                        tag_size,
                        tag_offset: 0,
                        payload_offset: scalar_slot,
                        variants: variant_layouts,
                    },
                };
            }
            // 未找到 HIR 声明：默认为空 struct。
            TypeLayout {
                size: 0,
                align: 1,
                kind: TypeLayoutKind::Struct { fields: Vec::new() },
            }
        }
        TypeKind::Ref(ref_kind) => {
            let (gc_traceable, rk) = match ref_kind {
                RefTypeKind::String => (true, RefKind::String),
                RefTypeKind::Nominal(n) => {
                    // Any（scoop.core.Any）布局为根引用：GC 可追踪、RefKind::Any。
                    if n.fqn == types.any_fqn() {
                        (true, RefKind::Any)
                    } else {
                        // 区分 interface 引用与 class 引用：interface 走 itable 分发，
                        // class 走 vtable 分发。HIR 的 interface_fqns 集合记录所有 interface
                        // 类型的 FQN，据此选择 RefKind。
                        let is_interface = hir.interface_fqns.contains(&n.fqn);
                        let rk = if is_interface {
                            RefKind::Interface
                        } else {
                            RefKind::Class
                        };
                        (true, rk)
                    }
                }
                RefTypeKind::Function(_) => {
                    // 函数类型走 Function 布局种类。
                    return TypeLayout {
                        size: 8,
                        align: 8,
                        kind: TypeLayoutKind::Function,
                    };
                }
                RefTypeKind::Union(_) => (true, RefKind::Any),
            };
            TypeLayout {
                size: 8,
                align: 8,
                kind: TypeLayoutKind::Reference {
                    gc_traceable,
                    ref_kind: rk,
                },
            }
        }
        TypeKind::Nothing => TypeLayout {
            size: 0,
            align: 1,
            kind: TypeLayoutKind::Nothing,
        },
        // 类型参数 / 星投影：在 monomorphic MIR 中不应出现。保守按指针处理。
        TypeKind::Param(_) | TypeKind::StarProjection => TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Reference {
                gc_traceable: true,
                ref_kind: RefKind::Any,
            },
        },
    }
}

/// 计算 Option<inner> 的布局。
fn compute_option_layout(
    inner: TypeId,
    types: &TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    depth: u32,
) -> TypeLayout {
    // niche 判定：inner 是引用 → Pointer niche（null 表示 None）。
    // inner 是 Bool/Unit/Int 等标量 → Tagged（tag 字节 + payload）。
    let is_inner_ref = matches!(types.kind(inner), TypeKind::Ref(_) | TypeKind::Nothing);
    if is_inner_ref {
        // Option<Ref> = pointer size，null niche。
        TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Option {
                storage: NicheStorage::Pointer,
                payload_size: 8,
                payload_ty: inner,
            },
        }
    } else {
        // 标量/聚合 inner：tag 字节 + payload（带 padding）。
        let (psize, palign) = sub_size_align(inner, types, hir, interner, depth);
        let tag_size: u64 = 1;
        let total_align = palign.max(1);
        let payload_offset = align_to(tag_size, palign);
        let end = payload_offset + psize;
        let size = align_to(end, total_align).max(tag_size);
        TypeLayout {
            size,
            align: total_align,
            kind: TypeLayoutKind::Option {
                storage: NicheStorage::Tagged,
                payload_size: psize,
                payload_ty: inner,
            },
        }
    }
}

/// 深层判定类型是否含 GC 引用（NEW-LLVM-CODEGEN.md §3.1：enum 布局的
/// scalar/ref variant 分类依据；浅判定——value 类型一律 false——是 bug，禁止回潮）。
/// 结构递归展开 Tuple/Option/struct/enum；`seen` 防止值类型自引用无限递归
/// （重访节点不贡献新 ref——其可达 ref 已由首次展开覆盖）。
fn type_contains_ref(
    ty: TypeId,
    types: &TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    seen: &mut std::collections::HashSet<TypeId>,
) -> bool {
    if !seen.insert(ty) {
        return false;
    }
    match types.kind(ty) {
        // 引用类型（含 Any/String/class/interface/函数值）都是 GC 指针。
        TypeKind::Ref(_) => true,
        // 类型参数 / 星投影：单态化后不应出现；保守按含 ref 处理。
        TypeKind::Param(_) | TypeKind::StarProjection => true,
        TypeKind::Value(ValueTypeKind::Tuple(elems)) => elems
            .iter()
            .any(|&e| type_contains_ref(e, types, hir, interner, seen)),
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            let fqn_text = interner.resolve(n.fqn);
            let simple = fqn_text.rsplit('.').next().unwrap_or("");
            if builtin_scalar_kind(simple).is_some() {
                return false;
            }
            // Option<T>：按 inner 递归判定（Option 现为 value nominal，按 FQN 判定）。
            if n.fqn == types.option_fqn()
                && let Some(inner) = n.args.first().copied()
            {
                return type_contains_ref(inner, types, hir, interner, seen);
            }
            // struct 值类型：任一成员深层含 ref 即含 ref。
            if hir.members.contains_key(&n.fqn) {
                return hir
                    .ordered_members(&n.fqn)
                    .iter()
                    .any(|(_, mt)| type_contains_ref(*mt, types, hir, interner, seen));
            }
            // enum 值类型：任一 variant 的任一 payload 字段深层含 ref 即含 ref。
            if let Some(variants) = hir.enum_variants.get(&n.fqn) {
                return variants.iter().any(|&v| {
                    let variant_fqn_text = format!("{fqn_text}.{}", interner.resolve(v));
                    interner
                        .get(&variant_fqn_text)
                        .map(|vf| hir.ordered_members(&vf))
                        .unwrap_or_default()
                        .iter()
                        .any(|(_, mt)| type_contains_ref(*mt, types, hir, interner, seen))
                });
            }
            false
        }
        _ => false,
    }
}

/// 取子类型的 (size, align)，优先用已计算的布局表，否则就地递归计算。
/// 注意：这里只读 types，不写 program，避免借用冲突；调用方需保证子类型最终入队。
/// `depth` 为 nominal 值类型嵌套深度：Nominal 分支就地调用 compute_layout 取真实
/// 尺寸（嵌套 struct/enum 值类型不能按保守指针尺寸估算，否则字段偏移/总尺寸错）；
/// depth 达到上限时退回保守值 (8, 8)，防止值类型自引用无限递归。
fn sub_size_align(
    ty: TypeId,
    types: &TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    depth: u32,
) -> (u64, u64) {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Unit) | TypeKind::Nothing => (0, 1),
        TypeKind::Value(ValueTypeKind::Bool) => (1, 1),
        TypeKind::Value(ValueTypeKind::Char) => (4, 4),
        TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::Float64) => (8, 8),
        TypeKind::Value(ValueTypeKind::Float32) => (4, 4),
        TypeKind::Value(ValueTypeKind::IntN(bits))
        | TypeKind::Value(ValueTypeKind::UIntN(bits)) => {
            let b = *bits as u64;
            ((b + 7) / 8, align_up_pow2(b))
        }
        TypeKind::Ref(_) => (8, 8),
        TypeKind::Param(_) | TypeKind::StarProjection => (8, 8),
        TypeKind::Value(ValueTypeKind::Tuple(elems)) => {
            let mut offset: u64 = 0;
            let mut max_align: u64 = 1;
            for &e in elems {
                let (s, a) = sub_size_align(e, types, hir, interner, depth);
                offset = align_to(offset, a) + s;
                if a > max_align {
                    max_align = a;
                }
            }
            (align_to(offset, max_align), max_align)
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            // Option<T>：直接走 Option 专属布局（Option 现为 value nominal，按 FQN 判定）。
            if n.fqn == types.option_fqn()
                && let Some(inner) = n.args.first().copied()
            {
                let l = compute_option_layout(inner, types, hir, interner, depth);
                return (l.size, l.align);
            }
            // Nominal value struct/enum：就地递归计算真实布局取 (size, align)。
            // 嵌套深度达上限（值类型自引用）时退回保守值；enqueue 传 no-op——
            // 子类型的入队由外层 compute_layout 的 struct/enum 分支负责。
            const MAX_NEST_DEPTH: u32 = 32;
            if depth >= MAX_NEST_DEPTH {
                return (8, 8);
            }
            let l = compute_layout(ty, types, hir, interner, depth + 1, &mut |_| {});
            (l.size, l.align)
        }
    }
}

/// 在已计算的 type_layouts 中查找首个匹配 `pred` 的 Scalar 类型 TypeId。
fn find_scalar_type_in_program(
    program: &LirProgram,
    pred: impl Fn(&crate::ScalarKind) -> bool,
) -> Option<TypeId> {
    program
        .type_layouts
        .entries
        .iter()
        .find(
            |(_, l)| matches!(&l.kind, TypeLayoutKind::Scalar { scalar_kind } if pred(scalar_kind)),
        )
        .map(|(t, _)| *t)
}

/// 把 `n` 向上取整到 >= n 的最小 2 的幂（用于对齐）。
fn align_up_pow2(bits: u64) -> u64 {
    if bits <= 8 {
        1
    } else if bits <= 16 {
        2
    } else if bits <= 32 {
        4
    } else {
        8
    }
}

/// 把 `offset` 向上对齐到 `align` 的倍数（align 必须是 2 的幂；非 2 的幂退化为按 align 处理）。
fn align_to(offset: u64, align: u64) -> u64 {
    if align <= 1 {
        return offset;
    }
    let mask = align - 1;
    (offset + mask) & !mask
}

// ---------------------------------------------------------------------------
// Body 内 TypeId 收集
// ---------------------------------------------------------------------------

/// 遍历 Body 中的所有 TypeId（rvalue / statement / terminator 的 transport metadata）。
fn collect_body_type_ids(body: &scoop2_mir::mir::Body, emit: &mut impl FnMut(TypeId)) {
    use scoop2_mir::mir::{StatementKind, TerminatorKind};
    for block in &body.blocks {
        for stmt in &block.stmts {
            match &stmt.kind {
                StatementKind::Assign { value, .. } => collect_rvalue_type_ids(value, emit),
                StatementKind::StoreMember { value_ty, .. }
                | StatementKind::StoreTupleIndex { value_ty, .. }
                | StatementKind::StoreTopLevelVar { value_ty, .. } => emit(*value_ty),
                _ => {}
            }
        }
        match &block.terminator.kind {
            TerminatorKind::Perform { metadata, args, .. } => {
                emit(metadata.effect_ty);
                emit(metadata.result_ty);
                for t in &metadata.op_type_args {
                    emit(*t);
                }
                if let Some(pt) = metadata.payload_tuple_ty {
                    emit(pt);
                }
                for t in &metadata.payload_component_tys {
                    emit(*t);
                }
                for a in args {
                    emit(a.value_ty);
                }
            }
            TerminatorKind::Handle { metadata, arms, .. } => {
                emit(metadata.result_ty);
                emit(metadata.body_result_ty);
                if let Some(f) = metadata.finally_result_ty {
                    emit(f);
                }
                for arm in arms {
                    emit(arm.handled_effect_ty);
                    emit(arm.body_ty);
                    for t in &arm.op_type_args {
                        emit(*t);
                    }
                    if let Some(pt) = arm.payload_tuple_ty {
                        emit(pt);
                    }
                    for t in &arm.payload_component_tys {
                        emit(*t);
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_rvalue_type_ids(rv: &scoop2_mir::mir::Rvalue, emit: &mut impl FnMut(TypeId)) {
    use scoop2_mir::mir::Rvalue;
    match rv {
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => {
            collect_call_kind_type_ids(kind, emit);
            for a in args {
                emit(a.value_ty);
            }
            emit(transport.result.source_ty);
        }
        Rvalue::MakeTuple {
            elements: _,
            transport,
        } => {
            emit(transport.aggregate_ty);
            for f in &transport.fields {
                emit(f.ty);
            }
        }
        Rvalue::MakeArray { result_ty, .. } => emit(*result_ty),
        Rvalue::EnumVariant {
            enum_ty,
            args,
            payload,
            ..
        } => {
            emit(*enum_ty);
            emit(payload.aggregate_ty);
            for a in args {
                emit(a.value_ty);
            }
        }
        Rvalue::ClassCtor {
            args,
            hidden_effects: _,
            ..
        } => {
            for a in args {
                emit(a.value_ty);
            }
        }
        Rvalue::StructLit {
            fields, transport, ..
        } => {
            emit(transport.aggregate_ty);
            for f in fields {
                emit(f.value_ty);
            }
        }
        Rvalue::WithUpdate { result_ty, .. } => emit(*result_ty),
        Rvalue::MakeClosure { env_contract, .. } => {
            emit(env_contract.env_ty);
        }
        Rvalue::TupleIndex { element_ty, .. } | Rvalue::IndexAccess { element_ty, .. } => {
            emit(*element_ty)
        }
        Rvalue::PatternExtract { result_ty, .. } => emit(*result_ty),
        Rvalue::PerformResult { result_ty, .. } => emit(*result_ty),
        _ => {}
    }
}

fn collect_call_kind_type_ids(kind: &scoop2_mir::mir::CallKind, emit: &mut impl FnMut(TypeId)) {
    use scoop2_mir::mir::CallKind;
    match kind {
        CallKind::Direct {
            type_args,
            generic_type_args,
            ..
        } => {
            for t in type_args {
                emit(*t);
            }
            for t in generic_type_args {
                emit(*t);
            }
        }
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            emit(dispatch.receiver_ty);
            for t in &dispatch.generic_type_args {
                emit(*t);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Nominal value 类型收集（struct/enum）
// ---------------------------------------------------------------------------

/// 遍历 TypeStore，把所有 value_nominal 类型 id 通过 emit 报告。
/// 由于 TypeStore 不暴露 iter，我们通过遍历 module 中已知的 nominal FQN，
/// 在 BackendContracts 中查找并构造对应布局。
fn collect_nominal_value_types(_types: &TypeStore, _emit: &mut impl FnMut(TypeId)) {
    // TypeStore 没有公开的遍历接口；BackendContracts 的字段是 FQN 文本而非 TypeId，
    // 因此 nominal value 类型的布局通过 lookup 路径在需要时按 FQN 补全。
    // 这里留空：value_nominal 类型在 body 中被引用时会通过 worklist 入队。
}

// ---------------------------------------------------------------------------
// Effect 合成类型布局
// ---------------------------------------------------------------------------

fn prepare_effect_synthetic_layouts(
    program: &mut LirProgram,
    types: &TypeStore,
    hir: &TypedHir,
    interner: &Interner,
    fd: &scoop2_mir::mir::FunDecl,
    eff_abi: &scoop2_mir::mir::EffectStepAbi,
) {
    // Frame tuple：frame_ty（一个 value Tuple 类型）。
    if program
        .synthetic_types
        .iter()
        .all(|s| s.fqn != format!("{}$frame", fd.fqn))
    {
        let frame_layout = program
            .type_layouts
            .get(eff_abi.frame_ty)
            .cloned()
            .unwrap_or(TypeLayout {
                size: 0,
                align: 1,
                kind: TypeLayoutKind::Tuple {
                    elements: Vec::new(),
                },
            });
        program.synthetic_types.push(SyntheticTypeDecl {
            fqn: format!("{}$frame", fd.fqn),
            kind: SyntheticTypeKind::FrameTuple,
            layout: frame_layout,
        });
    }

    // Step enum：step_ty（一个 value Nominal 类型，但语义上是 tagged union）。
    if program
        .synthetic_types
        .iter()
        .all(|s| s.fqn != format!("{}$step", fd.fqn))
    {
        // tag 字节 + payload union（按最大 payload 对齐）。
        // escape continuation 的 answer 经克隆边界 Complete 回传，与函数返回
        // 共用 payload 槽——answer 类型一并参与尺寸/对齐计算（元数据里的
        // Complete payload_ty 仍是函数返回类型，caller 提取不受影响）。
        let mut max_payload_size: u64 = 0;
        let mut max_payload_align: u64 = 1;
        for v in &eff_abi.step_variants {
            let (s, a) = sub_size_align(v.payload_ty, types, hir, interner, 0);
            if s > max_payload_size {
                max_payload_size = s;
            }
            if a > max_payload_align {
                max_payload_align = a;
            }
        }
        for &ty in &eff_abi.escape_answer_tys {
            let (s, a) = sub_size_align(ty, types, hir, interner, 0);
            if s > max_payload_size {
                max_payload_size = s;
            }
            if a > max_payload_align {
                max_payload_align = a;
            }
        }
        let tag_size: u64 = 1;
        let payload_offset = align_to(tag_size, max_payload_align);
        let total = align_to(
            payload_offset + max_payload_size,
            max_payload_align.max(tag_size),
        );
        let step_layout = TypeLayout {
            size: total,
            align: max_payload_align.max(tag_size),
            kind: TypeLayoutKind::Enum {
                tag_size,
                tag_offset: 0,
                payload_offset,
                variants: eff_abi
                    .step_variants
                    .iter()
                    .enumerate()
                    .map(|(i, v)| EnumVariantLayout {
                        name: v.name.clone(),
                        tag_value: i as u64,
                        slot_offset: payload_offset,
                        payload_ty: Some(v.payload_ty),
                        payload_fields: Vec::new(),
                    })
                    .collect(),
            },
        };
        // Step enum 的真实 tagged-union 布局同时是 type_layouts 中 step_ty 的权威布局
        //（step_ty 是合成 nominal 类型，通用 compute_layout 路径只会产出空 struct）。
        program.type_layouts.insert(eff_abi.step_ty, step_layout.clone());
        program.synthetic_types.push(SyntheticTypeDecl {
            fqn: format!("{}$step", fd.fqn),
            kind: SyntheticTypeKind::StepEnum,
            layout: step_layout,
        });
    }

    // 间接调用链站点（FunValue/Closure）的合成 Step 布局：与函数自身 Step
    // 同款 tagged-union，按站点变体表计算。Direct 站点的 step_ty 即 callee 的
    // abi.step_ty（由 callee 的登记覆盖，含 escape_answer_tys 撑大尺寸），
    // 这里只登记 callee 处看不到的站点级合成类型（step_fqn_sym 为 default）。
    // 已知限制：站点布局不含动态 callee 的 escape_answer_tys——callee 内部
    // 若有 answer 类型大于返回类型的 escape continuation，其 wrapper 写出的
    // Step 可能大于站点布局（当前 fixture 集未覆盖该形状）。
    for site in &eff_abi.call_chain_sites {
        // 无条件覆盖：通用 compute_layout 可能已先为站点合成 nominal 登记了
        // 空 struct 布局（nominal 无字段元数据），tagged-union 才是权威布局。
        // 同 TypeId ⇒ 同 payload 组合 ⇒ 同布局，重复登记幂等。
        if site.step_variants.is_empty()
            || site.step_ty == eff_abi.step_ty
            || site.step_fqn_sym != scoop2_base::Symbol::default()
        {
            continue;
        }
        let mut max_payload_size: u64 = 0;
        let mut max_payload_align: u64 = 1;
        for v in &site.step_variants {
            let (s, a) = sub_size_align(v.payload_ty, types, hir, interner, 0);
            if s > max_payload_size {
                max_payload_size = s;
            }
            if a > max_payload_align {
                max_payload_align = a;
            }
        }
        let tag_size: u64 = 1;
        let payload_offset = align_to(tag_size, max_payload_align);
        let total = align_to(
            payload_offset + max_payload_size,
            max_payload_align.max(tag_size),
        );
        let site_layout = TypeLayout {
            size: total,
            align: max_payload_align.max(tag_size),
            kind: TypeLayoutKind::Enum {
                tag_size,
                tag_offset: 0,
                payload_offset,
                variants: site
                    .step_variants
                    .iter()
                    .enumerate()
                    .map(|(i, v)| EnumVariantLayout {
                        name: v.name.clone(),
                        tag_value: i as u64,
                        slot_offset: payload_offset,
                        payload_ty: Some(v.payload_ty),
                        payload_fields: Vec::new(),
                    })
                    .collect(),
            },
        };
        program.type_layouts.insert(site.step_ty, site_layout.clone());
        program.synthetic_types.push(SyntheticTypeDecl {
            fqn: format!("{}$ccstep{}_{}", fd.fqn, site.block_idx, site.stmt_idx),
            kind: SyntheticTypeKind::StepEnum,
            layout: site_layout,
        });
    }

    // Continuation struct（resuming arm 的 continuation binder 类型）：
    // canonical 布局（见 effect/mod.rs 顶部常量），与 resume lowering 严格一致。
    if program
        .synthetic_types
        .iter()
        .all(|s| s.fqn != format!("{}$continuation", fd.fqn))
    {
        let bool_ty =
            find_scalar_type_in_program(program, |sk| matches!(sk, crate::ScalarKind::Bool))
                .unwrap_or(eff_abi.frame_ty);
        let int_ty =
            find_scalar_type_in_program(program, |sk| matches!(sk, crate::ScalarKind::Int { .. }))
                .unwrap_or(eff_abi.frame_ty);
        let fields =
            crate::effect::canonical_continuation_fields(eff_abi.frame_ty, bool_ty, int_ty)
                .into_iter()
                .map(|f| {
                    let size = match f.kind {
                        crate::ContinuationFieldKind::Header => {
                            crate::effect::OBJECT_HEADER_SIZE_BYTES
                        }
                        crate::ContinuationFieldKind::ResumedFlag => 1,
                        _ => 8,
                    };
                    FieldLayout {
                        offset: f.offset,
                        size,
                        ty: f.ty,
                    }
                })
                .collect();
        let cont_layout = TypeLayout {
            size: crate::effect::CONT_SIZE_BYTES,
            align: crate::effect::CONT_ALIGN_BYTES,
            kind: TypeLayoutKind::Struct { fields },
        };
        program.synthetic_types.push(SyntheticTypeDecl {
            fqn: format!("{}$continuation", fd.fqn),
            kind: SyntheticTypeKind::ContinuationStruct,
            layout: cont_layout,
        });
    }
}

/// 内建标量名 → ScalarKind 映射（识别 @Intrinsic struct 的标量别名）。
fn builtin_scalar_kind(simple: &str) -> Option<crate::ScalarKind> {
    use crate::ScalarKind;
    Some(match simple {
        "Unit" => ScalarKind::Unit,
        "Bool" => ScalarKind::Bool,
        "Char" => ScalarKind::Char,
        "Int" => ScalarKind::Int {
            bits: 64,
            unsigned: false,
        },
        "UInt" => ScalarKind::Int {
            bits: 64,
            unsigned: true,
        },
        "Int8" => ScalarKind::Int {
            bits: 8,
            unsigned: false,
        },
        "Int16" => ScalarKind::Int {
            bits: 16,
            unsigned: false,
        },
        "Int32" => ScalarKind::Int {
            bits: 32,
            unsigned: false,
        },
        "Int64" | "Long" => ScalarKind::Int {
            bits: 64,
            unsigned: false,
        },
        "UInt8" | "Byte" => ScalarKind::Int {
            bits: 8,
            unsigned: true,
        },
        "UInt16" | "UShort" => ScalarKind::Int {
            bits: 16,
            unsigned: true,
        },
        "UInt32" => ScalarKind::Int {
            bits: 32,
            unsigned: true,
        },
        "UInt64" | "ULong" => ScalarKind::Int {
            bits: 64,
            unsigned: true,
        },
        "UIntPtr" => ScalarKind::Int {
            bits: 64,
            unsigned: true,
        },
        "Float64" | "Double" => ScalarKind::Float { bits: 64 },
        "Float32" => ScalarKind::Float { bits: 32 },
        _ => return None,
    })
}

/// 为 ScalarKind 生成 TypeLayout。
fn scalar_layout(kind: crate::ScalarKind) -> crate::TypeLayout {
    use crate::ScalarKind;
    let (size, align) = match kind {
        ScalarKind::Unit => (0, 1),
        ScalarKind::Bool => (1, 1),
        ScalarKind::Char => (4, 4),
        ScalarKind::Int { bits, .. } => {
            let bytes = (bits as u64 + 7) / 8;
            (bytes.max(1), bytes.max(1))
        }
        ScalarKind::Float { bits: 32 } => (4, 4),
        ScalarKind::Float { bits: 64 } => (8, 8),
        ScalarKind::Float { bits } => ((bits as u64 + 7) / 8, (bits as u64 + 7) / 8),
    };
    crate::TypeLayout {
        size,
        align,
        kind: crate::TypeLayoutKind::Scalar { scalar_kind: kind },
    }
}
