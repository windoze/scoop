#![allow(dead_code)]

use std::collections::BTreeMap;

use inkwell::types::{BasicTypeEnum, FunctionType, StructType};

use crate::effect_facts::{CaseTag, ContinuationSchemaId, StepSchemaId};
use crate::effect_lowered::ir::{
    ContinuationObjectId, FrameSlotId, ResumeInterfaceId, SystemSlotKind,
};
use crate::ty::TypeId;

/// 单个 refactor ABI 值位的 LLVM 形状。
///
/// `elided=true` 表示该值在 function ABI 中可被省略；但若它出现在 frame/step payload field 中，
/// 仍可能用零大小 struct 保留稳定 field index。
#[derive(Clone, Copy)]
pub(super) struct RefactorAbiValue<'ctx> {
    llvm_ty: BasicTypeEnum<'ctx>,
    elided: bool,
}

impl<'ctx> RefactorAbiValue<'ctx> {
    pub(super) fn new(llvm_ty: BasicTypeEnum<'ctx>, elided: bool) -> Self {
        Self { llvm_ty, elided }
    }

    pub(super) fn llvm_ty(&self) -> BasicTypeEnum<'ctx> {
        self.llvm_ty
    }

    pub(super) fn is_elided(&self) -> bool {
        self.elided
    }
}

/// `Step_F` 的单个 variant 布局。
pub(super) struct RefactorStepVariantLayout<'ctx> {
    tag_value: u32,
    payload_ty: StructType<'ctx>,
    payload_field_count: usize,
    payload_anchor_name: String,
    payload_is_elided: bool,
}

impl<'ctx> RefactorStepVariantLayout<'ctx> {
    pub(super) fn new(
        tag_value: u32,
        payload_ty: StructType<'ctx>,
        payload_field_count: usize,
        payload_anchor_name: String,
        payload_is_elided: bool,
    ) -> Self {
        Self {
            tag_value,
            payload_ty,
            payload_field_count,
            payload_anchor_name,
            payload_is_elided,
        }
    }

    pub(super) fn tag_value(&self) -> u32 {
        self.tag_value
    }

    pub(super) fn payload_ty(&self) -> StructType<'ctx> {
        self.payload_ty
    }

    pub(super) fn payload_field_count(&self) -> usize {
        self.payload_field_count
    }

    pub(super) fn payload_anchor_name(&self) -> &str {
        &self.payload_anchor_name
    }

    pub(super) fn payload_is_elided(&self) -> bool {
        self.payload_is_elided
    }
}

/// `Step_F` 中某个 canonical outward case 的 LLVM 布局。
pub(super) struct RefactorStepCaseLayout<'ctx> {
    case_tag: CaseTag,
    tag_constant_name: String,
    variant: RefactorStepVariantLayout<'ctx>,
}

impl<'ctx> RefactorStepCaseLayout<'ctx> {
    pub(super) fn new(
        case_tag: CaseTag,
        tag_constant_name: String,
        variant: RefactorStepVariantLayout<'ctx>,
    ) -> Self {
        Self {
            case_tag,
            tag_constant_name,
            variant,
        }
    }

    pub(super) fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub(super) fn tag_constant_name(&self) -> &str {
        &self.tag_constant_name
    }

    pub(super) fn variant(&self) -> &RefactorStepVariantLayout<'ctx> {
        &self.variant
    }
}

/// 单个 `StepSchemaId` 对应的 canonical `Step_F` 布局。
pub(super) struct RefactorStepLayout<'ctx> {
    step_schema: StepSchemaId,
    llvm_ty: StructType<'ctx>,
    layout_anchor_name: String,
    complete_tag_constant_name: String,
    complete_variant: RefactorStepVariantLayout<'ctx>,
    cases: BTreeMap<CaseTag, RefactorStepCaseLayout<'ctx>>,
}

