//! GC 信息生成：safepoint root map + 类型描述符。

use scoop2_base::Interner;
use scoop2_hir::hir::TypedHir;
use scoop2_hir::ty::TypeId;
use scoop2_mir::mir::{materialize::MaterializedMir, Body, Rvalue, StatementKind, TerminatorKind};

use crate::*;

/// 判定一个类型是否需要 GC 跟踪。
pub fn is_gc_traceable_type(ty: TypeId, layouts: &TypeLayoutTable) -> bool {
    match layouts.get(ty) {
        Some(layout) => match &layout.kind {
            TypeLayoutKind::Reference { gc_traceable, .. } => *gc_traceable,
            TypeLayoutKind::Function => true,
            _ => false,
        },
        None => false,
    }
}

/// 主入口：生成 GC 类型描述符。
pub fn generate_gc_info(
    program: &mut LirProgram,
    mir: &MaterializedMir,
    hir: &TypedHir,
    interner: &Interner,
) {
    let mut next_type_id: u64 = 1;
    let mut class_type_ids: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    // 为每个 class 生成 TypeDescriptor，含 trace_offsets（GC 指针字段偏移）。
    for cv in &mir.backend_contracts.class_vtables {
        let type_id = next_type_id;
        next_type_id += 1;
        class_type_ids.insert(cv.class_fqn.clone(), type_id);

        // 计算 trace_offsets：遍历 class 的字段，找出 GC 引用字段的偏移。
        let trace_offsets = compute_class_trace_offsets(&cv.class_fqn, hir, interner, &program.type_layouts);
        // 计算 class 布局大小。
        let (size, align) = compute_class_layout_size(&cv.class_fqn, hir, interner, &program.type_layouts);
        // 查找父类 type_id：HIR 的 `direct_subtypes` 映射是「父类 → 子类列表」，
        // 无法直接反查「子类 → 父类」。要恢复父类需要在 typecheck 阶段额外导出
        // `supertypes`（当前 TypedHir 未暴露）。此处先填 None；后续若 HIR 暴露
        // supertypes，可在此查 `class_type_ids[父类 FQN]` 写入 parent_type_id。
        let parent_type_id = compute_parent_type_id(&cv.class_fqn, hir, interner, &class_type_ids);

        program.type_descriptors.push(TypeDescriptor {
            type_fqn: cv.class_fqn.clone(),
            size,
            align,
            trace_offsets,
            // @ReleaseHook 函数符号：MIR 当前不暴露注解（annotations 未进入
            // BackendContracts / module items）。待 release-hook 信息从 typecheck
            // 导出后，可在此按 class FQN 查其 @ReleaseHook 方法并 mangle 符号。
            release_fn: None,
            type_id,
            parent_type_id,
        });
    }

    // 为每个 interface 生成 TypeDescriptor。
    for ic in &mir.backend_contracts.interfaces {
        if program.type_descriptors.iter().any(|td| td.type_fqn == ic.interface_fqn) {
            continue;
        }
        let type_id = next_type_id;
        next_type_id += 1;
        program.type_descriptors.push(TypeDescriptor {
            type_fqn: ic.interface_fqn.clone(),
            size: 8,
            align: 8,
            trace_offsets: Vec::new(),
            // interface 无 release hook / 无父类（接口继承在 Scoop 中通过 union 体现）。
            release_fn: None,
            type_id,
            parent_type_id: None,
        });
    }
}

/// 计算 class 的 GC 指针字段偏移列表。
fn compute_class_trace_offsets(
    class_fqn: &str,
    hir: &TypedHir,
    interner: &Interner,
    layouts: &TypeLayoutTable,
) -> Vec<u64> {
    let mut offsets: Vec<u64> = Vec::new();
    let fqn_sym = interner.get(class_fqn);
    if let Some(sym) = fqn_sym {
        if let Some(members) = hir.members.get(&sym) {
            // GC 对象头占 8 字节，字段从 offset 8 开始。
            let mut field_offset: u64 = 8;
            for (_, &member_ty) in members {
                let (field_size, field_align) = layouts.get(member_ty)
                    .map(|l| (l.size, l.align))
                    .unwrap_or((8, 8));
                // 对齐到字段的对齐要求。
                field_offset = align_to(field_offset, field_align);
                if is_gc_traceable_type(member_ty, layouts) {
                    offsets.push(field_offset);
                }
                field_offset += field_size;
            }
        }
    }
    offsets
}

