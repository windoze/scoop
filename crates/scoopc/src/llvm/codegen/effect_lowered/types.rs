#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};

use inkwell::types::{BasicTypeEnum, FunctionType, StructType};

use crate::effect_facts::{
    CallSiteEffectFacts, CallTargetMode, CaseTag, ConcreteOpKey, ContinuationSchemaId, StepSchemaId,
};
use crate::effect_lowered::ir::{
    BoundaryId, ContinuationObjectId, FrameSlotId, LateLoweredBodyVersionKey,
    LateLoweredCallBoundaryOperandContract, LateLoweredCompletionPayloadBinding,
    LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationMethodReachability, LateLoweredHandleBoundaryRouting,
    LateLoweredHandleDispatchContract, LateLoweredHandlePendingCompletion,
    LateLoweredHandlePendingCompletionOrigin, LateLoweredHandlePendingPayloadTransport,
    LateLoweredHandleStateRegion, LateLoweredHandleStateRegionEntry,
    LateLoweredLocalRuntimeErrorTerminalAction, LateLoweredPerformBoundaryOperandContract,
    LateLoweredPublishedRuntimeEntry, LateLoweredResumeBoundaryOperandContract,
    LateLoweredResumePayloadBinding, LateLoweredSurfaceResumeDispatchSourceKind,
    LateLoweredSurfaceResumeWrapperProjection, ResumeInterfaceId, StateId, SystemSlotKind,
};
use crate::llvm::LlvmEmitError;
use crate::mir::{InstanceKey, LocalId, SiteId};
use crate::ty::TypeId;

use super::super::CallableCarrierKind;

/// 单个 ABI 值位的 LLVM 形状。
///
/// `elided=true` 表示该值在 function ABI 中可被省略；但若它出现在 frame/step payload field 中，
/// 仍可能用零大小 struct 保留稳定 field index。
#[derive(Clone, Copy, Debug)]
pub(super) struct AbiValue<'ctx> {
    llvm_ty: BasicTypeEnum<'ctx>,
    elided: bool,
}

impl<'ctx> AbiValue<'ctx> {
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

/// source type 在 LLVM ABI 中的稳定 carrier 分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceAbiLayoutKind {
    Scalar,
    Tuple,
}

/// tuple-like source carrier 中单个 source field 对应的 ABI field 映射。
#[derive(Debug, Clone, Copy)]
pub(super) struct SourceAbiFieldLayout<'ctx> {
    source_index: u32,
    source_ty: TypeId,
    abi_field_index: Option<u32>,
    abi: AbiValue<'ctx>,
}

impl<'ctx> SourceAbiFieldLayout<'ctx> {
    pub(super) fn new(
        source_index: u32,
        source_ty: TypeId,
        abi_field_index: Option<u32>,
        abi: AbiValue<'ctx>,
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

    pub(super) fn abi(&self) -> &AbiValue<'ctx> {
        &self.abi
    }

    pub(super) fn is_elided(&self) -> bool {
        self.abi.is_elided()
    }
}

/// late-lowered source type 到 LLVM ABI value 的 authoritative 查询面。
#[derive(Debug, Clone)]
pub(super) struct SourceAbiLayout<'ctx> {
    source_ty: TypeId,
    kind: SourceAbiLayoutKind,
    abi: AbiValue<'ctx>,
    fields: Vec<SourceAbiFieldLayout<'ctx>>,
}

impl<'ctx> SourceAbiLayout<'ctx> {
    pub(super) fn new(
        source_ty: TypeId,
        kind: SourceAbiLayoutKind,
        abi: AbiValue<'ctx>,
        fields: Vec<SourceAbiFieldLayout<'ctx>>,
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

    pub(super) fn kind(&self) -> SourceAbiLayoutKind {
        self.kind
    }

    pub(super) fn is_tuple(&self) -> bool {
        self.kind == SourceAbiLayoutKind::Tuple
    }

    pub(super) fn abi(&self) -> &AbiValue<'ctx> {
        &self.abi
    }

    pub(super) fn fields(&self) -> &[SourceAbiFieldLayout<'ctx>] {
        &self.fields
    }

    pub(super) fn field(&self, source_index: usize) -> Option<&SourceAbiFieldLayout<'ctx>> {
        self.fields.get(source_index)
    }

    pub(super) fn abi_field_count(&self) -> usize {
        self.fields
            .iter()
            .filter(|field| field.abi_field_index().is_some())
            .count()
    }
}

/// 单个 concrete class instance field 在 LLVM handoff 中的稳定来源。
#[derive(Debug, Clone)]
pub(super) struct ClassInstanceFieldLayout {
    field_fqn: String,
    source_ty: TypeId,
}

impl ClassInstanceFieldLayout {
    pub(super) fn new(field_fqn: String, source_ty: TypeId) -> Self {
        Self {
            field_fqn,
            source_ty,
        }
    }

    pub(super) fn field_fqn(&self) -> &str {
        &self.field_fqn
    }

    pub(super) fn source_ty(&self) -> TypeId {
        self.source_ty
    }
}

/// concrete class source type 到 canonical class payload/type-descriptor key 的 handoff。
#[derive(Debug, Clone)]
pub(super) struct ClassInstanceLayout {
    source_ty: TypeId,
    base_fqn: String,
    class_key: String,
    fields: Vec<ClassInstanceFieldLayout>,
}

impl ClassInstanceLayout {
    pub(super) fn new(
        source_ty: TypeId,
        base_fqn: String,
        class_key: String,
        fields: Vec<ClassInstanceFieldLayout>,
    ) -> Self {
        Self {
            source_ty,
            base_fqn,
            class_key,
            fields,
        }
    }

    pub(super) fn source_ty(&self) -> TypeId {
        self.source_ty
    }

    pub(super) fn base_fqn(&self) -> &str {
        &self.base_fqn
    }

    pub(super) fn class_key(&self) -> &str {
        &self.class_key
    }

    pub(super) fn fields(&self) -> &[ClassInstanceFieldLayout] {
        &self.fields
    }
}

/// `Step_F` 的单个 variant 布局。
pub(super) struct StepVariantLayout<'ctx> {
    tag_value: u32,
    payload_source_ty: TypeId,
    payload_ty: StructType<'ctx>,
    payload_field_count: usize,
    payload_anchor_name: String,
    payload_is_elided: bool,
}

impl<'ctx> StepVariantLayout<'ctx> {
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
pub(super) struct StepCaseLayout<'ctx> {
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    tag_constant_name: String,
    variant: StepVariantLayout<'ctx>,
}

impl<'ctx> StepCaseLayout<'ctx> {
    pub(super) fn new(
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        payload_tuple_ty: TypeId,
        tag_constant_name: String,
        variant: StepVariantLayout<'ctx>,
    ) -> Self {
        Self {
            case_tag,
            concrete_op_key,
            payload_tuple_ty,
            tag_constant_name,
            variant,
        }
    }

    pub(super) fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub(super) fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub(super) fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub(super) fn tag_constant_name(&self) -> &str {
        &self.tag_constant_name
    }

    pub(super) fn variant(&self) -> &StepVariantLayout<'ctx> {
        &self.variant
    }
}

/// 单个 `StepSchemaId` 对应的 canonical `Step_F` 布局。
pub(super) struct StepLayout<'ctx> {
    step_schema: StepSchemaId,
    stable_effect_key_text: String,
    llvm_ty: StructType<'ctx>,
    layout_anchor_name: String,
    complete_tag_constant_name: String,
    complete_variant: StepVariantLayout<'ctx>,
    cases: BTreeMap<CaseTag, StepCaseLayout<'ctx>>,
}

impl<'ctx> StepLayout<'ctx> {
    pub(super) fn new(
        step_schema: StepSchemaId,
        stable_effect_key_text: String,
        llvm_ty: StructType<'ctx>,
        layout_anchor_name: String,
        complete_tag_constant_name: String,
        complete_variant: StepVariantLayout<'ctx>,
        cases: BTreeMap<CaseTag, StepCaseLayout<'ctx>>,
    ) -> Self {
        Self {
            step_schema,
            stable_effect_key_text,
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

    pub(super) fn stable_effect_key_text(&self) -> &str {
        &self.stable_effect_key_text
    }

    pub(super) fn layout_anchor_name(&self) -> &str {
        &self.layout_anchor_name
    }

    pub(super) fn complete_tag_constant_name(&self) -> &str {
        &self.complete_tag_constant_name
    }

    pub(super) fn complete_variant(&self) -> &StepVariantLayout<'ctx> {
        &self.complete_variant
    }

    pub(super) fn cases(&self) -> &BTreeMap<CaseTag, StepCaseLayout<'ctx>> {
        &self.cases
    }

    pub(super) fn case_layout(&self, case_tag: CaseTag) -> Option<&StepCaseLayout<'ctx>> {
        self.cases.get(&case_tag)
    }
}

/// frame 内单个 field 的稳定分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameFieldKind {
    Header,
    Slot(FrameSlotId),
}