impl<'ctx> RefactorStepLayout<'ctx> {
    pub(super) fn new(
        step_schema: StepSchemaId,
        llvm_ty: StructType<'ctx>,
        layout_anchor_name: String,
        complete_tag_constant_name: String,
        complete_variant: RefactorStepVariantLayout<'ctx>,
        cases: BTreeMap<CaseTag, RefactorStepCaseLayout<'ctx>>,
    ) -> Self {
        Self {
            step_schema,
            llvm_ty,
            layout_anchor_name,
            complete_tag_constant_name,
            complete_variant,
            cases,
        }
    }

    pub(super) fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub(super) fn llvm_ty(&self) -> StructType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn layout_anchor_name(&self) -> &str {
        &self.layout_anchor_name
    }

    pub(super) fn complete_tag_constant_name(&self) -> &str {
        &self.complete_tag_constant_name
    }

    pub(super) fn complete_variant(&self) -> &RefactorStepVariantLayout<'ctx> {
        &self.complete_variant
    }

    pub(super) fn cases(&self) -> &BTreeMap<CaseTag, RefactorStepCaseLayout<'ctx>> {
        &self.cases
    }

    pub(super) fn case_layout(&self, case_tag: CaseTag) -> Option<&RefactorStepCaseLayout<'ctx>> {
        self.cases.get(&case_tag)
    }
}

/// frame 内单个 field 的稳定分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefactorFrameFieldKind {
    Header,
    Slot(FrameSlotId),
}

/// 单个 frame field 的 LLVM 布局。
pub(super) struct RefactorFrameFieldLayout<'ctx> {
    field_index: u32,
    kind: RefactorFrameFieldKind,
    llvm_ty: BasicTypeEnum<'ctx>,
}

impl<'ctx> RefactorFrameFieldLayout<'ctx> {
    pub(super) fn new(
        field_index: u32,
        kind: RefactorFrameFieldKind,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> Self {
        Self {
            field_index,
            kind,
            llvm_ty,
        }
    }

    pub(super) fn field_index(&self) -> u32 {
        self.field_index
    }

    pub(super) fn kind(&self) -> RefactorFrameFieldKind {
        self.kind
    }

    pub(super) fn llvm_ty(&self) -> BasicTypeEnum<'ctx> {
        self.llvm_ty
    }
}

/// 单个 callable version 的 frame 布局查询面。
pub(super) struct RefactorFrameLayout<'ctx> {
    step_schema: StepSchemaId,
    llvm_ty: StructType<'ctx>,
    layout_anchor_name: String,
    fields: Vec<RefactorFrameFieldLayout<'ctx>>,
    slot_field_indices: BTreeMap<FrameSlotId, u32>,
    system_field_indices: BTreeMap<SystemSlotKind, u32>,
}

impl<'ctx> RefactorFrameLayout<'ctx> {
    pub(super) fn new(
        step_schema: StepSchemaId,
        llvm_ty: StructType<'ctx>,
        layout_anchor_name: String,
        fields: Vec<RefactorFrameFieldLayout<'ctx>>,
        slot_field_indices: BTreeMap<FrameSlotId, u32>,
        system_field_indices: BTreeMap<SystemSlotKind, u32>,
    ) -> Self {
        Self {
            step_schema,
            llvm_ty,
            layout_anchor_name,
            fields,
            slot_field_indices,
            system_field_indices,
        }
    }

    pub(super) fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub(super) fn llvm_ty(&self) -> StructType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn layout_anchor_name(&self) -> &str {
        &self.layout_anchor_name
    }

    pub(super) fn fields(&self) -> &[RefactorFrameFieldLayout<'ctx>] {
        &self.fields
    }

    pub(super) fn field_index_for_slot(&self, slot_id: FrameSlotId) -> Option<u32> {
        self.slot_field_indices.get(&slot_id).copied()
    }

    pub(super) fn field_index_for_system(&self, kind: SystemSlotKind) -> Option<u32> {
        self.system_field_indices.get(&kind).copied()
    }
}

/// callable entry（dynamic/direct invoke）共享的 LLVM 函数签名。
pub(super) struct RefactorCallableEntryLayout<'ctx> {
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    args_abi: RefactorAbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

