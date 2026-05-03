#![allow(dead_code)]

use std::collections::BTreeMap;

use inkwell::types::{BasicTypeEnum, FunctionType, StructType};

use crate::effect_facts::{
    CallSiteEffectFacts, CallTargetMode, CaseTag, ContinuationSchemaId, StepSchemaId,
};
use crate::effect_lowered::ir::{
    ContinuationObjectId, FrameSlotId, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredHandleBoundaryRouting, LateLoweredHandleDispatchContract,
    LateLoweredHandlePendingCompletion, LateLoweredHandleStateRegion,
    LateLoweredHandleStateRegionEntry, LateLoweredLocalRuntimeErrorTerminalAction,
    LateLoweredPublishedRuntimeEntry, ResumeInterfaceId, StateId, SystemSlotKind,
};
use crate::llvm::LlvmEmitError;
use crate::mir::{LocalId, SiteId};
use crate::ty::TypeId;

/// 单个 refactor ABI 值位的 LLVM 形状。
///
/// `elided=true` 表示该值在 function ABI 中可被省略；但若它出现在 frame/step payload field 中，
/// 仍可能用零大小 struct 保留稳定 field index。
#[derive(Clone, Copy, Debug)]
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

/// refactor source type 在 LLVM ABI 中的稳定 carrier 分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefactorSourceAbiLayoutKind {
    Scalar,
    Tuple,
}

/// tuple-like source carrier 中单个 source field 对应的 ABI field 映射。
#[derive(Debug, Clone, Copy)]
pub(super) struct RefactorSourceAbiFieldLayout<'ctx> {
    source_index: u32,
    source_ty: TypeId,
    abi_field_index: Option<u32>,
    abi: RefactorAbiValue<'ctx>,
}

impl<'ctx> RefactorSourceAbiFieldLayout<'ctx> {
    pub(super) fn new(
        source_index: u32,
        source_ty: TypeId,
        abi_field_index: Option<u32>,
        abi: RefactorAbiValue<'ctx>,
    ) -> Self {
        Self {
            source_index,
            source_ty,
            abi_field_index,
            abi,
        }
    }

    pub(super) fn source_index(&self) -> u32 {
        self.source_index
    }

    pub(super) fn source_ty(&self) -> TypeId {
        self.source_ty
    }

    pub(super) fn abi_field_index(&self) -> Option<u32> {
        self.abi_field_index
    }

    pub(super) fn abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.abi
    }

    pub(super) fn is_elided(&self) -> bool {
        self.abi.is_elided()
    }
}

/// late-lowered source type 到 LLVM ABI value 的 authoritative 查询面。
#[derive(Debug, Clone)]
pub(super) struct RefactorSourceAbiLayout<'ctx> {
    source_ty: TypeId,
    kind: RefactorSourceAbiLayoutKind,
    abi: RefactorAbiValue<'ctx>,
    fields: Vec<RefactorSourceAbiFieldLayout<'ctx>>,
}

impl<'ctx> RefactorSourceAbiLayout<'ctx> {
    pub(super) fn new(
        source_ty: TypeId,
        kind: RefactorSourceAbiLayoutKind,
        abi: RefactorAbiValue<'ctx>,
        fields: Vec<RefactorSourceAbiFieldLayout<'ctx>>,
    ) -> Self {
        Self {
            source_ty,
            kind,
            abi,
            fields,
        }
    }

    pub(super) fn source_ty(&self) -> TypeId {
        self.source_ty
    }

    pub(super) fn kind(&self) -> RefactorSourceAbiLayoutKind {
        self.kind
    }

    pub(super) fn is_tuple(&self) -> bool {
        self.kind == RefactorSourceAbiLayoutKind::Tuple
    }