/// 单个 frame field 的 LLVM 布局。
pub(super) struct FrameFieldLayout<'ctx> {
    field_index: u32,
    kind: FrameFieldKind,
    llvm_ty: BasicTypeEnum<'ctx>,
}

impl<'ctx> FrameFieldLayout<'ctx> {
    pub(super) fn new(
        field_index: u32,
        kind: FrameFieldKind,
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

    pub(super) fn kind(&self) -> FrameFieldKind {
        self.kind
    }

    pub(super) fn llvm_ty(&self) -> BasicTypeEnum<'ctx> {
        self.llvm_ty
    }
}

/// 单个 callable version 的 frame 布局查询面。
pub(super) struct FrameLayout<'ctx> {
    step_schema: StepSchemaId,
    llvm_ty: StructType<'ctx>,
    layout_anchor_name: String,
    fields: Vec<FrameFieldLayout<'ctx>>,
    slot_field_indices: BTreeMap<FrameSlotId, u32>,
    system_field_indices: BTreeMap<SystemSlotKind, u32>,
}

impl<'ctx> FrameLayout<'ctx> {
    pub(super) fn new(
        step_schema: StepSchemaId,
        llvm_ty: StructType<'ctx>,
        layout_anchor_name: String,
        fields: Vec<FrameFieldLayout<'ctx>>,
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

    pub(super) fn fields(&self) -> &[FrameFieldLayout<'ctx>] {
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
pub(super) struct CallableEntryLayout<'ctx> {
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    invoke_args_tuple_ty: TypeId,
    args_abi: AbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

/// plain callable 入口使用普通函数 ABI；它不携带 `StepSchema` 或 continuation/state-machine shell。
pub(super) struct PlainCallableEntryLayout<'ctx> {
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    function_ty: TypeId,
    param_tys: Vec<TypeId>,
    return_ty: TypeId,
}

impl<'ctx> PlainCallableEntryLayout<'ctx> {
    pub(super) fn new(
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        function_ty: TypeId,
        param_tys: Vec<TypeId>,
        return_ty: TypeId,
    ) -> Self {
        Self {
            symbol_name,
            llvm_ty,
            param_count,
            function_ty,
            param_tys,
            return_ty,
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

    pub(super) fn function_ty(&self) -> TypeId {
        self.function_ty
    }

    pub(super) fn param_tys(&self) -> &[TypeId] {
        &self.param_tys
    }

    pub(super) fn return_ty(&self) -> TypeId {
        self.return_ty
    }
}

impl<'ctx> CallableEntryLayout<'ctx> {
    pub(super) fn new(
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        invoke_args_tuple_ty: TypeId,
        args_abi: AbiValue<'ctx>,
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

    pub(super) fn args_abi(&self) -> &AbiValue<'ctx> {
        &self.args_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// closure-like runtime callable object 的 carrier 布局。
#[derive(Debug)]
pub(super) struct ClosureCarrierLayout<'ctx> {
    object_ty: StructType<'ctx>,
    receiver_abi: AbiValue<'ctx>,
    env_field_index: u32,
    fn_field_index: u32,
}

impl<'ctx> ClosureCarrierLayout<'ctx> {
    pub(super) fn new(
        object_ty: StructType<'ctx>,
        receiver_abi: AbiValue<'ctx>,
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

    pub(super) fn receiver_abi(&self) -> &AbiValue<'ctx> {
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
pub(super) struct DispatchReceiverLayout<'ctx> {
    receiver_ty: TypeId,
    receiver_abi: AbiValue<'ctx>,
    owner_fqn: String,
    member_name: String,
    method_slot: u32,
    interface_id: Option<u64>,
}

impl<'ctx> DispatchReceiverLayout<'ctx> {
    pub(super) fn new(
        receiver_ty: TypeId,
        receiver_abi: AbiValue<'ctx>,
        owner_fqn: String,
        member_name: String,
        method_slot: u32,
        interface_id: Option<u64>,
    ) -> Self {
        Self {
            receiver_ty,
            receiver_abi,
            owner_fqn,
            member_name,
            method_slot,
            interface_id,
        }
    }

    pub(super) fn receiver_ty(&self) -> TypeId {
        self.receiver_ty
    }

    pub(super) fn receiver_abi(&self) -> &AbiValue<'ctx> {
        &self.receiver_abi
    }

    pub(super) fn owner_fqn(&self) -> &str {
        &self.owner_fqn
    }

    pub(super) fn member_name(&self) -> &str {
        &self.member_name
    }

    pub(super) fn method_slot(&self) -> u32 {
        self.method_slot
    }

    pub(super) fn interface_id(&self) -> Option<u64> {
        self.interface_id
    }
}

/// runtime callable value 在 call boundary 上的 carrier 形状。
#[derive(Debug)]
pub(super) enum DynamicInvokeCarrierLayout<'ctx> {
    ClosureObject(ClosureCarrierLayout<'ctx>),
    FunPtr(AbiValue<'ctx>),
    VirtualReceiver(DispatchReceiverLayout<'ctx>),
    InterfaceReceiver(DispatchReceiverLayout<'ctx>),
}

impl<'ctx> DynamicInvokeCarrierLayout<'ctx> {
    pub(super) fn receiver_abi(&self) -> &AbiValue<'ctx> {
        match self {
            Self::ClosureObject(layout) => layout.receiver_abi(),
            Self::FunPtr(abi) => abi,
            Self::VirtualReceiver(layout) | Self::InterfaceReceiver(layout) => {
                layout.receiver_abi()
            }
        }
    }
}

/// 按 call boundary 发布的 canonical dynamic-invoke surface：`invoke(receiver, args_tuple) -> Step_F`。
#[derive(Debug)]
pub(super) struct DynamicInvokeLayout<'ctx> {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    target_mode: CallTargetMode,
    invoke_args_tuple_ty: TypeId,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    args_abi: AbiValue<'ctx>,
    return_step_schema: StepSchemaId,
    carrier: DynamicInvokeCarrierLayout<'ctx>,
    candidate_targets: Vec<String>,
}

impl<'ctx> DynamicInvokeLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        target_mode: CallTargetMode,
        invoke_args_tuple_ty: TypeId,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        args_abi: AbiValue<'ctx>,
        return_step_schema: StepSchemaId,
        carrier: DynamicInvokeCarrierLayout<'ctx>,
        candidate_targets: Vec<String>,
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
            candidate_targets,
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

    pub(super) fn args_abi(&self) -> &AbiValue<'ctx> {
        &self.args_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }

    pub(super) fn carrier(&self) -> &DynamicInvokeCarrierLayout<'ctx> {
        &self.carrier
    }

    pub(super) fn candidate_targets(&self) -> &[String] {
        &self.candidate_targets
    }
}

/// `CallSiteTarget` 经 ABI query 解析后的稳定 lowering 入口。
pub(super) enum CallTargetQuery<'a, 'ctx> {
    KnownInstance(&'a CallableLayout<'ctx>),
    DynamicInvoke(&'a DynamicInvokeLayout<'ctx>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallBoundaryOperandLayout {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    contract: LateLoweredCallBoundaryOperandContract,
}

impl CallBoundaryOperandLayout {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: LateLoweredCallBoundaryOperandContract,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            contract,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn contract(&self) -> &LateLoweredCallBoundaryOperandContract {
        &self.contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PerformBoundaryOperandLayout {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    contract: LateLoweredPerformBoundaryOperandContract,
}

impl PerformBoundaryOperandLayout {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: LateLoweredPerformBoundaryOperandContract,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            contract,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn contract(&self) -> &LateLoweredPerformBoundaryOperandContract {
        &self.contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResumeBoundaryOperandLayout {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    contract: LateLoweredResumeBoundaryOperandContract,
}

impl ResumeBoundaryOperandLayout {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: LateLoweredResumeBoundaryOperandContract,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            contract,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn contract(&self) -> &LateLoweredResumeBoundaryOperandContract {
        &self.contract
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResumePayloadBindingLayout {
    owner_step_schema: StepSchemaId,
    binding: LateLoweredResumePayloadBinding,
    frame_field_index: Option<u32>,
}

impl ResumePayloadBindingLayout {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        binding: LateLoweredResumePayloadBinding,
        frame_field_index: Option<u32>,
    ) -> Self {
        Self {
            owner_step_schema,
            binding,
            frame_field_index,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn binding(&self) -> &LateLoweredResumePayloadBinding {
        &self.binding
    }

    pub(super) fn boundary_id(&self) -> BoundaryId {
        self.binding.boundary_id()
    }

    pub(super) fn resume_state(&self) -> StateId {
        self.binding.resume_state()
    }

    pub(super) fn consumer_local(&self) -> LocalId {
        self.binding.consumer_local()
    }

    pub(super) fn consumer_frame_slot(&self) -> Option<FrameSlotId> {
        self.binding.consumer_frame_slot()
    }

    pub(super) fn frame_field_index(&self) -> Option<u32> {
        self.frame_field_index
    }
}

#[derive(Debug, Clone)]
pub(super) struct CompletionPayloadBindingLayout<'ctx> {
    owner_step_schema: StepSchemaId,
    binding: LateLoweredCompletionPayloadBinding,
    payload_abi: AbiValue<'ctx>,
    frame_field_index: Option<u32>,
}

impl<'ctx> CompletionPayloadBindingLayout<'ctx> {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        binding: LateLoweredCompletionPayloadBinding,
        payload_abi: AbiValue<'ctx>,
        frame_field_index: Option<u32>,
    ) -> Self {
        Self {
            owner_step_schema,
            binding,
            payload_abi,
            frame_field_index,
        }
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn binding(&self) -> &LateLoweredCompletionPayloadBinding {
        &self.binding
    }

    pub(super) fn return_state(&self) -> StateId {
        self.binding.return_state()
    }

    pub(super) fn complete_state(&self) -> StateId {
        self.binding.complete_state()
    }

    pub(super) fn payload_source(&self) -> &LateLoweredCompletionPayloadSource {
        self.binding.payload_source()
    }

    pub(super) fn payload_frame_slot(&self) -> Option<FrameSlotId> {
        self.binding.payload_frame_slot()
    }

    pub(super) fn payload_abi(&self) -> &AbiValue<'ctx> {
        &self.payload_abi
    }

    pub(super) fn frame_field_index(&self) -> Option<u32> {
        self.frame_field_index
    }
}

/// pure caller call boundary 本地消费 compiler-generated runtime-error case 的稳定 lowering 查询面。
pub(super) struct PublishedRuntimeEntryLayout<'ctx> {
    kind: LateLoweredPublishedRuntimeEntry,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
}

impl<'ctx> PublishedRuntimeEntryLayout<'ctx> {
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

pub(super) enum LocalRuntimeErrorTerminalAction<'ctx> {
    RuntimeFatal {
        runtime_entry: PublishedRuntimeEntryLayout<'ctx>,
    },
}

impl<'ctx> LocalRuntimeErrorTerminalAction<'ctx> {
    pub(super) fn lowered_action(&self) -> LateLoweredLocalRuntimeErrorTerminalAction {
        match self {
            Self::RuntimeFatal { runtime_entry } => {
                LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
                    runtime_entry: runtime_entry.kind(),
                }
            }
        }
    }

    pub(super) fn runtime_entry(&self) -> &PublishedRuntimeEntryLayout<'ctx> {
        match self {
            Self::RuntimeFatal { runtime_entry } => runtime_entry,
        }
    }
}

pub(super) struct LocalRuntimeErrorContract<'ctx> {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    input_case_tag: CaseTag,
    payload_tuple_ty: TypeId,
    payload_abi: AbiValue<'ctx>,
    terminal_action: LocalRuntimeErrorTerminalAction<'ctx>,
    target_state: StateId,
}

impl<'ctx> LocalRuntimeErrorContract<'ctx> {
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        input_case_tag: CaseTag,
        payload_tuple_ty: TypeId,
        payload_abi: AbiValue<'ctx>,
        terminal_action: LocalRuntimeErrorTerminalAction<'ctx>,
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

    pub(super) fn payload_abi(&self) -> &AbiValue<'ctx> {
        &self.payload_abi
    }

    pub(super) fn terminal_action(&self) -> &LocalRuntimeErrorTerminalAction<'ctx> {
        &self.terminal_action
    }

    pub(super) fn target_state(&self) -> StateId {
        self.target_state
    }
}

/// `HandleDispatch` 在 LLVM query 层发布的 field/tag 布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HandlePayloadBinderLayout {
    ordinal: u32,
    local: LocalId,
    frame_slot: Option<FrameSlotId>,
    frame_field_index: Option<u32>,
}

impl HandlePayloadBinderLayout {
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
pub(super) struct HandleContinuationBinderLayout {
    local: LocalId,
    frame_slot: Option<FrameSlotId>,
    frame_field_index: Option<u32>,
    continuation_schema: ContinuationSchemaId,
    continuation_object: ContinuationObjectId,
    surface_resume_source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
    surface_resume_return_step_schema: StepSchemaId,
}

impl HandleContinuationBinderLayout {
    pub(super) fn new(
        local: LocalId,
        frame_slot: Option<FrameSlotId>,
        frame_field_index: Option<u32>,
        continuation_schema: ContinuationSchemaId,
        continuation_object: ContinuationObjectId,
        surface_resume_source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
        surface_resume_return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            local,
            frame_slot,
            frame_field_index,
            continuation_schema,
            continuation_object,
            surface_resume_source_kind,
            surface_resume_return_step_schema,
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

    pub(super) fn surface_resume_source_kind(&self) -> LateLoweredSurfaceResumeDispatchSourceKind {
        self.surface_resume_source_kind
    }

    pub(super) fn surface_resume_return_step_schema(&self) -> StepSchemaId {
        self.surface_resume_return_step_schema
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HandleArmLayout {
    handled_case: CaseTag,
    arm_state: StateId,
    arm_ordinal: u32,
    payload_tuple_ty: TypeId,
    payload_binders: Vec<HandlePayloadBinderLayout>,
    continuation_binder: Option<HandleContinuationBinderLayout>,
    arm_outward_cases: Vec<CaseTag>,
}

impl HandleArmLayout {
    pub(super) fn new(
        handled_case: CaseTag,
        arm_state: StateId,
        arm_ordinal: u32,
        payload_tuple_ty: TypeId,
        payload_binders: Vec<HandlePayloadBinderLayout>,
        continuation_binder: Option<HandleContinuationBinderLayout>,
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

    pub(super) fn payload_binders(&self) -> &[HandlePayloadBinderLayout] {
        &self.payload_binders
    }

    pub(super) fn continuation_binder(&self) -> Option<HandleContinuationBinderLayout> {
        self.continuation_binder
    }

    pub(super) fn arm_outward_cases(&self) -> &[CaseTag] {
        &self.arm_outward_cases
    }
}

pub(super) struct HandleDispatchLayout {
    owner_step_schema: StepSchemaId,
    site_id: SiteId,
    lowered_contract: LateLoweredHandleDispatchContract,
    state_tag_field_index: u32,
    completion_tag_field_index: u32,
    payload_carrier_field_index: u32,
    completion_tags: BTreeMap<LateLoweredHandlePendingCompletion, u32>,
    pending_completion_origin_tags: BTreeMap<LateLoweredHandlePendingCompletionOrigin, u32>,
    pending_payload_transports:
        BTreeMap<LateLoweredHandlePendingCompletion, HandlePendingPayloadTransportLayout>,
    handled_arms: Vec<HandleArmLayout>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HandlePendingPayloadTransportLayout {
    lowered_transport: LateLoweredHandlePendingPayloadTransport,
    frame_field_index: u32,
}

impl HandlePendingPayloadTransportLayout {
    pub(super) fn new(
        lowered_transport: LateLoweredHandlePendingPayloadTransport,
        frame_field_index: u32,
    ) -> Self {
        Self {
            lowered_transport,
            frame_field_index,
        }
    }

    pub(super) fn lowered_transport(&self) -> LateLoweredHandlePendingPayloadTransport {
        self.lowered_transport
    }

    pub(super) fn completion(&self) -> LateLoweredHandlePendingCompletion {
        self.lowered_transport.completion()
    }

    pub(super) fn payload_tuple_ty(&self) -> TypeId {
        self.lowered_transport.payload_tuple_ty()
    }

    pub(super) fn frame_slot(&self) -> FrameSlotId {
        self.lowered_transport.frame_slot()
    }

    pub(super) fn frame_field_index(&self) -> u32 {
        self.frame_field_index
    }
}

impl HandleDispatchLayout {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        lowered_contract: LateLoweredHandleDispatchContract,
        state_tag_field_index: u32,
        completion_tag_field_index: u32,
        payload_carrier_field_index: u32,
        completion_tags: BTreeMap<LateLoweredHandlePendingCompletion, u32>,
        pending_completion_origin_tags: BTreeMap<LateLoweredHandlePendingCompletionOrigin, u32>,
        pending_payload_transports: BTreeMap<
            LateLoweredHandlePendingCompletion,
            HandlePendingPayloadTransportLayout,
        >,
        handled_arms: Vec<HandleArmLayout>,
    ) -> Self {
        Self {
            owner_step_schema,
            site_id,
            lowered_contract,
            state_tag_field_index,
            completion_tag_field_index,
            payload_carrier_field_index,
            completion_tags,
            pending_completion_origin_tags,
            pending_payload_transports,
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

    pub(super) fn pending_completion_origin_tag_value(
        &self,
        origin: LateLoweredHandlePendingCompletionOrigin,
    ) -> Option<u32> {
        self.pending_completion_origin_tags.get(&origin).copied()
    }

    pub(super) fn pending_completion_origin_tags(
        &self,
    ) -> &BTreeMap<LateLoweredHandlePendingCompletionOrigin, u32> {
        &self.pending_completion_origin_tags
    }

    pub(super) fn pending_payload_transport_layout(
        &self,
        completion: LateLoweredHandlePendingCompletion,
    ) -> Option<&HandlePendingPayloadTransportLayout> {
        self.pending_payload_transports.get(&completion)
    }

    pub(super) fn handled_arms(&self) -> &[HandleArmLayout] {
        &self.handled_arms
    }

    pub(super) fn handled_arm(&self, handled_case: CaseTag) -> Option<&HandleArmLayout> {
        self.handled_arms
            .iter()
            .find(|arm| arm.handled_case() == handled_case)
    }

    pub(super) fn handled_arm_by_ordinal(
        &self,
        arm_ordinal: u32,
    ) -> Option<&HandleArmLayout> {
        self.handled_arms
            .iter()
            .find(|arm| arm.arm_ordinal() == arm_ordinal)
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
pub(super) struct ContinuationSurfaceResumeLayout<'ctx> {
    continuation_schema: ContinuationSchemaId,
    dispatch_source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
    stable_continuation_key_text: String,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    resume_payload_abi: AbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

impl<'ctx> ContinuationSurfaceResumeLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        continuation_schema: ContinuationSchemaId,
        dispatch_source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
        stable_continuation_key_text: String,
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        resume_payload_abi: AbiValue<'ctx>,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            continuation_schema,
            dispatch_source_kind,
            stable_continuation_key_text,
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

    pub(super) fn dispatch_source_kind(&self) -> LateLoweredSurfaceResumeDispatchSourceKind {
        self.dispatch_source_kind
    }

    pub(super) fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    pub(super) fn stable_continuation_key_text(&self) -> &str {
        &self.stable_continuation_key_text
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

    pub(super) fn resume_payload_abi(&self) -> &AbiValue<'ctx> {
        &self.resume_payload_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// surface-resume shared symbol 经过 continuation object 可回查到的 object-side packing method lookup。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContinuationSurfaceResumeMethodLookup {
    continuation_object: ContinuationObjectId,
    packing_interface_id: ResumeInterfaceId,
    packing_field_index: u32,
    case_tag: CaseTag,
    vtable_index: u32,
}

impl ContinuationSurfaceResumeMethodLookup {
    pub(super) fn new(
        continuation_object: ContinuationObjectId,
        packing_interface_id: ResumeInterfaceId,
        packing_field_index: u32,
        case_tag: CaseTag,
        vtable_index: u32,
    ) -> Self {
        Self {
            continuation_object,
            packing_interface_id,
            packing_field_index,
            case_tag,
            vtable_index,
        }
    }

    pub(super) fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }

    pub(super) fn packing_interface_id(&self) -> ResumeInterfaceId {
        self.packing_interface_id
    }

    pub(super) fn packing_field_index(&self) -> u32 {
        self.packing_field_index
    }

    pub(super) fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub(super) fn vtable_index(&self) -> u32 {
        self.vtable_index
    }
}

/// owner trampoline 继续分派到 handle continuation binder 时所需的已发布 route。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContinuationSurfaceResumeHandleBinderRoute {
    site_id: SiteId,
    arm_ordinal: u32,
    handled_case: CaseTag,
}

impl ContinuationSurfaceResumeHandleBinderRoute {
    pub(super) fn new(site_id: SiteId, arm_ordinal: u32, handled_case: CaseTag) -> Self {
        Self {
            site_id,
            arm_ordinal,
            handled_case,
        }
    }

    pub(super) fn site_id(&self) -> SiteId {
        self.site_id
    }

    pub(super) fn arm_ordinal(&self) -> u32 {
        self.arm_ordinal
    }

    pub(super) fn handled_case(&self) -> CaseTag {
        self.handled_case
    }
}

/// surface-resume shared symbol 继续进入 owner-specific lowering 时的 trampoline contract。
pub(super) struct ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx> {
    owner_version_key: LateLoweredBodyVersionKey,
    owner_root_fqn: String,
    owner_step_schema: StepSchemaId,
    owner_continuation_object: ContinuationObjectId,
    stable_owner_dispatch_key_text: String,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    resume_boundary_sites: Vec<SiteId>,
    handle_binder_routes: Vec<ContinuationSurfaceResumeHandleBinderRoute>,
    wrapper_projection: Option<LateLoweredSurfaceResumeWrapperProjection>,
}

impl<'ctx> ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        owner_version_key: LateLoweredBodyVersionKey,
        owner_root_fqn: String,
        owner_step_schema: StepSchemaId,
        owner_continuation_object: ContinuationObjectId,
        stable_owner_dispatch_key_text: String,
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        resume_boundary_sites: Vec<SiteId>,
        handle_binder_routes: Vec<ContinuationSurfaceResumeHandleBinderRoute>,
        wrapper_projection: Option<LateLoweredSurfaceResumeWrapperProjection>,
    ) -> Self {
        Self {
            owner_version_key,
            owner_root_fqn,
            owner_step_schema,
            owner_continuation_object,
            stable_owner_dispatch_key_text,
            symbol_name,
            llvm_ty,
            param_count,
            resume_boundary_sites,
            handle_binder_routes,
            wrapper_projection,
        }
    }

    pub(super) fn owner_version_key(&self) -> &LateLoweredBodyVersionKey {
        &self.owner_version_key
    }

    pub(super) fn owner_root_fqn(&self) -> &str {
        &self.owner_root_fqn
    }

    pub(super) fn owner_step_schema(&self) -> StepSchemaId {
        self.owner_step_schema
    }

    pub(super) fn owner_continuation_object(&self) -> ContinuationObjectId {
        self.owner_continuation_object
    }

    pub(super) fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    pub(super) fn stable_owner_dispatch_key_text(&self) -> &str {
        &self.stable_owner_dispatch_key_text
    }

    pub(super) fn llvm_ty(&self) -> FunctionType<'ctx> {
        self.llvm_ty
    }

    pub(super) fn param_count(&self) -> usize {
        self.param_count
    }

    pub(super) fn resume_boundary_sites(&self) -> &[SiteId] {
        &self.resume_boundary_sites
    }

    pub(super) fn handle_binder_routes(
        &self,
    ) -> &[ContinuationSurfaceResumeHandleBinderRoute] {
        &self.handle_binder_routes
    }

    pub(super) fn wrapper_projection(&self) -> Option<&LateLoweredSurfaceResumeWrapperProjection> {
        self.wrapper_projection.as_ref()
    }
}

/// `ContinuationSchemaId` authoritative 地路由到 owner-specific lowering target。
pub(super) enum ContinuationSurfaceResumeDispatchTarget<'ctx> {
    OwnerTrampoline(Box<ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>>),
    OwnerTrampolines(Vec<ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>>),
    Unreachable,
}

impl<'ctx> ContinuationSurfaceResumeDispatchTarget<'ctx> {
    pub(super) fn owner_trampolines(
        &self,
    ) -> &[ContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>] {
        match self {
            Self::OwnerTrampoline(target) => std::slice::from_ref(target.as_ref()),
            Self::OwnerTrampolines(targets) => targets,
            Self::Unreachable => &[],
        }
    }
}

/// shared surface-resume symbol 到 owner dispatch target 的稳定 LLVM query。
pub(super) struct ContinuationSurfaceResumeDispatchLayout<'ctx> {
    continuation_schema: ContinuationSchemaId,
    source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
    method_targets: Vec<ContinuationSurfaceResumeMethodLookup>,
    target: ContinuationSurfaceResumeDispatchTarget<'ctx>,
}

impl<'ctx> ContinuationSurfaceResumeDispatchLayout<'ctx> {
    pub(super) fn new(
        continuation_schema: ContinuationSchemaId,
        source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
        method_targets: Vec<ContinuationSurfaceResumeMethodLookup>,
        target: ContinuationSurfaceResumeDispatchTarget<'ctx>,
    ) -> Self {
        Self {
            continuation_schema,
            source_kind,
            method_targets,
            target,
        }
    }

    pub(super) fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub(super) fn source_kind(&self) -> LateLoweredSurfaceResumeDispatchSourceKind {
        self.source_kind
    }

    pub(super) fn method_targets(&self) -> &[ContinuationSurfaceResumeMethodLookup] {
        &self.method_targets
    }

    pub(super) fn target(&self) -> &ContinuationSurfaceResumeDispatchTarget<'ctx> {
        &self.target
    }
}

/// 单个 resume packing method 的 LLVM 级合同。
pub(super) struct ResumeMethodLayout<'ctx> {
    packing_interface_id: ResumeInterfaceId,
    case_tag: CaseTag,
    symbol_name: String,
    llvm_ty: FunctionType<'ctx>,
    param_count: usize,
    vtable_index: u32,
    resume_payload_abi: AbiValue<'ctx>,
    return_step_schema: StepSchemaId,
}

impl<'ctx> ResumeMethodLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        packing_interface_id: ResumeInterfaceId,
        case_tag: CaseTag,
        symbol_name: String,
        llvm_ty: FunctionType<'ctx>,
        param_count: usize,
        vtable_index: u32,
        resume_payload_abi: AbiValue<'ctx>,
        return_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            packing_interface_id,
            case_tag,
            symbol_name,
            llvm_ty,
            param_count,
            vtable_index,
            resume_payload_abi,
            return_step_schema,
        }
    }