impl<'ctx> RefactorCallableEntryLayout<'ctx> {
    pub(super) fn new(
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        args_abi: RefactorAbiValue<'ctx>,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            symbol_name,
            llvm_ty,
            param_count,
            args_abi,
            return_step_schema,
        }
    }

    pub(super) fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    pub(super) fn llvm_ty(&self) -> FunctionType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn param_count(&self) -> usize {
        self.param_count
    }

    pub(super) fn args_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.args_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// 源码可见 `Continuation.resume(...) -> Step_F` 的 LLVM 级合同。
pub(super) struct RefactorContinuationSurfaceResumeLayout<'ctx> {
    continuation_schema: ContinuationSchemaId,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    resume_payload_abi: RefactorAbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

impl<'ctx> RefactorContinuationSurfaceResumeLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        continuation_schema: ContinuationSchemaId,
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        resume_payload_abi: RefactorAbiValue<'ctx>,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            continuation_schema,
            symbol_name,
            llvm_ty,
            param_count,
            resume_tuple_ty,
            answer_ty,
            resume_payload_abi,
            return_step_schema,
        }
    }

    pub(super) fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub(super) fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    pub(super) fn llvm_ty(&self) -> FunctionType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn param_count(&self) -> usize {
        self.param_count
    }

    pub(super) fn resume_tuple_ty(&self) -> TypeId {
        self.resume_tuple_ty
    }

    pub(super) fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub(super) fn resume_payload_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.resume_payload_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// 单个 resume method 的 LLVM 级合同。
pub(super) struct RefactorResumeMethodLayout<'ctx> {
    interface_id: ResumeInterfaceId,
    case_tag: CaseTag,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    vtable_index: u32,
    resume_payload_abi: RefactorAbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

impl<'ctx> RefactorResumeMethodLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        interface_id: ResumeInterfaceId,
        case_tag: CaseTag,
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        vtable_index: u32,
        resume_payload_abi: RefactorAbiValue<'ctx>,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            interface_id,
            case_tag,
            symbol_name,
            llvm_ty,
            param_count,
            vtable_index,
            resume_payload_abi,
            return_step_schema,
        }
    }

    pub(super) fn interface_id(&self) -> ResumeInterfaceId {
        self.interface_id
    }

    pub(super) fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub(super) fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    pub(super) fn llvm_ty(&self) -> FunctionType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn param_count(&self) -> usize {
        self.param_count
    }

    pub(super) fn vtable_index(&self) -> u32 {
        self.vtable_index
    }

    pub(super) fn resume_payload_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.resume_payload_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// 单个 internal resume interface 的 vtable / method 布局。
pub(super) struct RefactorResumeInterfaceLayout<'ctx> {
    interface_id: ResumeInterfaceId,
    effect_family_fqn: String,
    llvm_vtable_ty: StructType<'ctx>,
    layout_anchor_name: String,
    methods: BTreeMap<CaseTag, RefactorResumeMethodLayout<'ctx>>,
}

impl<'ctx> RefactorResumeInterfaceLayout<'ctx> {
    pub(super) fn new(
        interface_id: ResumeInterfaceId,
        effect_family_fqn: String,
        llvm_vtable_ty: StructType<'ctx>,
        layout_anchor_name: String,
        methods: BTreeMap<CaseTag, RefactorResumeMethodLayout<'ctx>>,
    ) -> Self {
        Self {
            interface_id,
            effect_family_fqn,
            llvm_vtable_ty,
            layout_anchor_name,
            methods,
        }
    }

    pub(super) fn interface_id(&self) -> ResumeInterfaceId {
        self.interface_id
    }

    pub(super) fn effect_family_fqn(&self) -> &str {
        &self.effect_family_fqn
    }

    pub(super) fn llvm_vtable_ty(&self) -> StructType<'ctx> {
        self.llvm_vtable_ty
    }

    pub(super) fn layout_anchor_name(&self) -> &str {
        &self.layout_anchor_name
    }

    pub(super) fn methods(&self) -> &BTreeMap<CaseTag, RefactorResumeMethodLayout<'ctx>> {
        &self.methods
    }

    pub(super) fn method(&self, case_tag: CaseTag) -> Option<&RefactorResumeMethodLayout<'ctx>> {
        self.methods.get(&case_tag)
    }
}