    pub(super) fn abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.abi
    }

    pub(super) fn fields(&self) -> &[RefactorSourceAbiFieldLayout<'ctx>] {
        &self.fields
    }

    pub(super) fn field(&self, source_index: usize) -> Option<&RefactorSourceAbiFieldLayout<'ctx>> {
        self.fields.get(source_index)
    }

    pub(super) fn abi_field_count(&self) -> usize {
        self.fields
            .iter()
            .filter(|field| field.abi_field_index().is_some())
            .count()
    }
}

/// `Step_F` 的单个 variant 布局。
pub(super) struct RefactorStepVariantLayout<'ctx> {
    tag_value: u32,
    payload_source_ty: TypeId,
    payload_ty: StructType<'ctx>,
    payload_field_count: usize,
    payload_anchor_name: String,
    payload_is_elided: bool,
}

impl<'ctx> RefactorStepVariantLayout<'ctx> {
    pub(super) fn new(
        tag_value: u32,
        payload_source_ty: TypeId,
        payload_ty: StructType<'ctx>,
        payload_field_count: usize,
        payload_anchor_name: String,
        payload_is_elided: bool,
    ) -> Self {
        Self {
            tag_value,
            payload_source_ty,
            payload_ty,
            payload_field_count,
            payload_anchor_name,
            payload_is_elided,
        }
    }

    pub(super) fn tag_value(&self) -> u32 {
        self.tag_value
    }

    pub(super) fn payload_source_ty(&self) -> TypeId {
        self.payload_source_ty
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
    invoke_args_tuple_ty: TypeId,
    args_abi: RefactorAbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

impl<'ctx> RefactorCallableEntryLayout<'ctx> {
    pub(super) fn new(
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        invoke_args_tuple_ty: TypeId,
        args_abi: RefactorAbiValue<'ctx>,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            symbol_name,
            llvm_ty,
            param_count,
            invoke_args_tuple_ty,
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

    pub(super) fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub(super) fn args_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.args_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// closure-like runtime callable object 的 carrier 布局。
#[derive(Debug)]
pub(super) struct RefactorClosureCarrierLayout<'ctx> {
    object_ty: StructType<'ctx>,
    receiver_abi: RefactorAbiValue<'ctx>,
    env_field_index: u32,
    fn_field_index: u32,
}

impl<'ctx> RefactorClosureCarrierLayout<'ctx> {
    pub(super) fn new(
        object_ty: StructType<'ctx>,
        receiver_abi: RefactorAbiValue<'ctx>,
        env_field_index: u32,
        fn_field_index: u32,
    ) -> Self {
        Self {
            object_ty,
            receiver_abi,
            env_field_index,
            fn_field_index,
        }
    }

    pub(super) fn object_ty(&self) -> StructType<'ctx> {
        self.object_ty
    }

    pub(super) fn receiver_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.receiver_abi
    }

    pub(super) fn env_field_index(&self) -> u32 {
        self.env_field_index
    }

    pub(super) fn fn_field_index(&self) -> u32 {
        self.fn_field_index
    }
}

/// virtual/interface dispatch receiver 的 authoritative carrier 布局。
#[derive(Debug)]
pub(super) struct RefactorDispatchReceiverLayout<'ctx> {
    receiver_ty: TypeId,
    receiver_abi: RefactorAbiValue<'ctx>,
    owner_fqn: String,
    member_name: String,
}

impl<'ctx> RefactorDispatchReceiverLayout<'ctx> {
    pub(super) fn new(
        receiver_ty: TypeId,
        receiver_abi: RefactorAbiValue<'ctx>,
        owner_fqn: String,
        member_name: String,
    ) -> Self {
        Self {
            receiver_ty,
            receiver_abi,
            owner_fqn,
            member_name,
        }
    }

    pub(super) fn receiver_ty(&self) -> TypeId {
        self.receiver_ty
    }

    pub(super) fn receiver_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.receiver_abi
    }

    pub(super) fn owner_fqn(&self) -> &str {
        &self.owner_fqn
    }

    pub(super) fn member_name(&self) -> &str {
        &self.member_name
    }
}