    pub(super) fn packing_interface_id(&self) -> ResumeInterfaceId {
        self.packing_interface_id
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

    pub(super) fn resume_payload_abi(&self) -> &AbiValue<'ctx> {
        &self.resume_payload_abi
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }
}

/// 单个 internal resume packing 的 vtable / method 布局。
///
/// 这里保留 effect-family 分组只为 continuation object 上的 packing/vtable 物理布局服务，
/// 不能替代 `ContinuationSchemaId` / `CaseTag` 的 authoritative resume 语义入口。
pub(super) struct ResumeInterfaceLayout<'ctx> {
    packing_interface_id: ResumeInterfaceId,
    packing_family_fqn: String,
    llvm_vtable_ty: StructType<'ctx>,
    layout_anchor_name: String,
    methods: BTreeMap<CaseTag, ResumeMethodLayout<'ctx>>,
}

impl<'ctx> ResumeInterfaceLayout<'ctx> {
    pub(super) fn new(
        packing_interface_id: ResumeInterfaceId,
        packing_family_fqn: String,
        llvm_vtable_ty: StructType<'ctx>,
        layout_anchor_name: String,
        methods: BTreeMap<CaseTag, ResumeMethodLayout<'ctx>>,
    ) -> Self {
        Self {
            packing_interface_id,
            packing_family_fqn,
            llvm_vtable_ty,
            layout_anchor_name,
            methods,
        }
    }

