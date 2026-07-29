//! Effect step 准备：FrameSchema / StepLayout / ContinuationLayout。
//!
//! 遍历带 `effect_abi` 的 EffectStep 函数，构建 frame schema（状态机 frame 的字段表）、
//! step layout（Step tagged union 的变体表）和 continuation layout（resuming continuation 对象布局）。

use scoop2_base::Interner;
use scoop2_hir::hir::TypedHir;
use scoop2_mir::mir::{materialize::MaterializedMir, EffectStepAbi};
use scoop2_hir::ty::TypeId;

use crate::*;
use crate::gc::is_gc_traceable_type;

/// 主入口：确保 EffectStep 的合成类型布局就绪在 synthetic_types 中。
/// callable 级别的 frame_schema/step_layout/continuation_layout 在 map_callable 中直接调用 prepare_effect_abi 挂载。
pub fn prepare_effect_steps(
    _program: &mut LirProgram,
    _mir: &MaterializedMir,
    _hir: &TypedHir,
    _interner: &Interner,
) {
    // synthetic_types 已在 layout::compute_type_layouts 中通过 prepare_effect_synthetic_layouts 填充。
    // callable 级别的 frame_schema/step_layout/continuation_layout 在 map_callable 中通过
    // prepare_effect_abi 直接计算并挂载到 LirCallable。
    // 此函数作为管线中的幂等检查点保留——确保 synthetic_types 已就绪。
}

/// 为单个 EffectStep 函数构建 (FrameSchema, StepLayout, ContinuationLayout)。
pub fn prepare_effect_abi(
    abi: &EffectStepAbi,
    fqn: &str,
    layouts: &TypeLayoutTable,
    _hir: &TypedHir,
    _interner: &Interner,
) -> (Option<FrameSchema>, Option<StepLayout>, Option<StateDispatch>, Option<ContinuationLayout>) {
    // Frame schema：从 frame_ty 的 tuple 布局提取 slot 列表。
    // 第一个 slot 固定为 State（Int state tag）；其余为 frame tuple 的元素。
    let frame = build_frame_schema(abi, layouts);

    // Step layout：从 step_variants 构建变体列表。
    let mut variants: Vec<StepVariantLayout> = Vec::with_capacity(abi.step_variants.len());
    let mut complete_variant: Option<StepVariantLayout> = None;
    for (i, v) in abi.step_variants.iter().enumerate() {
        let svl = StepVariantLayout {
            name: v.name.clone(),
            tag_value: i as u64,
            payload: Some(v.payload_ty),
        };
        if v.is_complete {
            complete_variant = Some(svl.clone());
        }
        variants.push(svl);
    }
    let complete_variant = complete_variant.unwrap_or_else(|| StepVariantLayout {
        name: "Complete".to_string(),
        tag_value: 0,
        payload: None,
    });
    let step = StepLayout {
        step_ty: abi.step_ty,
        complete_variant,
        effect_variants: variants,
    };

    // Continuation layout：固定字段集合。
    // 各字段使用与其语义匹配的真实类型而非全部用 frame_ty：
    // - Header：GC 对象头（不透明 GC 指针），用 frame_ty（已是 frame 指针类型，
    //   与指针同 size 8B）作为不透明 GC 指针的占位类型。
    // - ResumedFlag：Bool 标志。
    // - ResumeStateTag：Int state tag。
    // - FramePtr：指向 frame tuple 的指针（与指针同 size）。
    // - StepFnPtr：函数指针（与指针同 size）。
    let bool_ty = find_scalar_type(layouts, |sk| matches!(sk, ScalarKind::Bool))
        .unwrap_or(abi.frame_ty);
    let int_ty = state_local_ty_from_layouts(layouts);
    let cont = ContinuationLayout {
        cont_fqn: format!("{}$continuation", fqn),
        fields: vec![
            ContinuationField {
                name: "__header".to_string(),
                offset: 0,
                ty: abi.frame_ty,
                kind: ContinuationFieldKind::Header,
            },
            ContinuationField {
                name: "__resumed".to_string(),
                offset: 8,
                ty: bool_ty,
                kind: ContinuationFieldKind::ResumedFlag,
            },
            ContinuationField {
                name: "__state".to_string(),
                offset: 16,
                ty: int_ty,
                kind: ContinuationFieldKind::ResumeStateTag,
            },
            ContinuationField {
                name: "__frame".to_string(),
                offset: 24,
                ty: abi.frame_ty,
                kind: ContinuationFieldKind::FramePtr,
            },
            ContinuationField {
                name: "__step_fn".to_string(),
                offset: 32,
                ty: abi.frame_ty,
                kind: ContinuationFieldKind::StepFnPtr,
            },
        ],
    };

    // State dispatch：从 EffectStepAbi 构建基本的 state dispatch 信息。
    // state 0 = 初始入口（block 0 = body start）。
    // state N = 第 N 个 outward case 的 resume 续点。
    // 具体 block-id 由 codegen 从 body 的 CondBr dispatch 链推导。
    // 此处提供 state value → outward case 索引的映射。
    let state_dispatch = StateDispatch {
        entries: (0..=abi.step_variants.len() as u32)
            .map(|state| StateDispatchEntry {
                state_value: state,
                block_id: state, // 简化：state value = block-id 占位；codegen 从 body 推导真实 block-id
            })
            .collect(),
    };

    (Some(frame), Some(step), Some(state_dispatch), Some(cont))
}

/// 构建 frame schema：从 frame_ty 的 Tuple 布局提取元素作为 frame slot。
fn build_frame_schema(abi: &EffectStepAbi, layouts: &TypeLayoutTable) -> FrameSchema {
    let mut slots: Vec<FrameSlot> = Vec::new();
    // slot 0: state（Int）。
    slots.push(FrameSlot {
        slot_index: 0,
        kind: FrameSlotKind::State,
        ty: state_local_ty_from_layouts(layouts),
        gc_traceable: false,
    });
    // 后续 slot：从 frame_ty 的 Tuple 布局提取。
    if let Some(layout) = layouts.get(abi.frame_ty) {
        if let TypeLayoutKind::Tuple { elements } = &layout.kind {
            for (i, f) in elements.iter().enumerate() {
                slots.push(FrameSlot {
                    slot_index: (i + 1) as u32,
                    kind: FrameSlotKind::SourceLocal {
                        local_id: i as u32,
                    },
                    ty: f.ty,
                    gc_traceable: is_gc_traceable_type(f.ty, layouts),
                });
            }
        }
    }
    FrameSchema {
        frame_ty: abi.frame_ty,
        slots,
    }
}

/// 从 TypeLayoutTable 查找 Int 类型的 TypeId（用作 state tag 字段类型）。
fn state_local_ty_from_layouts(layouts: &TypeLayoutTable) -> TypeId {
    find_scalar_type(layouts, |sk| matches!(sk, ScalarKind::Int { .. })).unwrap_or(TypeId(0))
}

/// 在布局表中查找首个匹配 `pred` 的 Scalar 类型 TypeId。
fn find_scalar_type(
    layouts: &TypeLayoutTable,
    pred: impl Fn(&ScalarKind) -> bool,
) -> Option<TypeId> {
    layouts.entries.iter()
        .find(|(_, l)| matches!(&l.kind, TypeLayoutKind::Scalar { scalar_kind } if pred(scalar_kind)))
        .map(|(t, _)| *t)
}