/// 把 offset 向上对齐到 align 的倍数。
fn align_to(offset: u64, align: u64) -> u64 {
    if align <= 1 { return offset; }
    let mask = align - 1;
    (offset + mask) & !mask
}

/// 计算 class 的布局大小（GC 对象头 + 字段）。
fn compute_class_layout_size(
    class_fqn: &str,
    hir: &TypedHir,
    interner: &Interner,
    layouts: &TypeLayoutTable,
) -> (u64, u64) {
    let header_size: u64 = 8; // GC 对象头
    let header_align: u64 = 8;
    let mut total: u64 = header_size;
    let fqn_sym = interner.get(class_fqn);
    if let Some(sym) = fqn_sym {
        if let Some(members) = hir.members.get(&sym) {
            for (_, &member_ty) in members {
                let field_size = layouts.get(member_ty).map(|l| l.size).unwrap_or(8);
                total += field_size;
            }
        }
    }
    (total.max(header_size), header_align)
}

/// 查找 class 的父类 type_id。
///
/// HIR 的 `direct_subtypes` 是「父类 Symbol → 子类 Symbol 列表」映射。要找 `class_fqn`
/// 的父类，需扫描所有 entries：若 `class_fqn` 出现在某个父类的子类列表中，则该父类即
/// 为所求。找到父类后用其 FQN 在 `class_type_ids` 中查 type_id。未找到（无父类 / 父类
/// 未发布）返回 None。
fn compute_parent_type_id(
    class_fqn: &str,
    hir: &TypedHir,
    interner: &Interner,
    class_type_ids: &std::collections::HashMap<String, u64>,
) -> Option<u64> {
    let child_sym = interner.get(class_fqn)?;
    for (&parent_sym, children) in &hir.direct_subtypes {
        if children.contains(&child_sym) {
            let parent_fqn = interner.resolve(parent_sym);
            return class_type_ids.get(parent_fqn).copied();
        }
    }
    None
}