    pub(super) fn packing_interface_id(&self) -> ResumeInterfaceId {
        self.packing_interface_id
    }

    pub(super) fn packing_family_fqn(&self) -> &str {
        &self.packing_family_fqn
    }

    pub(super) fn llvm_vtable_ty(&self) -> StructType<'ctx> {
        self.llvm_vtable_ty
    }

    pub(super) fn layout_anchor_name(&self) -> &str {
        &self.layout_anchor_name
    }

    pub(super) fn methods(&self) -> &BTreeMap<CaseTag, ResumeMethodLayout<'ctx>> {
        &self.methods
    }

    pub(super) fn method(&self, case_tag: CaseTag) -> Option<&ResumeMethodLayout<'ctx>> {
        self.methods.get(&case_tag)
    }
}

/// continuation object 上单个 surface `resume(...)` case 的 object-side 已发布映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContinuationSurfaceResumeBinding {
    continuation_schema: ContinuationSchemaId,
    return_step_schema: StepSchemaId,
    case_tag: CaseTag,
    reachability: LateLoweredContinuationMethodReachability,
}

impl ContinuationSurfaceResumeBinding {
    pub(super) fn new(
        continuation_schema: ContinuationSchemaId,
        return_step_schema: StepSchemaId,
        case_tag: CaseTag,
        reachability: LateLoweredContinuationMethodReachability,
    ) -> Self {
        Self {
            continuation_schema,
            return_step_schema,
            case_tag,
            reachability,
        }
    }