/// runtime callable value 在 call boundary 上的 carrier 形状。
#[derive(Debug)]
pub(super) enum RefactorDynamicInvokeCarrierLayout<'ctx> {
    ClosureObject(RefactorClosureCarrierLayout<'ctx>),
    VirtualReceiver(RefactorDispatchReceiverLayout<'ctx>),
    InterfaceReceiver(RefactorDispatchReceiverLayout<'ctx>),
}

impl<'ctx> RefactorDynamicInvokeCarrierLayout<'ctx> {
    pub(super) fn receiver_abi(&self) -> &RefactorAbiValue<'ctx> {
        match self {
            Self::ClosureObject(layout) => layout.receiver_abi(),
            Self::VirtualReceiver(layout) | Self::InterfaceReceiver(layout) => {
                layout.receiver_abi()
            }
        }
    }
}

/// 按 call boundary 发布的 canonical dynamic-invoke surface：`invoke(receiver, args_tuple) -> Step_F`。
#[derive(Debug)]
pub(super) struct RefactorDynamicInvokeLayout<'ctx> {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    target_mode: CallTargetMode,
    invoke_args_tuple_ty: TypeId,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    args_abi: RefactorAbiValue<'ctx>,
    return_step_schema: StepSchemaId,
    carrier: RefactorDynamicInvokeCarrierLayout<'ctx>,
}

impl<'ctx> RefactorDynamicInvokeLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        target_mode: CallTargetMode,
        invoke_args_tuple_ty: TypeId,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        args_abi: RefactorAbiValue<'ctx>,
        return_step_schema: StepSchemaId,
        carrier: RefactorDynamicInvokeCarrierLayout<'ctx>,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            target_mode,
            invoke_args_tuple_ty,
            llvm_ty,
            param_count,
            args_abi,
            return_step_schema,
            carrier,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn target_mode(&self) -> CallTargetMode {
        self.target_mode
    }

    pub(super) fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
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

    pub(super) fn carrier(&self) -> &RefactorDynamicInvokeCarrierLayout<'ctx> {
        &self.carrier
    }
}

/// `CallSiteTarget` 经 ABI query 解析后的稳定 lowering 入口。
pub(super) enum RefactorCallTargetQuery<'a, 'ctx> {
    KnownInstance(&'a RefactorCallableLayout<'ctx>),
    DynamicInvoke(&'a RefactorDynamicInvokeLayout<'ctx>),
}

/// pure caller call boundary 本地消费 compiler-generated runtime-error case 的稳定 lowering 查询面。
pub(super) struct RefactorPublishedRuntimeEntryLayout<'ctx> {
    kind: LateLoweredPublishedRuntimeEntry,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
}

impl<'ctx> RefactorPublishedRuntimeEntryLayout<'ctx> {
    pub(super) fn new(
        kind: LateLoweredPublishedRuntimeEntry,
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
    ) -> Self {
        Self {
            kind,
            symbol_name,
            llvm_ty,
            param_count,
        }
    }

    pub(super) fn kind(&self) -> LateLoweredPublishedRuntimeEntry {
        self.kind
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
}

pub(super) enum RefactorLocalRuntimeErrorTerminalAction<'ctx> {
    RuntimeFatal {
        runtime_entry: RefactorPublishedRuntimeEntryLayout<'ctx>,
    },
}

impl<'ctx> RefactorLocalRuntimeErrorTerminalAction<'ctx> {
    pub(super) fn lowered_action(&self) -> LateLoweredLocalRuntimeErrorTerminalAction {
        match self {
            Self::RuntimeFatal { runtime_entry } => {
                LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
                    runtime_entry: runtime_entry.kind(),
                }
            }
        }
    }

    pub(super) fn runtime_entry(&self) -> &RefactorPublishedRuntimeEntryLayout<'ctx> {
        match self {
            Self::RuntimeFatal { runtime_entry } => runtime_entry,
        }
    }
}