/// 计算函数体的 GC 信息：safepoint 列表 + 存活的 GC local。
///
/// `frame_ty`：若该函数是 EffectStep 函数，传入其 frame tuple 类型；函数会把
/// frame tuple 中 gc_traceable 的元素也登记为额外的 GcLocal（base_local 指向
/// frame local），以便 codegen 在 safepoint 处把它们作为 frame 内部 root 跟踪。
pub fn compute_gc_info_for_body(
    body: &Body,
    _fqn: &str,
    layouts: &TypeLayoutTable,
    frame_ty: Option<(scoop2_hir::ty::TypeId, scoop2_mir::mir::LocalId)>,
) -> GcInfo {
    // 1. GC local 列表。
    let mut gc_locals: Vec<GcLocal> = body.locals.iter().enumerate()
        .filter(|(_, d)| is_gc_traceable_type(d.ty, layouts))
        .map(|(i, d)| GcLocal {
            local_id: i as u32,
            ty: d.ty,
            base_local: None,
        })
        .collect();
    // 1b. EffectStep 的 frame tuple GC 槽：frame tuple 本身是一个 aggregate local，
    //     其内含的 gc_traceable 元素需要作为额外 root 暴露给 GC。这里为 frame 的每个
    //     gc_traceable 元素追加一条 GcLocal，base_local 指向 frame local，local_id
    //     用一个超出 body locals 范围的合成 ID（u32::MAX - index），避免与真实 local
    //     冲突。codegen 据此在 safepoint 处把 frame 内的 GC 指针作为 interior root。
    //
    // 这些 frame interior root 在整个 EffectStep 生命周期内始终存活（frame 持续存在
    // 到 Complete），因此不参与 MIR 的 per-block liveness 分析——它们在每个 safepoint
    // 都被无条件加入 live 集合。
    let mut frame_interior_ids: Vec<u32> = Vec::new();
    if let Some((fty, frame_local)) = frame_ty {
        if let Some(layout) = layouts.get(fty) {
            if let TypeLayoutKind::Tuple { elements } = &layout.kind {
                let base = frame_local.0;
                for (i, f) in elements.iter().enumerate() {
                    if is_gc_traceable_type(f.ty, layouts) {
                        let id = u32::MAX - i as u32;
                        frame_interior_ids.push(id);
                        gc_locals.push(GcLocal {
                            // 合成 interior local id，避开真实 local id 区间。
                            local_id: id,
                            ty: f.ty,
                            base_local: Some(base),
                        });
                    }
                }
            }
        }
    }
    // 仅真实 local 参与 per-block liveness（合成 interior id 不在 MIR LocalId 空间内）。
    let gc_local_ids: std::collections::HashSet<u32> = gc_locals.iter()
        .map(|g| g.local_id)
        .filter(|&id| !frame_interior_ids.contains(&id))
        .collect();

    // 2. 计算 liveness：使用 MIR 的 compute_live_out 得到每个块的 live-out 集合。
    let live_out = scoop2_mir::mir::effect_lower::analyze::compute_live_out(body);

    // 3. Safepoint：每个 call 点 + 每个 effect 挂起点。
    //    live_gc_locals = (gc_local_ids ∩ live_out[当前块]) ∪ frame_interior_ids
    //    即：在当前块出口处存活的 GC local，加上始终存活的 frame interior root。
    //
    // 注：SafepointKind::Poll（循环回边处的纯 GC 轮询）当前未发射。正确发射需要
    // 识别循环头（存在序号大于自身的后向前驱的块），而 MIR body 此处以 RPO 顺序
    // 存储、无显式循环信息。待后端（codegen）在构造 CFG 时按机器码布局识别回边
    // 并插入 Poll 更合适——此处仅产出语义 safepoint（Call / EffectSuspend）。
    let mut safepoints: Vec<GcSafepoint> = Vec::new();
    for (bi, block) in body.blocks.iter().enumerate() {
        let block_live: std::collections::HashSet<scoop2_mir::mir::LocalId> =
            live_out.get(bi).cloned().unwrap_or_default();
        let mut live_gc: Vec<u32> = gc_local_ids.iter()
            .filter(|&&id| block_live.contains(&scoop2_mir::mir::LocalId(id)))
            .copied()
            .collect();
        // frame interior root 始终存活，无条件加入。
        live_gc.extend_from_slice(&frame_interior_ids);
        live_gc.sort_unstable();
        live_gc.dedup();
        for (si, stmt) in block.stmts.iter().enumerate() {
            if let StatementKind::Assign { value, .. } = &stmt.kind {
                if let Some(callee_symbol) = extract_call_callee(value) {
                    safepoints.push(GcSafepoint {
                        block_id: bi as u32,
                        stmt_index: si as u32,
                        kind: SafepointKind::Call { callee_symbol },
                        live_gc_locals: live_gc.clone(),
                    });
                }
            }
        }
        if let TerminatorKind::Perform { .. } = &block.terminator.kind {
            safepoints.push(GcSafepoint {
                block_id: bi as u32,
                stmt_index: block.stmts.len() as u32,
                kind: SafepointKind::EffectSuspend,
                live_gc_locals: live_gc.clone(),
            });
        }
    }

    GcInfo { gc_locals, safepoints }
}

fn extract_call_callee(rv: &Rvalue) -> Option<String> {
    if let Rvalue::Call { kind, .. } = rv {
        use scoop2_mir::mir::CallKind;
        match kind {
            CallKind::Direct { callee_fqn, .. } => Some(callee_fqn.clone()),
            CallKind::Closure { invoke_fqn, .. } => Some(invoke_fqn.clone()),
            CallKind::Virtual { dispatch, .. } => {
                Some(format!("<virtual:{}.{}>", dispatch.owner_fqn, dispatch.member_name))
            }
            CallKind::Interface { dispatch, .. } => Some(format!(
                "<interface:{}.{}>",
                dispatch.owner_fqn, dispatch.member_name
            )),
            CallKind::FunValue { .. } => Some("<fun_value>".to_string()),
            CallKind::Resume { .. } => Some("<resume>".to_string()),
        }
    } else {
        None
    }
}