    pub(super) fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub(super) fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }

    pub(super) fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub(super) fn reachability(&self) -> LateLoweredContinuationMethodReachability {
        self.reachability
    }
}

/// continuation object field 的稳定分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContinuationFieldKind {
    Header,
    ResumedFlag,
    ResumeStateTag,
    CapturedEffectCtxRef,
    StateRef,
    StepFn,
    ResumeWord,
    ResumeGcRef,
    CapturedCalleeSuspendStateRef,
    PackingVtable(ResumeInterfaceId),
}

/// 单个 continuation object field 的 LLVM 布局。
pub(super) struct ContinuationFieldLayout<'ctx> {
    field_index: u32,
    kind: ContinuationFieldKind,
    llvm_ty: BasicTypeEnum<'ctx>,
}

impl<'ctx> ContinuationFieldLayout<'ctx> {
    pub(super) fn new(
        field_index: u32,
        kind: ContinuationFieldKind,
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

    pub(super) fn kind(&self) -> ContinuationFieldKind {
        self.kind
    }

    pub(super) fn llvm_ty(&self) -> BasicTypeEnum<'ctx> {
        self.llvm_ty
    }
}

/// 单个 continuation object 的 LLVM 布局。
pub(super) struct ContinuationObjectLayout<'ctx> {
    object_id: ContinuationObjectId,
    owner_step_schema: StepSchemaId,
    llvm_ty: StructType<'ctx>,
    layout_anchor_name: String,
    fields: Vec<ContinuationFieldLayout<'ctx>>,
    packing_field_indices: BTreeMap<ResumeInterfaceId, u32>,
    surface_resume_bindings:
        BTreeMap<ContinuationSchemaId, Vec<ContinuationSurfaceResumeBinding>>,
}

impl<'ctx> ContinuationObjectLayout<'ctx> {
    pub(super) fn new(
        object_id: ContinuationObjectId,
        owner_step_schema: StepSchemaId,
        llvm_ty: StructType<'ctx>,
        layout_anchor_name: String,
        fields: Vec<ContinuationFieldLayout<'ctx>>,
        packing_field_indices: BTreeMap<ResumeInterfaceId, u32>,
        surface_resume_bindings: BTreeMap<
            ContinuationSchemaId,
            Vec<ContinuationSurfaceResumeBinding>,
        >,
    ) -> Self {
        Self {
            object_id,
            owner_step_schema,
            llvm_ty,
            layout_anchor_name,
            fields,
            packing_field_indices,
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

    pub(super) fn fields(&self) -> &[ContinuationFieldLayout<'ctx>] {
        &self.fields
    }

    pub(super) fn field_index_for_packing(
        &self,
        packing_interface_id: ResumeInterfaceId,
    ) -> Option<u32> {
        self.packing_field_indices
            .get(&packing_interface_id)
            .copied()
    }

    pub(super) fn surface_resume_bindings(
        &self,
        continuation_schema: ContinuationSchemaId,
    ) -> Option<&[ContinuationSurfaceResumeBinding]> {
        self.surface_resume_bindings
            .get(&continuation_schema)
            .map(Vec::as_slice)
    }
}

/// 单个 callable version 暴露给后续 body emitter 的 LLVM ABI 查询面。
pub(super) struct CallableLayout<'ctx> {
    root_fqn: String,
    body_version_key: LateLoweredBodyVersionKey,
    stable_callable_key_text: String,
    step_schema: StepSchemaId,
    dynamic_entry: CallableEntryLayout<'ctx>,
    direct_entry: CallableEntryLayout<'ctx>,
    continuation_object: ContinuationObjectId,
    resume_packings: Vec<ResumeInterfaceId>,
}

impl<'ctx> CallableLayout<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        root_fqn: String,
        body_version_key: LateLoweredBodyVersionKey,
        stable_callable_key_text: String,
        step_schema: StepSchemaId,
        dynamic_entry: CallableEntryLayout<'ctx>,
        direct_entry: CallableEntryLayout<'ctx>,
        continuation_object: ContinuationObjectId,
        resume_packings: Vec<ResumeInterfaceId>,
    ) -> Self {
        Self {
            root_fqn,
            body_version_key,
            stable_callable_key_text,
            step_schema,
            dynamic_entry,
            direct_entry,
            continuation_object,
            resume_packings,
        }
    }

    pub(super) fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub(super) fn body_version_key(&self) -> &LateLoweredBodyVersionKey {
        &self.body_version_key
    }

    pub(super) fn stable_callable_key_text(&self) -> &str {
        &self.stable_callable_key_text
    }

    pub(super) fn surface_instance(&self) -> &InstanceKey {
        self.body_version_key.surface_instance()
    }

    pub(super) fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub(super) fn dynamic_entry(&self) -> &CallableEntryLayout<'ctx> {
        &self.dynamic_entry
    }

    pub(super) fn direct_entry(&self) -> &CallableEntryLayout<'ctx> {
        &self.direct_entry
    }

    pub(super) fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }

    pub(super) fn resume_packings(&self) -> &[ResumeInterfaceId] {
        &self.resume_packings
    }
}

/// 单个 plain callable version 暴露给 body emitter 的普通 ABI 查询面。
pub(super) struct PlainCallableLayout<'ctx> {
    root_fqn: String,
    body_version_key: LateLoweredBodyVersionKey,
    stable_callable_key_text: String,
    direct_entry: PlainCallableEntryLayout<'ctx>,
}

impl<'ctx> PlainCallableLayout<'ctx> {
    pub(super) fn new(
        root_fqn: String,
        body_version_key: LateLoweredBodyVersionKey,
        stable_callable_key_text: String,
        direct_entry: PlainCallableEntryLayout<'ctx>,
    ) -> Self {
        Self {
            root_fqn,
            body_version_key,
            stable_callable_key_text,
            direct_entry,
        }
    }

    pub(super) fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub(super) fn body_version_key(&self) -> &LateLoweredBodyVersionKey {
        &self.body_version_key
    }

    pub(super) fn stable_callable_key_text(&self) -> &str {
        &self.stable_callable_key_text
    }

    pub(super) fn surface_instance(&self) -> &InstanceKey {
        self.body_version_key.surface_instance()
    }