pub(super) struct RefactorLocalRuntimeErrorContract<'ctx> {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    input_case_tag: CaseTag,
    payload_tuple_ty: TypeId,
    payload_abi: RefactorAbiValue<'ctx>,
    terminal_action: RefactorLocalRuntimeErrorTerminalAction<'ctx>,
    target_state: StateId,
}

impl<'ctx> RefactorLocalRuntimeErrorContract<'ctx> {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        input_case_tag: CaseTag,
        payload_tuple_ty: TypeId,
        payload_abi: RefactorAbiValue<'ctx>,
        terminal_action: RefactorLocalRuntimeErrorTerminalAction<'ctx>,
        target_state: StateId,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            input_case_tag,
            payload_tuple_ty,
            payload_abi,
            terminal_action,
            target_state,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn input_case_tag(&self) -> CaseTag {
        self.input_case_tag
    }

    pub(super) fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub(super) fn payload_abi(&self) -> &RefactorAbiValue<'ctx> {
        &self.payload_abi
    }

    pub(super) fn terminal_action(&self) -> &RefactorLocalRuntimeErrorTerminalAction<'ctx> {
        &self.terminal_action
    }

    pub(super) fn target_state(&self) -> StateId {
        self.target_state
    }
}

/// `HandleDispatch` 在 LLVM query 层发布的 field/tag 布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RefactorHandlePayloadBinderLayout {
    ordinal: u32,
    local: LocalId,
    frame_slot: Option<FrameSlotId>,
    frame_field_index: Option<u32>,
}

impl RefactorHandlePayloadBinderLayout {
    pub(super) fn new(
        ordinal: u32,
        local: LocalId,
        frame_slot: Option<FrameSlotId>,
        frame_field_index: Option<u32>,
    ) -> Self {
        Self {
            ordinal,
            local,
            frame_slot,
            frame_field_index,
        }
    }

    pub(super) fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(super) fn local(&self) -> LocalId {
        self.local
    }

    pub(super) fn frame_slot(&self) -> Option<FrameSlotId> {
        self.frame_slot
    }