/// continuation object 上单个 surface `resume(...)` 的已发布映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RefactorContinuationSurfaceResumeBinding {
    continuation_schema: ContinuationSchemaId,
    return_step_schema: StepSchemaId,
}

impl RefactorContinuationSurfaceResumeBinding {
    pub(super) fn new(
        continuation_schema: ContinuationSchemaId,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            continuation_schema,
            return_step_schema,
        }
    }

    pub(super) fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// continuation object field 的稳定分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefactorContinuationFieldKind {
    Header,
    CapturedFrame,
    ResumeStateTag,
    OneShotFlag,
    InterfaceVtable(ResumeInterfaceId),
}

/// 单个 continuation object field 的 LLVM 布局。
pub(super) struct RefactorContinuationFieldLayout<'ctx> {
    field_index: u32,
    kind: RefactorContinuationFieldKind,
    llvm_ty: BasicTypeEnum<'ctx>,
}

impl<'ctx> RefactorContinuationFieldLayout<'ctx> {
    pub(super) fn new(
        field_index: u32,
        kind: RefactorContinuationFieldKind,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> Self {
        Self {
            field_index,
            kind,
            llvm_ty,
        }
    }

    pub(super) fn field_index(&self) -> u32 {
        self.field_index
    }

    pub(super) fn kind(&self) -> RefactorContinuationFieldKind {
        self.kind
    }

    pub(super) fn llvm_ty(&self) -> BasicTypeEnum<'ctx> {
        self.llvm_ty
    }
}

/// 单个 continuation object 的 LLVM 布局。
pub(super) struct RefactorContinuationObjectLayout<'ctx> {
    object_id: ContinuationObjectId,
    owner_step_schema: StepSchemaId,
    llvm_ty: StructType<'ctx>,
    layout_anchor_name: String,
    fields: Vec<RefactorContinuationFieldLayout<'ctx>>,
    interface_field_indices: BTreeMap<ResumeInterfaceId, u32>,
    surface_resume_bindings:
        BTreeMap<ContinuationSchemaId, Vec<RefactorContinuationSurfaceResumeBinding>>,
}

impl<'ctx> RefactorContinuationObjectLayout<'ctx> {
    pub(super) fn new(
        object_id: ContinuationObjectId,
        owner_step_schema: StepSchemaId,
        llvm_ty: StructType<'ctx>,
        layout_anchor_name: String,
        fields: Vec<RefactorContinuationFieldLayout<'ctx>>,
        interface_field_indices: BTreeMap<ResumeInterfaceId, u32>,
        surface_resume_bindings: BTreeMap<
            ContinuationSchemaId,
            Vec<RefactorContinuationSurfaceResumeBinding>,
        >,
    ) -> Self {
        Self {
            object_id,
            owner_step_schema,
            llvm_ty,
            layout_anchor_name,
            fields,
            interface_field_indices,
            surface_resume_bindings,
        }
    }

    pub(super) fn object_id(&self) -> ContinuationObjectId {
        self.object_id
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn llvm_ty(&self) -> StructType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn layout_anchor_name(&self) -> &str {
        &self.layout_anchor_name
    }

    pub(super) fn fields(&self) -> &[RefactorContinuationFieldLayout<'ctx>] {
        &self.fields
    }

    pub(super) fn field_index_for_interface(&self, interface_id: ResumeInterfaceId) -> Option<u32> {
        self.interface_field_indices.get(&interface_id).copied()
    }

    pub(super) fn surface_resume_bindings(
        &self,
        continuation_schema: ContinuationSchemaId,
    ) -> Option<&[RefactorContinuationSurfaceResumeBinding]> {
        self.surface_resume_bindings
            .get(&continuation_schema)
            .map(Vec::as_slice)
    }
}

/// 单个 callable version 暴露给后续 body emitter 的 LLVM ABI 查询面。
pub(super) struct RefactorCallableLayout<'ctx> {
    root_fqn: String,
    step_schema: StepSchemaId,
    dynamic_entry: RefactorCallableEntryLayout<'ctx>,
    direct_entry: RefactorCallableEntryLayout<'ctx>,
    continuation_object: ContinuationObjectId,
    resume_interfaces: Vec<ResumeInterfaceId>,
}