    pub(super) fn direct_entry(&self) -> &PlainCallableEntryLayout<'ctx> {
        &self.direct_entry
    }
}

/// runtime callable carrier 对应的 canonical dynamic entry target contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallableCarrierTargetLayout {
    callable_fqn: String,
    body_version_key: LateLoweredBodyVersionKey,
    step_schema: StepSchemaId,
    symbol_name: String,
}

impl CallableCarrierTargetLayout {
    pub(super) fn new(
        callable_fqn: String,
        body_version_key: LateLoweredBodyVersionKey,
        step_schema: StepSchemaId,
        symbol_name: String,
    ) -> Self {
        Self {
            callable_fqn,
            body_version_key,
            step_schema,
            symbol_name,
        }
    }

    pub(super) fn callable_fqn(&self) -> &str {
        &self.callable_fqn
    }

    pub(super) fn body_version_key(&self) -> &LateLoweredBodyVersionKey {
        &self.body_version_key
    }

    pub(super) fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub(super) fn symbol_name(&self) -> &str {
        &self.symbol_name
    }
}

/// effect-lowered LLVM type/layout 层对下游 body emitter 暴露的稳定查询面。
pub(crate) struct ProgramAbiQuery<'ctx> {
    source_value_layouts: BTreeMap<TypeId, SourceAbiLayout<'ctx>>,
    class_instance_layouts: BTreeMap<TypeId, ClassInstanceLayout>,
    step_layouts: BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    frame_layouts: BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
    continuation_layouts: BTreeMap<ContinuationObjectId, ContinuationObjectLayout<'ctx>>,
    resume_packing_layouts: BTreeMap<ResumeInterfaceId, ResumeInterfaceLayout<'ctx>>,
    surface_resume_layouts:
        BTreeMap<ContinuationSchemaId, ContinuationSurfaceResumeLayout<'ctx>>,
    surface_resume_dispatch_layouts:
        BTreeMap<ContinuationSchemaId, ContinuationSurfaceResumeDispatchLayout<'ctx>>,
    callable_layouts: BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
    callable_layouts_by_version_key: HashMap<LateLoweredBodyVersionKey, StepSchemaId>,
    plain_local_effect_step_schemas_by_version_key:
        HashMap<LateLoweredBodyVersionKey, StepSchemaId>,
    plain_callable_layouts_by_version_key:
        HashMap<LateLoweredBodyVersionKey, PlainCallableLayout<'ctx>>,
    known_instance_callable_versions:
        HashMap<(InstanceKey, StepSchemaId), LateLoweredBodyVersionKey>,
    callable_carrier_target_layouts:
        HashMap<(CallableCarrierKind, String), CallableCarrierTargetLayout>,
    dynamic_invoke_layouts: BTreeMap<(StepSchemaId, SiteId), DynamicInvokeLayout<'ctx>>,
    call_boundary_operand_layouts:
        BTreeMap<(StepSchemaId, SiteId), CallBoundaryOperandLayout>,
    perform_boundary_operand_layouts:
        BTreeMap<(StepSchemaId, SiteId), PerformBoundaryOperandLayout>,
    resume_boundary_operand_layouts:
        BTreeMap<(StepSchemaId, SiteId), ResumeBoundaryOperandLayout>,
    resume_payload_binding_layouts:
        BTreeMap<(StepSchemaId, BoundaryId), ResumePayloadBindingLayout>,
    resume_payload_bindings_by_state:
        BTreeMap<(StepSchemaId, StateId), ResumePayloadBindingLayout>,
    completion_payload_binding_layouts:
        BTreeMap<(StepSchemaId, StateId), CompletionPayloadBindingLayout<'ctx>>,
    local_runtime_error_contracts:
        BTreeMap<(StepSchemaId, SiteId), LocalRuntimeErrorContract<'ctx>>,
    handle_dispatch_layouts: BTreeMap<(StepSchemaId, SiteId), HandleDispatchLayout>,
}