    pub(super) fn frame_field_index(&self) -> Option<u32> {
        self.frame_field_index
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RefactorHandleContinuationBinderLayout {
    local: LocalId,
    frame_slot: Option<FrameSlotId>,
    frame_field_index: Option<u32>,
    continuation_schema: ContinuationSchemaId,
    continuation_object: ContinuationObjectId,
    surface_resume_binding: RefactorContinuationSurfaceResumeBinding,
}

impl RefactorHandleContinuationBinderLayout {
    pub(super) fn new(
        local: LocalId,
        frame_slot: Option<FrameSlotId>,
        frame_field_index: Option<u32>,
        continuation_schema: ContinuationSchemaId,
        continuation_object: ContinuationObjectId,
        surface_resume_binding: RefactorContinuationSurfaceResumeBinding,
    ) -> Self {
        Self {
            local,
            frame_slot,
            frame_field_index,
            continuation_schema,
            continuation_object,
            surface_resume_binding,
        }
    }

    pub(super) fn local(&self) -> LocalId {
        self.local
    }

    pub(super) fn frame_slot(&self) -> Option<FrameSlotId> {
        self.frame_slot
    }

    pub(super) fn frame_field_index(&self) -> Option<u32> {
        self.frame_field_index
    }

    pub(super) fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub(super) fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }

    pub(super) fn surface_resume_binding(&self) -> RefactorContinuationSurfaceResumeBinding {
        self.surface_resume_binding
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RefactorHandleArmLayout {
    handled_case: CaseTag,
    arm_state: StateId,
    arm_ordinal: u32,
    payload_tuple_ty: TypeId,
    payload_binders: Vec<RefactorHandlePayloadBinderLayout>,
    continuation_binder: Option<RefactorHandleContinuationBinderLayout>,
    arm_outward_cases: Vec<CaseTag>,
}

impl RefactorHandleArmLayout {
    pub(super) fn new(
        handled_case: CaseTag,
        arm_state: StateId,
        arm_ordinal: u32,
        payload_tuple_ty: TypeId,
        payload_binders: Vec<RefactorHandlePayloadBinderLayout>,
        continuation_binder: Option<RefactorHandleContinuationBinderLayout>,
        arm_outward_cases: Vec<CaseTag>,
    ) -> Self {
        Self {
            handled_case,
            arm_state,
            arm_ordinal,
            payload_tuple_ty,
            payload_binders,
            continuation_binder,
            arm_outward_cases,
        }
    }

    pub(super) fn handled_case(&self) -> CaseTag {
        self.handled_case
    }

    pub(super) fn arm_state(&self) -> StateId {
        self.arm_state
    }

    pub(super) fn arm_ordinal(&self) -> u32 {
        self.arm_ordinal
    }

    pub(super) fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub(super) fn payload_binders(&self) -> &[RefactorHandlePayloadBinderLayout] {
        &self.payload_binders
    }

    pub(super) fn continuation_binder(&self) -> Option<RefactorHandleContinuationBinderLayout> {
        self.continuation_binder
    }

    pub(super) fn arm_outward_cases(&self) -> &[CaseTag] {
        &self.arm_outward_cases
    }
}

pub(super) struct RefactorHandleDispatchLayout {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    lowered_contract: LateLoweredHandleDispatchContract,
    state_tag_field_index: u32,
    completion_tag_field_index: u32,
    payload_carrier_field_index: u32,
    completion_tags: BTreeMap<LateLoweredHandlePendingCompletion, u32>,
    handled_arms: Vec<RefactorHandleArmLayout>,
}

impl RefactorHandleDispatchLayout {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        lowered_contract: LateLoweredHandleDispatchContract,
        state_tag_field_index: u32,
        completion_tag_field_index: u32,
        payload_carrier_field_index: u32,
        completion_tags: BTreeMap<LateLoweredHandlePendingCompletion, u32>,
        handled_arms: Vec<RefactorHandleArmLayout>,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            lowered_contract,
            state_tag_field_index,
            completion_tag_field_index,
            payload_carrier_field_index,
            completion_tags,
            handled_arms,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn lowered_contract(&self) -> &LateLoweredHandleDispatchContract {
        &self.lowered_contract
    }

    pub(super) fn state_tag_field_index(&self) -> u32 {
        self.state_tag_field_index
    }

    pub(super) fn completion_tag_field_index(&self) -> u32 {
        self.completion_tag_field_index
    }

    pub(super) fn payload_carrier_field_index(&self) -> u32 {
        self.payload_carrier_field_index
    }

    pub(super) fn completion_tag_value(
        &self,
        completion: LateLoweredHandlePendingCompletion,
    ) -> Option<u32> {
        self.completion_tags.get(&completion).copied()
    }

    pub(super) fn completion_tags(&self) -> &BTreeMap<LateLoweredHandlePendingCompletion, u32> {
        &self.completion_tags
    }

    pub(super) fn handled_arms(&self) -> &[RefactorHandleArmLayout] {
        &self.handled_arms
    }

    pub(super) fn handled_arm(&self, handled_case: CaseTag) -> Option<&RefactorHandleArmLayout> {
        self.handled_arms
            .iter()
            .find(|arm| arm.handled_case() == handled_case)
    }

    pub(super) fn state_regions(&self) -> &[LateLoweredHandleStateRegionEntry] {
        self.lowered_contract.state_regions()
    }

    pub(super) fn state_region(&self, state_id: StateId) -> LateLoweredHandleStateRegion {
        self.lowered_contract.state_region(state_id)
    }

    pub(super) fn boundary_routings(&self) -> &[LateLoweredHandleBoundaryRouting] {
        self.lowered_contract.boundary_routings()
    }

    pub(super) fn boundary_routing(
        &self,
        boundary_id: crate::effect_lowered::ir::BoundaryId,
    ) -> Option<&LateLoweredHandleBoundaryRouting> {
        self.lowered_contract.boundary_routing(boundary_id)
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
    source_value_layouts: BTreeMap<TypeId, RefactorSourceAbiLayout<'ctx>>,
    step_layouts: BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    frame_layouts: BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
    continuation_layouts: BTreeMap<ContinuationObjectId, RefactorContinuationObjectLayout<'ctx>>,
    resume_interface_layouts: BTreeMap<ResumeInterfaceId, RefactorResumeInterfaceLayout<'ctx>>,
    surface_resume_layouts:
        BTreeMap<ContinuationSchemaId, RefactorContinuationSurfaceResumeLayout<'ctx>>,
    callable_layouts: BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
    dynamic_invoke_layouts: BTreeMap<(StepSchemaId, SiteId), RefactorDynamicInvokeLayout<'ctx>>,
    local_runtime_error_contracts:
        BTreeMap<(StepSchemaId, SiteId), RefactorLocalRuntimeErrorContract<'ctx>>,
    handle_dispatch_layouts: BTreeMap<(StepSchemaId, SiteId), RefactorHandleDispatchLayout>,
}

impl<'ctx> RefactorAbiQuery<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        source_value_layouts: BTreeMap<TypeId, RefactorSourceAbiLayout<'ctx>>,
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
        dynamic_invoke_layouts: BTreeMap<(StepSchemaId, SiteId), RefactorDynamicInvokeLayout<'ctx>>,
        local_runtime_error_contracts: BTreeMap<
            (StepSchemaId, SiteId),
            RefactorLocalRuntimeErrorContract<'ctx>,
        >,
        handle_dispatch_layouts: BTreeMap<(StepSchemaId, SiteId), RefactorHandleDispatchLayout>,
    ) -> Self {
        Self {
            source_value_layouts,
            step_layouts,
            frame_layouts,
            continuation_layouts,
            resume_interface_layouts,
            surface_resume_layouts,
            callable_layouts,
            dynamic_invoke_layouts,
            local_runtime_error_contracts,
            handle_dispatch_layouts,
        }
    }

    pub(super) fn source_value_layout(
        &self,
        source_ty: TypeId,
    ) -> Result<&RefactorSourceAbiLayout<'ctx>, LlvmEmitError> {
        self.source_value_layouts
            .get(&source_ty)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "refactor LLVM ABI query 缺少 source type {} 的 ABI value lowering contract",
                    source_ty.as_u32()
                ),
            })
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

    pub(super) fn dynamic_invoke_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
    ) -> Option<&RefactorDynamicInvokeLayout<'ctx>> {
        self.dynamic_invoke_layouts
            .get(&(owner_step_schema, site_id))
    }

    pub(super) fn call_local_runtime_error_contract(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LateLoweredConsumedRuntimeErrorCase,
    ) -> Result<&RefactorLocalRuntimeErrorContract<'ctx>, LlvmEmitError> {
        let published = self
            .local_runtime_error_contracts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "refactor LLVM ABI query 缺少 owner step schema s{} call site {} 的 local runtime-error contract",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                ),
            })?;
        if published.input_case_tag() != contract.input_case_tag()
            || published.payload_tuple_ty() != contract.payload_tuple_ty()
            || published.terminal_action().lowered_action() != contract.terminal_action()
            || published.target_state() != contract.target_state()
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "refactor LLVM ABI query 发现 owner step schema s{} call site {} 的 local runtime-error contract 漂移：layout=(input_case=c{}, payload_tuple_ty={}, terminal_action={:?}, target_state=st{})，lowering=(input_case=c{}, payload_tuple_ty={}, terminal_action={:?}, target_state=st{})",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                    published.input_case_tag().as_u32(),
                    published.payload_tuple_ty().as_u32(),
                    published.terminal_action().lowered_action(),
                    published.target_state().as_u32(),
                    contract.input_case_tag().as_u32(),
                    contract.payload_tuple_ty().as_u32(),
                    contract.terminal_action(),
                    contract.target_state().as_u32(),
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn handle_dispatch_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LateLoweredHandleDispatchContract,
    ) -> Result<&RefactorHandleDispatchLayout, LlvmEmitError> {
        let published = self
            .handle_dispatch_layouts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "refactor LLVM ABI query 缺少 owner step schema s{} handle site {} 的 HandleDispatch contract",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                ),
            })?;
        if published.lowered_contract() != contract {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "refactor LLVM ABI query 发现 owner step schema s{} handle site {} 的 HandleDispatch contract 漂移：published={:?}，lowered={:?}",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                    published.lowered_contract(),
                    contract,
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn call_target_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        facts: &CallSiteEffectFacts,
    ) -> Result<RefactorCallTargetQuery<'_, 'ctx>, LlvmEmitError> {
        match facts.target() {
            crate::effect_facts::CallSiteTarget::KnownInstance(instance) => {
                let layout = self
                    .callable_layout_by_root_fqn(&instance.template.fqn)
                    .ok_or_else(|| LlvmEmitError::Frontend {
                        message: format!(
                            "refactor LLVM ABI query 缺少 known-instance call target `{}` 的 callable layout",
                            instance.template.fqn
                        ),
                    })?;
                if layout.dynamic_entry().invoke_args_tuple_ty() != facts.invoke_args_tuple_ty()
                    || layout.dynamic_entry().return_step_schema() != facts.callee_schema()
                {
                    return Err(LlvmEmitError::Frontend {
                        message: format!(
                            "refactor LLVM ABI query 发现 known-instance call target `{}` 的 dynamic entry contract 漂移：layout=(invoke_args_tuple_ty={}, return_step_schema={})，facts=(invoke_args_tuple_ty={}, callee_step_schema={})",
                            instance.template.fqn,
                            layout.dynamic_entry().invoke_args_tuple_ty().as_u32(),
                            layout.dynamic_entry().return_step_schema().as_u32(),
                            facts.invoke_args_tuple_ty().as_u32(),
                            facts.callee_schema().as_u32(),
                        ),
                    });
                }
                Ok(RefactorCallTargetQuery::KnownInstance(layout))
            }
            crate::effect_facts::CallSiteTarget::CandidateSet(_)
            | crate::effect_facts::CallSiteTarget::DynamicFallback => {
                let layout = self.dynamic_invoke_layout(owner_step_schema, site_id).ok_or_else(
                    || LlvmEmitError::Frontend {
                        message: format!(
                            "refactor LLVM ABI query 缺少 owner step schema s{} call site {} 的 dynamic-invoke contract",
                            owner_step_schema.as_u32(),
                            site_id.as_u32(),
                        ),
                    },
                )?;
                if layout.target_mode() != facts.target_mode()
                    || layout.invoke_args_tuple_ty() != facts.invoke_args_tuple_ty()
                    || layout.return_step_schema() != facts.callee_schema()
                {
                    return Err(LlvmEmitError::Frontend {
                        message: format!(
                            "refactor LLVM ABI query 发现 owner step schema s{} call site {} 的 dynamic-invoke contract 漂移：layout=(target_mode={:?}, invoke_args_tuple_ty={}, return_step_schema={})，facts=(target_mode={:?}, invoke_args_tuple_ty={}, callee_step_schema={})",
                            owner_step_schema.as_u32(),
                            site_id.as_u32(),
                            layout.target_mode(),
                            layout.invoke_args_tuple_ty().as_u32(),
                            layout.return_step_schema().as_u32(),
                            facts.target_mode(),
                            facts.invoke_args_tuple_ty().as_u32(),
                            facts.callee_schema().as_u32(),
                        ),
                    });
                }
                Ok(RefactorCallTargetQuery::DynamicInvoke(layout))
            }
        }
    }
}