impl<'ctx> RefactorCallableLayout<'ctx> {
    pub(super) fn new(
        root_fqn: String,
        step_schema: StepSchemaId,
        dynamic_entry: RefactorCallableEntryLayout<'ctx>,
        direct_entry: RefactorCallableEntryLayout<'ctx>,
        continuation_object: ContinuationObjectId,
        resume_interfaces: Vec<ResumeInterfaceId>,
    ) -> Self {
        Self {
            root_fqn,
            step_schema,
            dynamic_entry,
            direct_entry,
            continuation_object,
            resume_interfaces,
        }
    }

    pub(super) fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub(super) fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub(super) fn dynamic_entry(&self) -> &RefactorCallableEntryLayout<'ctx> {
        &self.dynamic_entry
    }

    pub(super) fn direct_entry(&self) -> &RefactorCallableEntryLayout<'ctx> {
        &self.direct_entry
    }

    pub(super) fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }

    pub(super) fn resume_interfaces(&self) -> &[ResumeInterfaceId] {
        &self.resume_interfaces
    }
}

/// refactor LLVM type/layout 层对下游 body emitter 暴露的稳定查询面。
pub(crate) struct RefactorAbiQuery<'ctx> {
    step_layouts: BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    frame_layouts: BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
    continuation_layouts: BTreeMap<ContinuationObjectId, RefactorContinuationObjectLayout<'ctx>>,
    resume_interface_layouts: BTreeMap<ResumeInterfaceId, RefactorResumeInterfaceLayout<'ctx>>,
    surface_resume_layouts:
        BTreeMap<ContinuationSchemaId, RefactorContinuationSurfaceResumeLayout<'ctx>>,
    callable_layouts: BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
}

impl<'ctx> RefactorAbiQuery<'ctx> {
    pub(super) fn new(
        step_layouts: BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        frame_layouts: BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
        continuation_layouts: BTreeMap<
            ContinuationObjectId,
            RefactorContinuationObjectLayout<'ctx>,
        >,
        resume_interface_layouts: BTreeMap<ResumeInterfaceId, RefactorResumeInterfaceLayout<'ctx>>,
        surface_resume_layouts: BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
        callable_layouts: BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
    ) -> Self {
        Self {
            step_layouts,
            frame_layouts,
            continuation_layouts,
            resume_interface_layouts,
            surface_resume_layouts,
            callable_layouts,
        }
    }

    pub(super) fn step_layout(
        &self,
        step_schema: StepSchemaId,
    ) -> Option<&RefactorStepLayout<'ctx>> {
        self.step_layouts.get(&step_schema)
    }

    pub(super) fn frame_layout(
        &self,
        step_schema: StepSchemaId,
    ) -> Option<&RefactorFrameLayout<'ctx>> {
        self.frame_layouts.get(&step_schema)
    }

    pub(super) fn continuation_layout(
        &self,
        object_id: ContinuationObjectId,
    ) -> Option<&RefactorContinuationObjectLayout<'ctx>> {
        self.continuation_layouts.get(&object_id)
    }

    pub(super) fn resume_interface_layout(
        &self,
        interface_id: ResumeInterfaceId,
    ) -> Option<&RefactorResumeInterfaceLayout<'ctx>> {
        self.resume_interface_layouts.get(&interface_id)
    }

    pub(super) fn surface_resume_layout(
        &self,
        continuation_schema: ContinuationSchemaId,
    ) -> Option<&RefactorContinuationSurfaceResumeLayout<'ctx>> {
        self.surface_resume_layouts.get(&continuation_schema)
    }

    pub(super) fn callable_layout(
        &self,
        step_schema: StepSchemaId,
    ) -> Option<&RefactorCallableLayout<'ctx>> {
        self.callable_layouts.get(&step_schema)
    }

    pub(super) fn callable_layout_by_root_fqn(
        &self,
        root_fqn: &str,
    ) -> Option<&RefactorCallableLayout<'ctx>> {
        self.callable_layouts
            .values()
            .find(|layout| layout.root_fqn() == root_fqn)
    }
}