impl<'ctx> ProgramAbiQuery<'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        source_value_layouts: BTreeMap<TypeId, SourceAbiLayout<'ctx>>,
        class_instance_layouts: BTreeMap<TypeId, ClassInstanceLayout>,
        step_layouts: BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        frame_layouts: BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        continuation_layouts: BTreeMap<
            ContinuationObjectId,
            ContinuationObjectLayout<'ctx>,
        >,
        resume_packing_layouts: BTreeMap<ResumeInterfaceId, ResumeInterfaceLayout<'ctx>>,
        surface_resume_layouts: BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
        surface_resume_dispatch_layouts: BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeDispatchLayout<'ctx>,
        >,
        callable_layouts: BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        callable_layouts_by_version_key: HashMap<LateLoweredBodyVersionKey, StepSchemaId>,
        plain_local_effect_step_schemas_by_version_key: HashMap<
            LateLoweredBodyVersionKey,
            StepSchemaId,
        >,
        plain_callable_layouts_by_version_key: HashMap<
            LateLoweredBodyVersionKey,
            PlainCallableLayout<'ctx>,
        >,
        known_instance_callable_versions: HashMap<
            (InstanceKey, StepSchemaId),
            LateLoweredBodyVersionKey,
        >,
        callable_carrier_target_layouts: HashMap<
            (CallableCarrierKind, String),
            CallableCarrierTargetLayout,
        >,
        dynamic_invoke_layouts: BTreeMap<(StepSchemaId, SiteId), DynamicInvokeLayout<'ctx>>,
        call_boundary_operand_layouts: BTreeMap<
            (StepSchemaId, SiteId),
            CallBoundaryOperandLayout,
        >,
        perform_boundary_operand_layouts: BTreeMap<
            (StepSchemaId, SiteId),
            PerformBoundaryOperandLayout,
        >,
        resume_boundary_operand_layouts: BTreeMap<
            (StepSchemaId, SiteId),
            ResumeBoundaryOperandLayout,
        >,
        resume_payload_binding_layouts: BTreeMap<
            (StepSchemaId, BoundaryId),
            ResumePayloadBindingLayout,
        >,
        resume_payload_bindings_by_state: BTreeMap<
            (StepSchemaId, StateId),
            ResumePayloadBindingLayout,
        >,
        completion_payload_binding_layouts: BTreeMap<
            (StepSchemaId, StateId),
            CompletionPayloadBindingLayout<'ctx>,
        >,
        local_runtime_error_contracts: BTreeMap<
            (StepSchemaId, SiteId),
            LocalRuntimeErrorContract<'ctx>,
        >,
        handle_dispatch_layouts: BTreeMap<(StepSchemaId, SiteId), HandleDispatchLayout>,
    ) -> Self {
        Self {
            source_value_layouts,
            class_instance_layouts,
            step_layouts,
            frame_layouts,
            continuation_layouts,
            resume_packing_layouts,
            surface_resume_layouts,
            surface_resume_dispatch_layouts,
            callable_layouts,
            callable_layouts_by_version_key,
            plain_local_effect_step_schemas_by_version_key,
            plain_callable_layouts_by_version_key,
            known_instance_callable_versions,
            callable_carrier_target_layouts,
            dynamic_invoke_layouts,
            call_boundary_operand_layouts,
            perform_boundary_operand_layouts,
            resume_boundary_operand_layouts,
            resume_payload_binding_layouts,
            resume_payload_bindings_by_state,
            completion_payload_binding_layouts,
            local_runtime_error_contracts,
            handle_dispatch_layouts,
        }
    }

    pub(super) fn source_value_layout(
        &self,
        source_ty: TypeId,
    ) -> Result<&SourceAbiLayout<'ctx>, LlvmEmitError> {
        self.source_value_layouts
            .get(&source_ty)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 source type {} 的 ABI value lowering contract",
                    source_ty.as_u32()
                ),
            })
    }

    pub(super) fn class_instance_layout(
        &self,
        source_ty: TypeId,
    ) -> Result<&ClassInstanceLayout, LlvmEmitError> {
        self.class_instance_layouts
            .get(&source_ty)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 source type {} 的 concrete class instance layout contract",
                    source_ty.as_u32()
                ),
            })
    }

    pub(super) fn step_layout(
        &self,
        step_schema: StepSchemaId,
    ) -> Option<&StepLayout<'ctx>> {
        self.step_layouts.get(&step_schema)
    }

    pub(super) fn step_layouts(&self) -> impl Iterator<Item = &StepLayout<'ctx>> {
        self.step_layouts.values()
    }

    pub(super) fn dynamic_invoke_layouts(
        &self,
    ) -> impl Iterator<Item = &DynamicInvokeLayout<'ctx>> {
        self.dynamic_invoke_layouts.values()
    }

    pub(super) fn frame_layout(
        &self,
        step_schema: StepSchemaId,
    ) -> Option<&FrameLayout<'ctx>> {
        self.frame_layouts.get(&step_schema)
    }

    pub(super) fn local_effect_step_schema_by_version_key(
        &self,
        key: &LateLoweredBodyVersionKey,
    ) -> Option<StepSchemaId> {
        self.plain_local_effect_step_schemas_by_version_key
            .get(key)
            .copied()
    }

    pub(super) fn continuation_layout(
        &self,
        object_id: ContinuationObjectId,
    ) -> Option<&ContinuationObjectLayout<'ctx>> {
        self.continuation_layouts.get(&object_id)
    }

    pub(super) fn resume_packing_layout(
        &self,
        packing_interface_id: ResumeInterfaceId,
    ) -> Option<&ResumeInterfaceLayout<'ctx>> {
        self.resume_packing_layouts.get(&packing_interface_id)
    }

    pub(super) fn surface_resume_layout(
        &self,
        continuation_schema: ContinuationSchemaId,
    ) -> Option<&ContinuationSurfaceResumeLayout<'ctx>> {
        self.surface_resume_layouts.get(&continuation_schema)
    }

    pub(super) fn surface_resume_layouts(
        &self,
    ) -> impl Iterator<Item = &ContinuationSurfaceResumeLayout<'ctx>> {
        self.surface_resume_layouts.values()
    }

    pub(super) fn unique_surface_resume_layout_for_signature(
        &self,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        context: &str,
    ) -> Result<&ContinuationSurfaceResumeLayout<'ctx>, LlvmEmitError> {
        let mut matches = self.surface_resume_layouts.values().filter(|layout| {
            layout.resume_tuple_ty() == resume_tuple_ty && layout.answer_ty() == answer_ty
        });
        let Some(first) = matches.next() else {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 {context} 需要的 surface-resume contract: resume_ty=t{} answer_ty=t{}",
                    resume_tuple_ty.as_u32(),
                    answer_ty.as_u32()
                ),
            });
        };
        if let Some(second) = matches.next() {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 {context} 的 surface-resume contract 多义：k{} 与 k{} 都匹配 resume_ty=t{} answer_ty=t{}",
                    first.continuation_schema().as_u32(),
                    second.continuation_schema().as_u32(),
                    resume_tuple_ty.as_u32(),
                    answer_ty.as_u32()
                ),
            });
        }
        Ok(first)
    }

    pub(super) fn unique_surface_resume_layout_for_equivalent_signature(
        &self,
        types: &crate::ty::TypeStore,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        context: &str,
    ) -> Result<&ContinuationSurfaceResumeLayout<'ctx>, LlvmEmitError> {
        if let Ok(layout) =
            self.unique_surface_resume_layout_for_signature(resume_tuple_ty, answer_ty, context)
        {
            return Ok(layout);
        }

        let resume_display = types.display(resume_tuple_ty).to_string();
        let answer_display = types.display(answer_ty).to_string();
        let mut matches = self.surface_resume_layouts.values().filter(|layout| {
            types.display(layout.resume_tuple_ty()).to_string() == resume_display
                && types.display(layout.answer_ty()).to_string() == answer_display
        });
        let Some(first) = matches.next() else {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 {context} 需要的 surface-resume contract: resume_ty=t{} ({}) answer_ty=t{} ({})",
                    resume_tuple_ty.as_u32(),
                    resume_display,
                    answer_ty.as_u32(),
                    answer_display,
                ),
            });
        };
        if let Some(second) = matches.next() {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 {context} 的等价 surface-resume contract 多义：k{} 与 k{} 都匹配 resume_ty={} answer_ty={}",
                    first.continuation_schema().as_u32(),
                    second.continuation_schema().as_u32(),
                    resume_display,
                    answer_display,
                ),
            });
        }
        Ok(first)
    }

    pub(super) fn surface_resume_dispatch_layout(
        &self,
        continuation_schema: ContinuationSchemaId,
    ) -> Result<&ContinuationSurfaceResumeDispatchLayout<'ctx>, LlvmEmitError> {
        self.surface_resume_dispatch_layouts
            .get(&continuation_schema)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 continuation schema k{} 的 surface-resume owner dispatch contract",
                    continuation_schema.as_u32()
                ),
            })
    }

    pub(super) fn surface_resume_dispatch_layouts(
        &self,
    ) -> impl Iterator<Item = &ContinuationSurfaceResumeDispatchLayout<'ctx>> {
        self.surface_resume_dispatch_layouts.values()
    }

    pub(super) fn surface_resume_method_layout(
        &self,
        lookup: ContinuationSurfaceResumeMethodLookup,
    ) -> Result<&ResumeMethodLayout<'ctx>, LlvmEmitError> {
        let packing = self.resume_packing_layout(lookup.packing_interface_id()).ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 continuation object ko{} surface-resume lookup 需要的 resume packing ri{}",
                    lookup.continuation_object().as_u32(),
                    lookup.packing_interface_id().as_u32(),
                ),
            }
        })?;
        let method = packing.method(lookup.case_tag()).ok_or_else(|| LlvmEmitError::Frontend {
            message: format!(
                "LLVM ABI query 缺少 continuation object ko{} surface-resume lookup 需要的 resume packing ri{}::c{} method layout",
                lookup.continuation_object().as_u32(),
                lookup.packing_interface_id().as_u32(),
                lookup.case_tag().as_u32(),
            ),
        })?;
        if method.packing_interface_id() != lookup.packing_interface_id()
            || method.vtable_index() != lookup.vtable_index()
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 continuation object ko{} 的 surface-resume packing lookup 漂移：lookup=(ri{}, field={}, case=c{}, vtable_index={})，layout=(ri{}, case=c{}, vtable_index={})",
                    lookup.continuation_object().as_u32(),
                    lookup.packing_interface_id().as_u32(),
                    lookup.packing_field_index(),
                    lookup.case_tag().as_u32(),
                    lookup.vtable_index(),
                    method.packing_interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.vtable_index(),
                ),
            });
        }
        Ok(method)
    }

    pub(super) fn callable_layout(
        &self,
        step_schema: StepSchemaId,
    ) -> Option<&CallableLayout<'ctx>> {
        self.callable_layouts.get(&step_schema)
    }

    pub(super) fn callable_layout_by_root_fqn(
        &self,
        root_fqn: &str,
    ) -> Result<&CallableLayout<'ctx>, LlvmEmitError> {
        let matches = self
            .callable_layouts
            .values()
            .filter(|layout| layout.root_fqn() == root_fqn)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 callable `{root_fqn}` 的 published callable version"
                ),
            }),
            [layout] => Ok(*layout),
            _ => Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 callable `{root_fqn}` 存在多个 published callable version；请改用 body version key 查询"
                ),
            }),
        }
    }

    pub(super) fn callable_layout_by_version_key(
        &self,
        version_key: &LateLoweredBodyVersionKey,
    ) -> Result<&CallableLayout<'ctx>, LlvmEmitError> {
        let step_schema = self
            .callable_layouts_by_version_key
            .get(version_key)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 body version key {:?} 的 callable layout",
                    version_key
                ),
            })?;
        self.callable_layout(*step_schema)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 body version key {:?} 指向缺失的 callable step schema s{}",
                    version_key,
                    step_schema.as_u32()
                ),
            })
    }

    pub(super) fn plain_callable_layout_by_version_key(
        &self,
        version_key: &LateLoweredBodyVersionKey,
    ) -> Result<&PlainCallableLayout<'ctx>, LlvmEmitError> {
        self.plain_callable_layouts_by_version_key
            .get(version_key)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 body version key {:?} 的 plain callable layout",
                    version_key
                ),
            })
    }

    pub(super) fn plain_callable_layout_by_root_fqn(
        &self,
        root_fqn: &str,
    ) -> Result<&PlainCallableLayout<'ctx>, LlvmEmitError> {
        let matches = self
            .plain_callable_layouts_by_version_key
            .values()
            .filter(|layout| layout.root_fqn() == root_fqn)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 plain callable `{root_fqn}` 的 published ordinary callable version"
                ),
            }),
            [layout] => Ok(*layout),
            _ => Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 plain callable `{root_fqn}` 存在多个 published callable version；请改用 body version key 查询"
                ),
            }),
        }
    }

    pub(super) fn maybe_plain_callable_layout_by_root_fqn(
        &self,
        root_fqn: &str,
    ) -> Result<Option<&PlainCallableLayout<'ctx>>, LlvmEmitError> {
        let matches = self
            .plain_callable_layouts_by_version_key
            .values()
            .filter(|layout| layout.root_fqn() == root_fqn)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [layout] => Ok(Some(*layout)),
            _ => Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 plain callable `{root_fqn}` 存在多个 published callable version；请改用 body version key 查询"
                ),
            }),
        }
    }

    pub(super) fn callable_carrier_target_layout(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<&CallableCarrierTargetLayout, LlvmEmitError> {
        self.callable_carrier_target_layouts
            .get(&(kind, callable_fqn.to_string()))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 {} `{}` 的 callable version 选择 contract",
                    kind.label(),
                    callable_fqn,
                ),
            })
    }

    pub(super) fn maybe_callable_carrier_target_layout(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Option<&CallableCarrierTargetLayout> {
        self.callable_carrier_target_layouts
            .get(&(kind, callable_fqn.to_string()))
    }

    pub(super) fn callable_carrier_target_layouts(
        &self,
    ) -> impl Iterator<
        Item = (
            CallableCarrierKind,
            &str,
            &CallableCarrierTargetLayout,
        ),
    > + '_ {
        self.callable_carrier_target_layouts
            .iter()
            .map(|((kind, callable_fqn), layout)| (*kind, callable_fqn.as_str(), layout))
    }

    pub(super) fn dynamic_invoke_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
    ) -> Option<&DynamicInvokeLayout<'ctx>> {
        self.dynamic_invoke_layouts
            .get(&(owner_step_schema, site_id))
    }

    pub(super) fn call_boundary_operand_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LateLoweredCallBoundaryOperandContract,
    ) -> Result<&CallBoundaryOperandLayout, LlvmEmitError> {
        let published = self
            .call_boundary_operand_layouts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} call site {} 的 boundary operand contract",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                ),
            })?;
        if published.contract() != contract {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 owner step schema s{} call site {} 的 boundary operand contract 漂移：published={:?}，lowering={:?}",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                    published.contract(),
                    contract,
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn perform_boundary_operand_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LateLoweredPerformBoundaryOperandContract,
    ) -> Result<&PerformBoundaryOperandLayout, LlvmEmitError> {
        let published = self
            .perform_boundary_operand_layouts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} perform site {} 的 boundary operand contract",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                ),
            })?;
        if published.contract() != contract {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 owner step schema s{} perform site {} 的 boundary operand contract 漂移：published={:?}，lowering={:?}",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                    published.contract(),
                    contract,
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn resume_boundary_operand_layout(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LateLoweredResumeBoundaryOperandContract,
    ) -> Result<&ResumeBoundaryOperandLayout, LlvmEmitError> {
        let published = self
            .resume_boundary_operand_layouts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} resume site {} 的 boundary operand contract",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                ),
            })?;
        if published.contract() != contract {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 owner step schema s{} resume site {} 的 boundary operand contract 漂移：published={:?}，lowering={:?}",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                    published.contract(),
                    contract,
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn resume_payload_binding_layout(
        &self,
        owner_step_schema: StepSchemaId,
        binding: &LateLoweredResumePayloadBinding,
    ) -> Result<&ResumePayloadBindingLayout, LlvmEmitError> {
        let published = self
            .resume_payload_binding_layouts
            .get(&(owner_step_schema, binding.boundary_id()))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} boundary bd{} 的 resumed local/home contract",
                    owner_step_schema.as_u32(),
                    binding.boundary_id().as_u32(),
                ),
            })?;
        if published.binding() != binding {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 owner step schema s{} boundary bd{} 的 resumed local/home contract 漂移：published={:?}，lowered={:?}",
                    owner_step_schema.as_u32(),
                    binding.boundary_id().as_u32(),
                    published.binding(),
                    binding,
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn resume_payload_binding_for_state(
        &self,
        owner_step_schema: StepSchemaId,
        resume_state: StateId,
    ) -> Result<&ResumePayloadBindingLayout, LlvmEmitError> {
        self.resume_payload_bindings_by_state
            .get(&(owner_step_schema, resume_state))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} resume state st{} 的 resumed local/home contract",
                    owner_step_schema.as_u32(),
                    resume_state.as_u32(),
                ),
            })
    }

    pub(super) fn completion_payload_binding_layout(
        &self,
        owner_step_schema: StepSchemaId,
        binding: &LateLoweredCompletionPayloadBinding,
    ) -> Result<&CompletionPayloadBindingLayout<'ctx>, LlvmEmitError> {
        let published = self
            .completion_payload_binding_layouts
            .get(&(owner_step_schema, binding.return_state()))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} return state st{} 的 completion payload contract",
                    owner_step_schema.as_u32(),
                    binding.return_state().as_u32(),
                ),
            })?;
        if published.binding() != binding {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 owner step schema s{} return state st{} 的 completion payload contract 漂移：published={:?}，lowered={:?}",
                    owner_step_schema.as_u32(),
                    binding.return_state().as_u32(),
                    published.binding(),
                    binding,
                ),
            });
        }
        Ok(published)
    }

    pub(super) fn completion_payload_binding_for_state(
        &self,
        owner_step_schema: StepSchemaId,
        return_state: StateId,
    ) -> Result<&CompletionPayloadBindingLayout<'ctx>, LlvmEmitError> {
        self.completion_payload_binding_layouts
            .get(&(owner_step_schema, return_state))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} return state st{} 的 completion payload contract",
                    owner_step_schema.as_u32(),
                    return_state.as_u32(),
                ),
            })
    }

    pub(super) fn call_local_runtime_error_contract(
        &self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LateLoweredConsumedRuntimeErrorCase,
    ) -> Result<&LocalRuntimeErrorContract<'ctx>, LlvmEmitError> {
        let published = self
            .local_runtime_error_contracts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} call site {} 的 local runtime-error contract",
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
                    "LLVM ABI query 发现 owner step schema s{} call site {} 的 local runtime-error contract 漂移：layout=(input_case=c{}, payload_tuple_ty={}, terminal_action={:?}, target_state=st{})，lowering=(input_case=c{}, payload_tuple_ty={}, terminal_action={:?}, target_state=st{})",
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
    ) -> Result<&HandleDispatchLayout, LlvmEmitError> {
        let published = self
            .handle_dispatch_layouts
            .get(&(owner_step_schema, site_id))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 缺少 owner step schema s{} handle site {} 的 HandleDispatch contract",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                ),
            })?;
        if published.lowered_contract() != contract {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM ABI query 发现 owner step schema s{} handle site {} 的 HandleDispatch contract 漂移：published={:?}，lowered={:?}",
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
    ) -> Result<CallTargetQuery<'_, 'ctx>, LlvmEmitError> {
        match facts.target() {
            crate::effect_facts::CallSiteTarget::KnownInstance(instance) => {
                let version_key = self
                    .known_instance_callable_versions
                    .get(&(instance.clone(), facts.callee_schema()))
                    .or_else(|| {
                        let mut matches = self
                            .known_instance_callable_versions
                            .iter()
                            .filter(|((candidate, _), _)| candidate == instance)
                            .map(|(_, version)| version);
                        let first = matches.next()?;
                        matches.next().is_none().then_some(first)
                    })
                    .ok_or_else(|| LlvmEmitError::Frontend {
                        message: format!(
                            "LLVM ABI query 缺少 known-instance call target `{:?}` + callee step schema s{} 的 callable version selector",
                            instance,
                            facts.callee_schema().as_u32()
                        ),
                    })?;
                let layout = self.callable_layout_by_version_key(version_key)?;
                if layout.surface_instance() != instance {
                    return Err(LlvmEmitError::Frontend {
                        message: format!(
                            "LLVM ABI query 发现 known-instance call target `{:?}` + s{} 的 callable version selector 漂移：layout=(instance={:?}, step_schema=s{}, version={:?})",
                            instance,
                            facts.callee_schema().as_u32(),
                            layout.surface_instance(),
                            layout.step_schema().as_u32(),
                            layout.body_version_key(),
                        ),
                    });
                }
                if layout.dynamic_entry().invoke_args_tuple_ty() != facts.invoke_args_tuple_ty() {
                    return Err(LlvmEmitError::Frontend {
                        message: format!(
                            "LLVM ABI query 发现 known-instance call target `{:?}` 的 dynamic entry contract 漂移：layout=(invoke_args_tuple_ty={}, return_step_schema={}, version={:?})，facts=(invoke_args_tuple_ty={}, callee_step_schema={})",
                            instance,
                            layout.dynamic_entry().invoke_args_tuple_ty().as_u32(),
                            layout.dynamic_entry().return_step_schema().as_u32(),
                            layout.body_version_key(),
                            facts.invoke_args_tuple_ty().as_u32(),
                            facts.callee_schema().as_u32(),
                        ),
                    });
                }
                Ok(CallTargetQuery::KnownInstance(layout))
            }
            crate::effect_facts::CallSiteTarget::CandidateSet(_)
            | crate::effect_facts::CallSiteTarget::DynamicFallback => {
                let layout = self.dynamic_invoke_layout(owner_step_schema, site_id).ok_or_else(
                    || LlvmEmitError::Frontend {
                        message: format!(
                            "LLVM ABI query 缺少 owner step schema s{} call site {} 的 dynamic-invoke contract",
                            owner_step_schema.as_u32(),
                            site_id.as_u32(),
                        ),
                    },
                )?;
                if layout.target_mode() != facts.target_mode()
                    || layout.invoke_args_tuple_ty() != facts.invoke_args_tuple_ty()
                {
                    return Err(LlvmEmitError::Frontend {
                        message: format!(
                            "LLVM ABI query 发现 owner step schema s{} call site {} 的 dynamic-invoke contract 漂移：layout=(target_mode={:?}, invoke_args_tuple_ty={}, return_step_schema={})，facts=(target_mode={:?}, invoke_args_tuple_ty={}, callee_step_schema={})",
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
                Ok(CallTargetQuery::DynamicInvoke(layout))
            }
        }
    }
}
