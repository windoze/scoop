use std::collections::BTreeMap;

use crate::effect_facts::{
    CallSiteEffectFacts, CaseTag, ConcreteOpKey, ContinuationSchemaId, EffectFamilyKey,
    HandleSiteEffectFacts, ImplPlan, PerformSiteEffectFacts, ResumeSiteEffectFacts, StepSchemaId,
};
use crate::mir::{BasicBlockId, ConstValue, InstanceKey, LocalId, SiteId};
use crate::span::Span;
use crate::ty::{EffectRow, TypeId};

/// P5 late-lowering 阶段的顶层中间表示。
///
/// 该容器显式区分两层 contract：
/// - authoritative per-op/per-schema contract：`Step_F` shell、continuation object surface/internal
///   resume publication、以及 shared `ContinuationSchemaId` -> dispatch inventory；
/// - optional packing layer：按 effect family 分组的 compiler-owned `LateLoweredResumeInterface`
///   helper，仅用于 object layout / query / completeness 校验，不再是 reverse-resume 语义主键。
///
/// 后续 T03-T06 只能继续在这些类型里补算法和内容，而不能再另起一套临时 IR。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredProgram {
    step_types: Vec<LateLoweredStepType>,
    resume_packings: Vec<LateLoweredResumeInterface>,
    continuation_objects: Vec<LateLoweredContinuationObject>,
    surface_resume_dispatch_inventory: Vec<LateLoweredSurfaceResumeDispatchInventoryEntry>,
    callables: Vec<LateLoweredCallable>,
}

impl LateLoweredProgram {
    pub(crate) fn new(
        step_types: Vec<LateLoweredStepType>,
        resume_packings: Vec<LateLoweredResumeInterface>,
        continuation_objects: Vec<LateLoweredContinuationObject>,
        callables: Vec<LateLoweredCallable>,
    ) -> Self {
        let surface_resume_dispatch_inventory =
            build_surface_resume_dispatch_inventory(&step_types, &continuation_objects, &callables);
        Self {
            step_types,
            resume_packings,
            continuation_objects,
            surface_resume_dispatch_inventory,
            callables,
        }
    }

    pub fn step_types(&self) -> &[LateLoweredStepType] {
        &self.step_types
    }

    pub fn step_type(&self, step_schema: StepSchemaId) -> Option<&LateLoweredStepType> {
        self.step_types
            .iter()
            .find(|step_type| step_type.step_schema() == step_schema)
    }

    pub fn resume_packings(&self) -> &[LateLoweredResumeInterface] {
        &self.resume_packings
    }

    /// 兼容旧调用点；新的 handoff 应优先使用 `resume_packings()` 叙事。
    pub fn resume_interfaces(&self) -> &[LateLoweredResumeInterface] {
        self.resume_packings()
    }

    pub fn resume_packing(
        &self,
        interface_id: ResumeInterfaceId,
    ) -> Option<&LateLoweredResumeInterface> {
        self.resume_packings
            .iter()
            .find(|interface| interface.interface_id() == interface_id)
    }

    /// 兼容旧调用点；新的 handoff 应优先使用 `resume_packing(...)` 叙事。
    pub fn resume_interface(
        &self,
        interface_id: ResumeInterfaceId,
    ) -> Option<&LateLoweredResumeInterface> {
        self.resume_packing(interface_id)
    }

    pub fn continuation_objects(&self) -> &[LateLoweredContinuationObject] {
        &self.continuation_objects
    }

    pub fn continuation_object(
        &self,
        object_id: ContinuationObjectId,
    ) -> Option<&LateLoweredContinuationObject> {
        self.continuation_objects
            .iter()
            .find(|object| object.object_id() == object_id)
    }

    pub fn surface_resume_dispatch_inventory(
        &self,
    ) -> &[LateLoweredSurfaceResumeDispatchInventoryEntry] {
        &self.surface_resume_dispatch_inventory
    }

    pub fn surface_resume_dispatch(
        &self,
        continuation_schema: ContinuationSchemaId,
    ) -> Option<&LateLoweredSurfaceResumeDispatchInventoryEntry> {
        self.surface_resume_dispatch_inventory
            .iter()
            .find(|entry| entry.continuation_schema() == continuation_schema)
    }

    pub fn callables(&self) -> &[LateLoweredCallable] {
        &self.callables
    }

    pub fn callable(&self, root_fqn: &str) -> Option<&LateLoweredCallable> {
        self.callables
            .iter()
            .find(|callable| callable.root_fqn() == root_fqn)
    }

    pub fn callable_by_version_key(
        &self,
        version_key: &LateLoweredBodyVersionKey,
    ) -> Option<&LateLoweredCallable> {
        self.callables
            .iter()
            .find(|callable| callable.body_version_key() == version_key)
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    /// 返回 late-lowered program 的稳定文本 surface，供后续 dump/snapshot/测试复用。
    pub fn stable_dump(&self) -> String {
        super::dump::render_late_lowered_program(self)
    }
}

/// callable surface instance 在 P5 中对应的具体 body 版本 identity。
///
/// 该 key 显式区分：
/// - surface callable identity；
/// - `allowed_row` 家族；
/// - P4 已决定的 `impl_plan`；
/// - `needs_reentry`。
///
/// 后续 widening / specialization 只能在这套显式 key 空间里发生，不能再回到 span、名字或隐藏缓存。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LateLoweredBodyVersionKey {
    surface_instance: InstanceKey,
    allowed_row: EffectRow,
    impl_plan: ImplPlan,
    needs_reentry: bool,
}

impl LateLoweredBodyVersionKey {
    pub(crate) fn new(
        surface_instance: InstanceKey,
        allowed_row: EffectRow,
        impl_plan: ImplPlan,
        needs_reentry: bool,
    ) -> Self {
        Self {
            surface_instance,
            allowed_row,
            impl_plan,
            needs_reentry,
        }
    }

    pub fn surface_instance(&self) -> &InstanceKey {
        &self.surface_instance
    }

    pub fn allowed_row(&self) -> &EffectRow {
        &self.allowed_row
    }

    pub fn impl_plan(&self) -> ImplPlan {
        self.impl_plan
    }

    pub fn needs_reentry(&self) -> bool {
        self.needs_reentry
    }
}

/// 单个 callable version 在 late lowering 入口处对应的最终目标骨架。
///
/// 其中 `step_schema` / boundary lowering / state graph 是 authoritative contract；
/// `resume_packings` 只是该 callable continuation object 对外附带发布的 effect-family packing
/// helper 列表，不能替代 per-op/per-schema 语义本体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredCallable {
    root_fqn: String,
    body_version_key: LateLoweredBodyVersionKey,
    step_schema: StepSchemaId,
    resolved_outward_cases: Vec<CaseTag>,
    dynamic_invoke_entry: LateLoweredDynamicInvokeEntry,
    state_graph: LateLoweredStateGraph,
    frame_schema: LateLoweredFrameSchema,
    boundary_map: LateLoweredBoundaryMap,
    resume_state_map: LateLoweredResumeStateMap,
    continuation_object: ContinuationObjectId,
    resume_packings: Vec<ResumeInterfaceId>,
}

impl LateLoweredCallable {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        root_fqn: String,
        body_version_key: LateLoweredBodyVersionKey,
        step_schema: StepSchemaId,
        resolved_outward_cases: Vec<CaseTag>,
        dynamic_invoke_entry: LateLoweredDynamicInvokeEntry,
        state_graph: LateLoweredStateGraph,
        frame_schema: LateLoweredFrameSchema,
        boundary_map: LateLoweredBoundaryMap,
        resume_state_map: LateLoweredResumeStateMap,
        continuation_object: ContinuationObjectId,
        resume_packings: Vec<ResumeInterfaceId>,
    ) -> Self {
        Self {
            root_fqn,
            body_version_key,
            step_schema,
            resolved_outward_cases,
            dynamic_invoke_entry,
            state_graph,
            frame_schema,
            boundary_map,
            resume_state_map,
            continuation_object,
            resume_packings,
        }
    }

    pub fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub fn body_version_key(&self) -> &LateLoweredBodyVersionKey {
        &self.body_version_key
    }

    pub fn instance_key(&self) -> &InstanceKey {
        self.body_version_key.surface_instance()
    }

    pub fn allowed_row(&self) -> &EffectRow {
        self.body_version_key.allowed_row()
    }

    pub fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub fn impl_plan(&self) -> ImplPlan {
        self.body_version_key.impl_plan()
    }

    pub fn needs_reentry(&self) -> bool {
        self.body_version_key.needs_reentry()
    }

    pub fn resolved_outward_cases(&self) -> &[CaseTag] {
        &self.resolved_outward_cases
    }

    pub fn dynamic_invoke_entry(&self) -> &LateLoweredDynamicInvokeEntry {
        &self.dynamic_invoke_entry
    }

    pub fn state_graph(&self) -> &LateLoweredStateGraph {
        &self.state_graph
    }

    pub fn frame_schema(&self) -> &LateLoweredFrameSchema {
        &self.frame_schema
    }

    pub fn boundary_map(&self) -> &LateLoweredBoundaryMap {
        &self.boundary_map
    }

    pub fn resume_state_map(&self) -> &LateLoweredResumeStateMap {
        &self.resume_state_map
    }

    pub fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }

    pub fn resume_packings(&self) -> &[ResumeInterfaceId] {
        &self.resume_packings
    }

    /// 兼容旧调用点；新的 handoff 应优先使用 `resume_packings()` 叙事。
    pub fn resume_interfaces(&self) -> &[ResumeInterfaceId] {
        self.resume_packings()
    }
}

/// canonical dynamic callable surface 的稳定表示：`invoke(args_tuple) -> Step_F`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredDynamicInvokeEntry {
    invoke_args_tuple_ty: TypeId,
    step_schema: StepSchemaId,
    entry_state: StateId,
    complete_state: StateId,
}

impl LateLoweredDynamicInvokeEntry {
    pub(crate) fn new(
        invoke_args_tuple_ty: TypeId,
        step_schema: StepSchemaId,
        entry_state: StateId,
        complete_state: StateId,
    ) -> Self {
        Self {
            invoke_args_tuple_ty,
            step_schema,
            entry_state,
            complete_state,
        }
    }

    pub fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub fn entry_state(&self) -> StateId {
        self.entry_state
    }

    pub fn complete_state(&self) -> StateId {
        self.complete_state
    }
}

/// `StepSchema(F)` 对应的内部 `Step_F` enum 物化壳层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredStepType {
    step_schema: StepSchemaId,
    invoke_args_tuple_ty: TypeId,
    complete_ty: TypeId,
    continuation_obj_ty: TypeId,
    cases: Vec<LateLoweredStepCase>,
}

impl LateLoweredStepType {
    pub(crate) fn new(
        step_schema: StepSchemaId,
        invoke_args_tuple_ty: TypeId,
        complete_ty: TypeId,
        continuation_obj_ty: TypeId,
        cases: Vec<LateLoweredStepCase>,
    ) -> Self {
        Self {
            step_schema,
            invoke_args_tuple_ty,
            complete_ty,
            continuation_obj_ty,
            cases,
        }
    }

    pub fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub fn complete_ty(&self) -> TypeId {
        self.complete_ty
    }

    pub fn continuation_obj_ty(&self) -> TypeId {
        self.continuation_obj_ty
    }

    pub fn cases(&self) -> &[LateLoweredStepCase] {
        &self.cases
    }

    pub fn case(&self, case_tag: CaseTag) -> Option<&LateLoweredStepCase> {
        self.cases.iter().find(|case| case.case_tag() == case_tag)
    }
}

/// continuation schema 在 late-lowered shell 中的双层 contract 快照。
///
/// - `resume_tuple_ty` / `answer_ty` / `out_step_schema` 是 internal `resume(...) -> Step_F`
///   lowering 的 authoritative 输入；
/// - `surface_ty` 只保留源码层 `Continuation<..., eff Out>` contract，不能被 internal
///   one-shot runtime-error upper bound 反向扩大。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredContinuationContract {
    continuation_schema: ContinuationSchemaId,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
    surface_ty: TypeId,
}

impl LateLoweredContinuationContract {
    pub(crate) fn new(
        continuation_schema: ContinuationSchemaId,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        out_step_schema: StepSchemaId,
        surface_ty: TypeId,
    ) -> Self {
        Self {
            continuation_schema,
            resume_tuple_ty,
            answer_ty,
            out_step_schema,
            surface_ty,
        }
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.resume_tuple_ty
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.out_step_schema
    }

    pub fn surface_ty(&self) -> TypeId {
        self.surface_ty
    }
}

/// shared surface `resume(...) -> Step_F` 符号的最小 published contract。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredSurfaceResumeContract {
    continuation_schema: ContinuationSchemaId,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
}

impl LateLoweredSurfaceResumeContract {
    pub(crate) fn new(
        continuation_schema: ContinuationSchemaId,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        out_step_schema: StepSchemaId,
    ) -> Self {
        Self {
            continuation_schema,
            resume_tuple_ty,
            answer_ty,
            out_step_schema,
        }
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.resume_tuple_ty
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.out_step_schema
    }
}

/// 一个 shared surface-resume schema 当前 authoritative 来自哪类 source。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredSurfaceResumeDispatchSourceKind {
    ContinuationObjectMethod,
    ResumeBoundaryOnly,
    HandleContinuationBinderOnly,
    OwnerTrampolineMixed,
    Unreachable,
}

/// 单个 `ContinuationSchemaId` 在 late-lowered handoff 中出现的位置记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LateLoweredSurfaceResumeDispatchPublication {
    SurfaceCase {
        object_id: ContinuationObjectId,
        case_tag: CaseTag,
        reachability: LateLoweredContinuationMethodReachability,
    },
    InternalMethod {
        object_id: ContinuationObjectId,
        packing_interface_id: ResumeInterfaceId,
        case_tag: CaseTag,
        reachability: LateLoweredContinuationMethodReachability,
    },
    ResumeBoundary {
        owner_version_key: LateLoweredBodyVersionKey,
        owner_continuation_object: ContinuationObjectId,
        site_id: SiteId,
    },
    HandleContinuationBinder {
        owner_version_key: LateLoweredBodyVersionKey,
        owner_continuation_object: ContinuationObjectId,
        site_id: SiteId,
        arm_ordinal: u32,
        handled_case: CaseTag,
    },
}

impl LateLoweredSurfaceResumeDispatchPublication {
    pub fn owner_version_key(&self) -> Option<&LateLoweredBodyVersionKey> {
        match self {
            Self::ResumeBoundary {
                owner_version_key, ..
            }
            | Self::HandleContinuationBinder {
                owner_version_key, ..
            } => Some(owner_version_key),
            Self::SurfaceCase { .. } | Self::InternalMethod { .. } => None,
        }
    }
}

/// 一个 continuation local 在 published handoff 中可回查到的 authoritative underlying route。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredContinuationRoute {
    continuation_schema: ContinuationSchemaId,
    publication: LateLoweredSurfaceResumeDispatchPublication,
}

impl LateLoweredContinuationRoute {
    pub(crate) fn new(
        continuation_schema: ContinuationSchemaId,
        publication: LateLoweredSurfaceResumeDispatchPublication,
    ) -> Self {
        Self {
            continuation_schema,
            publication,
        }
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub fn publication(&self) -> &LateLoweredSurfaceResumeDispatchPublication {
        &self.publication
    }
}

/// `ContinuationSchemaId` 到 authoritative dispatch source inventory 的 published entry。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredSurfaceResumeDispatchInventoryEntry {
    continuation_schema: ContinuationSchemaId,
    contract: LateLoweredSurfaceResumeContract,
    source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
    publications: Vec<LateLoweredSurfaceResumeDispatchPublication>,
}

impl LateLoweredSurfaceResumeDispatchInventoryEntry {
    pub(crate) fn new(
        continuation_schema: ContinuationSchemaId,
        contract: LateLoweredSurfaceResumeContract,
        source_kind: LateLoweredSurfaceResumeDispatchSourceKind,
        publications: Vec<LateLoweredSurfaceResumeDispatchPublication>,
    ) -> Self {
        Self {
            continuation_schema,
            contract,
            source_kind,
            publications,
        }
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub fn contract(&self) -> LateLoweredSurfaceResumeContract {
        self.contract
    }

    pub fn source_kind(&self) -> LateLoweredSurfaceResumeDispatchSourceKind {
        self.source_kind
    }

    pub fn publications(&self) -> &[LateLoweredSurfaceResumeDispatchPublication] {
        &self.publications
    }
}

#[derive(Default)]
struct SurfaceResumeDispatchInventoryAccumulator {
    contract: Option<LateLoweredSurfaceResumeContract>,
    publications: Vec<LateLoweredSurfaceResumeDispatchPublication>,
    has_object_source: bool,
    has_resume_boundary: bool,
    has_handle_binder: bool,
}

impl SurfaceResumeDispatchInventoryAccumulator {
    fn register(
        &mut self,
        contract: Option<LateLoweredSurfaceResumeContract>,
        publication: LateLoweredSurfaceResumeDispatchPublication,
    ) {
        if self.contract.is_none() {
            self.contract = contract;
        }
        match publication {
            LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { reachability, .. }
            | LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                reachability, ..
            } => {
                if reachability == LateLoweredContinuationMethodReachability::Reachable {
                    self.has_object_source = true;
                }
            }
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. } => {
                self.has_resume_boundary = true;
            }
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder { .. } => {
                self.has_handle_binder = true;
            }
        }
        self.publications.push(publication);
    }

    fn source_kind(&self) -> LateLoweredSurfaceResumeDispatchSourceKind {
        if self.has_object_source {
            LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
        } else if self.has_resume_boundary && self.has_handle_binder {
            LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
        } else if self.has_resume_boundary {
            LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
        } else if self.has_handle_binder {
            LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
        } else {
            LateLoweredSurfaceResumeDispatchSourceKind::Unreachable
        }
    }
}

fn build_surface_resume_dispatch_inventory(
    step_types: &[LateLoweredStepType],
    continuation_objects: &[LateLoweredContinuationObject],
    callables: &[LateLoweredCallable],
) -> Vec<LateLoweredSurfaceResumeDispatchInventoryEntry> {
    let step_types_by_schema = step_types
        .iter()
        .map(|step_type| (step_type.step_schema(), step_type))
        .collect::<BTreeMap<_, _>>();
    let mut inventory =
        BTreeMap::<ContinuationSchemaId, SurfaceResumeDispatchInventoryAccumulator>::new();

    for object in continuation_objects {
        for surface_resume in object.surface_resumes() {
            inventory
                .entry(surface_resume.continuation_schema())
                .or_default()
                .register(
                    Some(surface_resume_contract_from_continuation(
                        surface_resume.continuation_contract(),
                    )),
                    LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
                        object_id: object.object_id(),
                        case_tag: surface_resume.case_tag(),
                        reachability: surface_resume.reachability(),
                    },
                );
        }
        for method in object.methods() {
            inventory
                .entry(method.continuation_schema())
                .or_default()
                .register(
                    Some(surface_resume_contract_from_continuation(
                        method.continuation_contract(),
                    )),
                    LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                        object_id: object.object_id(),
                        packing_interface_id: method.packing_interface_id(),
                        case_tag: method.case_tag(),
                        reachability: method.reachability(),
                    },
                );
        }
    }

    for callable in callables {
        for boundary in callable.boundary_map().entries() {
            let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
                continue;
            };
            let LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } = boundary.source()
            else {
                continue;
            };
            let facts = lowering.facts();
            inventory
                .entry(facts.continuation_schema())
                .or_default()
                .register(
                    Some(surface_resume_contract_from_resume_facts(facts)),
                    LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                        owner_version_key: callable.body_version_key().clone(),
                        owner_continuation_object: callable.continuation_object(),
                        site_id,
                    },
                );
        }

        let owner_step = step_types_by_schema.get(&callable.step_schema()).copied();
        for state in callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = state.terminator()
            else {
                continue;
            };
            for arm in contract.handled_arms() {
                let Some(binder) = arm.continuation_binder() else {
                    continue;
                };
                let contract = owner_step.and_then(|step_type| {
                    step_type.case(arm.handled_case()).map(|case| {
                        surface_resume_contract_from_continuation(case.continuation_contract())
                    })
                });
                inventory
                    .entry(binder.continuation_schema())
                    .or_default()
                    .register(
                        contract,
                        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                            owner_version_key: callable.body_version_key().clone(),
                            owner_continuation_object: callable.continuation_object(),
                            site_id: *site_id,
                            arm_ordinal: arm.arm_ordinal(),
                            handled_case: arm.handled_case(),
                        },
                    );
            }
        }
    }

    inventory
        .into_iter()
        .filter_map(|(continuation_schema, entry)| {
            entry.contract.map(|contract| {
                LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                    continuation_schema,
                    contract,
                    entry.source_kind(),
                    entry.publications,
                )
            })
        })
        .collect()
}

fn surface_resume_contract_from_continuation(
    contract: LateLoweredContinuationContract,
) -> LateLoweredSurfaceResumeContract {
    LateLoweredSurfaceResumeContract::new(
        contract.continuation_schema(),
        contract.resume_tuple_ty(),
        contract.answer_ty(),
        contract.out_step_schema(),
    )
}

fn surface_resume_contract_from_resume_facts(
    facts: &ResumeSiteEffectFacts,
) -> LateLoweredSurfaceResumeContract {
    LateLoweredSurfaceResumeContract::new(
        facts.continuation_schema(),
        facts.resume_tuple_ty(),
        facts.answer_ty(),
        facts.out_step_schema(),
    )
}

/// `Step_F` 中某个 canonical outward case 的稳定记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredStepCase {
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    continuation_contract: LateLoweredContinuationContract,
}

impl LateLoweredStepCase {
    pub(crate) fn new(
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        payload_tuple_ty: TypeId,
        continuation_contract: LateLoweredContinuationContract,
    ) -> Self {
        Self {
            case_tag,
            concrete_op_key,
            payload_tuple_ty,
            continuation_contract,
        }
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_contract.continuation_schema()
    }

    pub fn continuation_contract(&self) -> LateLoweredContinuationContract {
        self.continuation_contract
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.continuation_contract.resume_tuple_ty()
    }

    pub fn answer_ty(&self) -> TypeId {
        self.continuation_contract.answer_ty()
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.continuation_contract.out_step_schema()
    }

    pub fn surface_ty(&self) -> TypeId {
        self.continuation_contract.surface_ty()
    }
}

/// internal resume interface 的稳定 identity。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResumeInterfaceId(u32);

impl ResumeInterfaceId {
    pub(crate) const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// compiler-owned effect-family resume packing shell。
///
/// 该层只负责把 authoritative per-op/per-schema resume contracts 做成 effect-family 分组的
/// object-side packing / query helper；语义主体仍然是 `LateLoweredStepCase`、
/// `LateLoweredContinuationSurfaceResume`、`LateLoweredContinuationMethod` 与
/// `LateLoweredSurfaceResumeDispatchInventoryEntry`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredResumeInterface {
    interface_id: ResumeInterfaceId,
    effect_family: EffectFamilyKey,
    return_step_schema: StepSchemaId,
    methods: Vec<LateLoweredResumeMethod>,
}

impl LateLoweredResumeInterface {
    pub(crate) fn new(
        interface_id: ResumeInterfaceId,
        effect_family: EffectFamilyKey,
        return_step_schema: StepSchemaId,
        methods: Vec<LateLoweredResumeMethod>,
    ) -> Self {
        Self {
            interface_id,
            effect_family,
            return_step_schema,
            methods,
        }
    }

    pub fn interface_id(&self) -> ResumeInterfaceId {
        self.interface_id
    }

    pub fn effect_family(&self) -> &EffectFamilyKey {
        &self.effect_family
    }

    pub fn return_step_schema(&self) -> StepSchemaId {
        self.return_step_schema
    }

    pub fn methods(&self) -> &[LateLoweredResumeMethod] {
        &self.methods
    }
}

/// single resume method shell。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredResumeMethod {
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    continuation_contract: LateLoweredContinuationContract,
}

impl LateLoweredResumeMethod {
    pub(crate) fn new(
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        continuation_contract: LateLoweredContinuationContract,
    ) -> Self {
        Self {
            case_tag,
            concrete_op_key,
            continuation_contract,
        }
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_contract.continuation_schema()
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.continuation_contract.resume_tuple_ty()
    }

    pub fn answer_ty(&self) -> TypeId {
        self.continuation_contract.answer_ty()
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.continuation_contract.out_step_schema()
    }

    pub fn surface_ty(&self) -> TypeId {
        self.continuation_contract.surface_ty()
    }

    pub fn continuation_contract(&self) -> LateLoweredContinuationContract {
        self.continuation_contract
    }
}

/// continuation object 的稳定 identity。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContinuationObjectId(u32);

impl ContinuationObjectId {
    pub(crate) const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// continuation object 定义壳层。
///
/// `surface_resumes` / `methods` 是 authoritative per-case publication；
/// `implemented_packings` 仅表示这些 publication 同时被哪些 effect-family packing helper 镜像。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredContinuationObject {
    object_id: ContinuationObjectId,
    owner_version_key: LateLoweredBodyVersionKey,
    continuation_obj_ty: TypeId,
    implemented_packings: Vec<ResumeInterfaceId>,
    captures: Vec<LateLoweredContinuationCapture>,
    surface_resumes: Vec<LateLoweredContinuationSurfaceResume>,
    methods: Vec<LateLoweredContinuationMethod>,
}

impl LateLoweredContinuationObject {
    pub(crate) fn new(
        object_id: ContinuationObjectId,
        owner_version_key: LateLoweredBodyVersionKey,
        continuation_obj_ty: TypeId,
        implemented_packings: Vec<ResumeInterfaceId>,
        captures: Vec<LateLoweredContinuationCapture>,
        surface_resumes: Vec<LateLoweredContinuationSurfaceResume>,
        methods: Vec<LateLoweredContinuationMethod>,
    ) -> Self {
        Self {
            object_id,
            owner_version_key,
            continuation_obj_ty,
            implemented_packings,
            captures,
            surface_resumes,
            methods,
        }
    }

    pub fn object_id(&self) -> ContinuationObjectId {
        self.object_id
    }

    pub fn owner_version_key(&self) -> &LateLoweredBodyVersionKey {
        &self.owner_version_key
    }

    pub fn continuation_obj_ty(&self) -> TypeId {
        self.continuation_obj_ty
    }

    pub fn implemented_packings(&self) -> &[ResumeInterfaceId] {
        &self.implemented_packings
    }

    /// 兼容旧调用点；新的 handoff 应优先使用 `implemented_packings()` 叙事。
    pub fn implemented_interfaces(&self) -> &[ResumeInterfaceId] {
        self.implemented_packings()
    }

    pub fn captures(&self) -> &[LateLoweredContinuationCapture] {
        &self.captures
    }

    pub fn surface_resumes(&self) -> &[LateLoweredContinuationSurfaceResume] {
        &self.surface_resumes
    }

    pub fn methods(&self) -> &[LateLoweredContinuationMethod] {
        &self.methods
    }
}

/// continuation 对 frame/context 的显式 capture ref。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredContinuationCapture {
    FrameSlot(FrameSlotId),
    State(StateId),
}

/// 单个 continuation method 的可达性壳层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredContinuationMethodReachability {
    Reachable,
    Unreachable,
}

/// continuation `resume(...)` / `k.op$ret(...)` 的显式 body kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredOneShotPolicy {
    OrdinaryRuntimeErrorOutward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredContinuationResumeBody {
    ResumeCapturedState {
        repeated_resume: LateLoweredOneShotPolicy,
    },
    Unreachable,
}

/// continuation source-visible `resume(...) -> Step_F` 合同壳层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredContinuationSurfaceResume {
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    continuation_contract: LateLoweredContinuationContract,
    body: LateLoweredContinuationResumeBody,
}

impl LateLoweredContinuationSurfaceResume {
    pub(crate) fn new(
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        continuation_contract: LateLoweredContinuationContract,
        body: LateLoweredContinuationResumeBody,
    ) -> Self {
        Self {
            case_tag,
            concrete_op_key,
            continuation_contract,
            body,
        }
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_contract.continuation_schema()
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.continuation_contract.resume_tuple_ty()
    }

    pub fn answer_ty(&self) -> TypeId {
        self.continuation_contract.answer_ty()
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.continuation_contract.out_step_schema()
    }

    pub fn surface_ty(&self) -> TypeId {
        self.continuation_contract.surface_ty()
    }

    pub fn continuation_contract(&self) -> LateLoweredContinuationContract {
        self.continuation_contract
    }

    pub fn body(&self) -> LateLoweredContinuationResumeBody {
        self.body
    }

    pub fn reachability(&self) -> LateLoweredContinuationMethodReachability {
        match self.body {
            LateLoweredContinuationResumeBody::ResumeCapturedState { .. } => {
                LateLoweredContinuationMethodReachability::Reachable
            }
            LateLoweredContinuationResumeBody::Unreachable => {
                LateLoweredContinuationMethodReachability::Unreachable
            }
        }
    }
}

/// continuation method shell。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredContinuationMethod {
    packing_interface_id: ResumeInterfaceId,
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    continuation_contract: LateLoweredContinuationContract,
    body: LateLoweredContinuationResumeBody,
}

impl LateLoweredContinuationMethod {
    pub(crate) fn new(
        packing_interface_id: ResumeInterfaceId,
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        continuation_contract: LateLoweredContinuationContract,
        body: LateLoweredContinuationResumeBody,
    ) -> Self {
        Self {
            packing_interface_id,
            case_tag,
            concrete_op_key,
            continuation_contract,
            body,
        }
    }

    pub fn packing_interface_id(&self) -> ResumeInterfaceId {
        self.packing_interface_id
    }

    /// 兼容旧调用点；新的 handoff 应优先使用 `packing_interface_id()` 叙事。
    pub fn interface_id(&self) -> ResumeInterfaceId {
        self.packing_interface_id()
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_contract.continuation_schema()
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.continuation_contract.resume_tuple_ty()
    }

    pub fn answer_ty(&self) -> TypeId {
        self.continuation_contract.answer_ty()
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.continuation_contract.out_step_schema()
    }

    pub fn surface_ty(&self) -> TypeId {
        self.continuation_contract.surface_ty()
    }

    pub fn continuation_contract(&self) -> LateLoweredContinuationContract {
        self.continuation_contract
    }

    pub fn body(&self) -> LateLoweredContinuationResumeBody {
        self.body
    }

    pub fn reachability(&self) -> LateLoweredContinuationMethodReachability {
        match self.body {
            LateLoweredContinuationResumeBody::ResumeCapturedState { .. } => {
                LateLoweredContinuationMethodReachability::Reachable
            }
            LateLoweredContinuationResumeBody::Unreachable => {
                LateLoweredContinuationMethodReachability::Unreachable
            }
        }
    }
}

/// callable version 内局部可用的 state id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateId(u32);

impl StateId {
    pub(crate) const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// callable version 内的稳定 boundary id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoundaryId(u32);

impl BoundaryId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// frame schema 内的稳定 slot id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameSlotId(u32);

impl FrameSlotId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// state graph 中单个 state 的稳定 role。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredStateRole {
    Entry,
    Segment,
    Resume,
    Complete,
    Cleanup,
    Drop,
}

/// 单个 late-lowered state 当前覆盖的 direct-style MIR 片段。
///
/// P5-T03 先把 segmentation skeleton 固定到“block + statement slice (+ 可选 terminator)”这一层，
/// 以便后续 frame lifting / boundary lowering 在不回 P3 MIR 猜测的前提下，继续沿用同一套切分结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredStateSlice {
    block_id: BasicBlockId,
    start_statement_index: u32,
    end_statement_index: u32,
    includes_terminator: bool,
}

impl LateLoweredStateSlice {
    pub(crate) fn new(
        block_id: BasicBlockId,
        start_statement_index: u32,
        end_statement_index: u32,
        includes_terminator: bool,
    ) -> Self {
        Self {
            block_id,
            start_statement_index,
            end_statement_index,
            includes_terminator,
        }
    }

    pub fn block_id(&self) -> BasicBlockId {
        self.block_id
    }

    pub fn start_statement_index(&self) -> u32 {
        self.start_statement_index
    }

    pub fn end_statement_index(&self) -> u32 {
        self.end_statement_index
    }

    pub fn includes_terminator(&self) -> bool {
        self.includes_terminator
    }
}

/// boundary operand 的最小值来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LateLoweredOperandValueSource {
    Local(LocalId),
    Const(ConstValue),
}

/// body emitter 可直接消费的已发布 operand/source contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredOperandSource {
    value: LateLoweredOperandValueSource,
    source_ty: TypeId,
    span: Option<Span>,
}

impl LateLoweredOperandSource {
    pub(crate) fn new_local(local: LocalId, source_ty: TypeId, span: Option<Span>) -> Self {
        Self {
            value: LateLoweredOperandValueSource::Local(local),
            source_ty,
            span,
        }
    }

    pub(crate) fn new_const(value: ConstValue, source_ty: TypeId, span: Option<Span>) -> Self {
        Self {
            value: LateLoweredOperandValueSource::Const(value),
            source_ty,
            span,
        }
    }

    pub fn value(&self) -> &LateLoweredOperandValueSource {
        &self.value
    }

    pub fn source_ty(&self) -> TypeId {
        self.source_ty
    }

    pub fn span(&self) -> Option<Span> {
        self.span
    }
}

/// boundary 在 owner state source-slice 中消费哪一个 anchor。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredBoundarySourceConsumption {
    Statement {
        source_slice: LateLoweredStateSlice,
        statement_index: u32,
        consumes_last_statement: bool,
    },
    Terminator {
        source_slice: LateLoweredStateSlice,
    },
}

impl LateLoweredBoundarySourceConsumption {
    pub(crate) fn statement(
        source_slice: LateLoweredStateSlice,
        statement_index: u32,
        consumes_last_statement: bool,
    ) -> Self {
        Self::Statement {
            source_slice,
            statement_index,
            consumes_last_statement,
        }
    }

    pub(crate) fn terminator(source_slice: LateLoweredStateSlice) -> Self {
        Self::Terminator { source_slice }
    }

    pub fn source_slice(&self) -> LateLoweredStateSlice {
        match self {
            Self::Statement { source_slice, .. } | Self::Terminator { source_slice } => {
                *source_slice
            }
        }
    }

    pub fn statement_index(&self) -> Option<u32> {
        match self {
            Self::Statement {
                statement_index, ..
            } => Some(*statement_index),
            Self::Terminator { .. } => None,
        }
    }

    pub fn statement_index_in_slice(&self) -> Option<u32> {
        match self {
            Self::Statement {
                source_slice,
                statement_index,
                ..
            } => Some(statement_index.saturating_sub(source_slice.start_statement_index())),
            Self::Terminator { .. } => None,
        }
    }

    pub fn consumes_last_statement(&self) -> Option<bool> {
        match self {
            Self::Statement {
                consumes_last_statement,
                ..
            } => Some(*consumes_last_statement),
            Self::Terminator { .. } => None,
        }
    }
}

/// 单个 state 结束时的显式控制流合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LateLoweredHandlePendingCompletion {
    ContinueToExit,
    ReturnFromFunction,
    PropagateOutward(CaseTag),
}

/// `HandleDispatch` 需要消费的 compiler-owned system carrier。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredHandleDispatchCarrierContract {
    state_tag_slot: SystemSlotKind,
    completion_tag_slot: SystemSlotKind,
    payload_carrier_slot: SystemSlotKind,
}

impl LateLoweredHandleDispatchCarrierContract {
    pub(crate) fn new(
        state_tag_slot: SystemSlotKind,
        completion_tag_slot: SystemSlotKind,
        payload_carrier_slot: SystemSlotKind,
    ) -> Self {
        Self {
            state_tag_slot,
            completion_tag_slot,
            payload_carrier_slot,
        }
    }

    pub fn state_tag_slot(&self) -> SystemSlotKind {
        self.state_tag_slot
    }

    pub fn completion_tag_slot(&self) -> SystemSlotKind {
        self.completion_tag_slot
    }

    pub fn payload_carrier_slot(&self) -> SystemSlotKind {
        self.payload_carrier_slot
    }
}

/// 单个 handled case 的 authoritative arm dispatch 映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredHandlePayloadBinder {
    ordinal: u32,
    local: LocalId,
    frame_slot: Option<FrameSlotId>,
}

impl LateLoweredHandlePayloadBinder {
    pub(crate) fn new(ordinal: u32, local: LocalId, frame_slot: Option<FrameSlotId>) -> Self {
        Self {
            ordinal,
            local,
            frame_slot,
        }
    }

    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn local(&self) -> LocalId {
        self.local
    }

    pub fn frame_slot(&self) -> Option<FrameSlotId> {
        self.frame_slot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredHandleContinuationBinder {
    local: LocalId,
    frame_slot: Option<FrameSlotId>,
    continuation_schema: ContinuationSchemaId,
    continuation_object: ContinuationObjectId,
}

impl LateLoweredHandleContinuationBinder {
    pub(crate) fn new(
        local: LocalId,
        frame_slot: Option<FrameSlotId>,
        continuation_schema: ContinuationSchemaId,
        continuation_object: ContinuationObjectId,
    ) -> Self {
        Self {
            local,
            frame_slot,
            continuation_schema,
            continuation_object,
        }
    }

    pub fn local(&self) -> LocalId {
        self.local
    }

    pub fn frame_slot(&self) -> Option<FrameSlotId> {
        self.frame_slot
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }
}

/// 单个 handled case 的 authoritative arm dispatch 映射。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredHandleArmDispatch {
    handled_case: CaseTag,
    arm_state: StateId,
    arm_ordinal: u32,
    payload_tuple_ty: TypeId,
    payload_binders: Vec<LateLoweredHandlePayloadBinder>,
    continuation_binder: Option<LateLoweredHandleContinuationBinder>,
    arm_outward_cases: Vec<CaseTag>,
}

impl LateLoweredHandleArmDispatch {
    pub(crate) fn new(
        handled_case: CaseTag,
        arm_state: StateId,
        arm_ordinal: u32,
        payload_tuple_ty: TypeId,
        payload_binders: Vec<LateLoweredHandlePayloadBinder>,
        continuation_binder: Option<LateLoweredHandleContinuationBinder>,
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

    pub fn handled_case(&self) -> CaseTag {
        self.handled_case
    }

    pub fn arm_state(&self) -> StateId {
        self.arm_state
    }

    pub fn arm_ordinal(&self) -> u32 {
        self.arm_ordinal
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn payload_binders(&self) -> &[LateLoweredHandlePayloadBinder] {
        &self.payload_binders
    }

    pub fn continuation_binder(&self) -> Option<LateLoweredHandleContinuationBinder> {
        self.continuation_binder
    }

    pub fn arm_outward_cases(&self) -> &[CaseTag] {
        &self.arm_outward_cases
    }
}

/// 当前 state 在某个 `HandleDispatch` 子图中的 authoritative region 归属。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredHandleStateRegion {
    OutsideHandle,
    Dispatch,
    Body,
    Arm {
        handled_case: CaseTag,
        arm_ordinal: u32,
    },
    Finally,
    Exit,
}

/// `HandleDispatch` 对单个 state 发布的 region membership。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredHandleStateRegionEntry {
    state_id: StateId,
    region: LateLoweredHandleStateRegion,
}

impl LateLoweredHandleStateRegionEntry {
    pub(crate) fn new(state_id: StateId, region: LateLoweredHandleStateRegion) -> Self {
        Self { state_id, region }
    }

    pub fn state_id(&self) -> StateId {
        self.state_id
    }

    pub fn region(&self) -> LateLoweredHandleStateRegion {
        self.region
    }
}

/// 单个 boundary outward case 在当前 handle 下的 authoritative routing。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredHandleBoundaryCaseRoutingAction {
    ConsumeToArm {
        arm_state: StateId,
        arm_ordinal: u32,
        continuation_resume_state: StateId,
    },
    PendingCompletion {
        completion: LateLoweredHandlePendingCompletion,
    },
    EmitOutward,
}

/// `BoundaryId + CaseTag` 的 published handle-routing 结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateLoweredHandleBoundaryCaseRouting {
    case_tag: CaseTag,
    action: LateLoweredHandleBoundaryCaseRoutingAction,
}

impl LateLoweredHandleBoundaryCaseRouting {
    pub(crate) fn new(
        case_tag: CaseTag,
        action: LateLoweredHandleBoundaryCaseRoutingAction,
    ) -> Self {
        Self { case_tag, action }
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn action(&self) -> LateLoweredHandleBoundaryCaseRoutingAction {
        self.action
    }
}

/// 单个 boundary 在当前 `HandleDispatch` 子图中的 region / case-routing published contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredHandleBoundaryRouting {
    boundary_id: BoundaryId,
    owner_state: StateId,
    owner_region: LateLoweredHandleStateRegion,
    resume_state: StateId,
    case_routings: Vec<LateLoweredHandleBoundaryCaseRouting>,
}

impl LateLoweredHandleBoundaryRouting {
    pub(crate) fn new(
        boundary_id: BoundaryId,
        owner_state: StateId,
        owner_region: LateLoweredHandleStateRegion,
        resume_state: StateId,
        case_routings: Vec<LateLoweredHandleBoundaryCaseRouting>,
    ) -> Self {
        Self {
            boundary_id,
            owner_state,
            owner_region,
            resume_state,
            case_routings,
        }
    }

    pub fn boundary_id(&self) -> BoundaryId {
        self.boundary_id
    }

    pub fn owner_state(&self) -> StateId {
        self.owner_state
    }

    pub fn owner_region(&self) -> LateLoweredHandleStateRegion {
        self.owner_region
    }

    pub fn resume_state(&self) -> StateId {
        self.resume_state
    }

    pub fn case_routings(&self) -> &[LateLoweredHandleBoundaryCaseRouting] {
        &self.case_routings
    }

    pub fn case_routing(&self, case_tag: CaseTag) -> Option<&LateLoweredHandleBoundaryCaseRouting> {
        self.case_routings
            .iter()
            .find(|route| route.case_tag() == case_tag)
    }
}

/// `HandleDispatch` 在 P5/P6 handoff 中显式发布的 completion/state contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredHandleDispatchContract {
    carrier: LateLoweredHandleDispatchCarrierContract,
    body_complete_target: StateId,
    arm_complete_target: StateId,
    finally_complete_target: Option<StateId>,
    handled_arms: Vec<LateLoweredHandleArmDispatch>,
    body_outward_cases: Vec<CaseTag>,
    finally_outward_cases: Vec<CaseTag>,
    outward_emissions: Vec<LateLoweredStepCaseEmission>,
    pending_completions: Vec<LateLoweredHandlePendingCompletion>,
    state_regions: Vec<LateLoweredHandleStateRegionEntry>,
    boundary_routings: Vec<LateLoweredHandleBoundaryRouting>,
    abandon_target: Option<StateId>,
}

impl LateLoweredHandleDispatchContract {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        carrier: LateLoweredHandleDispatchCarrierContract,
        body_complete_target: StateId,
        arm_complete_target: StateId,
        finally_complete_target: Option<StateId>,
        handled_arms: Vec<LateLoweredHandleArmDispatch>,
        body_outward_cases: Vec<CaseTag>,
        finally_outward_cases: Vec<CaseTag>,
        outward_emissions: Vec<LateLoweredStepCaseEmission>,
        pending_completions: Vec<LateLoweredHandlePendingCompletion>,
        state_regions: Vec<LateLoweredHandleStateRegionEntry>,
        boundary_routings: Vec<LateLoweredHandleBoundaryRouting>,
        abandon_target: Option<StateId>,
    ) -> Self {
        Self {
            carrier,
            body_complete_target,
            arm_complete_target,
            finally_complete_target,
            handled_arms,
            body_outward_cases,
            finally_outward_cases,
            outward_emissions,
            pending_completions,
            state_regions,
            boundary_routings,
            abandon_target,
        }
    }

    pub(crate) fn skeleton(
        body_complete_target: StateId,
        arm_complete_target: StateId,
        finally_complete_target: Option<StateId>,
        abandon_target: Option<StateId>,
    ) -> Self {
        Self::new(
            LateLoweredHandleDispatchCarrierContract::new(
                SystemSlotKind::StateTag,
                SystemSlotKind::CompletionTag,
                SystemSlotKind::ResumePayloadCarrier,
            ),
            body_complete_target,
            arm_complete_target,
            finally_complete_target,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            abandon_target,
        )
    }

    pub(crate) fn with_abandon_target(mut self, abandon_target: Option<StateId>) -> Self {
        self.abandon_target = abandon_target;
        self
    }

    pub fn carrier(&self) -> LateLoweredHandleDispatchCarrierContract {
        self.carrier
    }

    pub fn body_complete_target(&self) -> StateId {
        self.body_complete_target
    }

    pub fn arm_complete_target(&self) -> StateId {
        self.arm_complete_target
    }

    pub fn finally_complete_target(&self) -> Option<StateId> {
        self.finally_complete_target
    }

    pub fn handled_arms(&self) -> &[LateLoweredHandleArmDispatch] {
        &self.handled_arms
    }

    pub fn handled_arm(&self, handled_case: CaseTag) -> Option<&LateLoweredHandleArmDispatch> {
        self.handled_arms
            .iter()
            .find(|arm| arm.handled_case() == handled_case)
    }

    pub fn body_outward_cases(&self) -> &[CaseTag] {
        &self.body_outward_cases
    }

    pub fn finally_outward_cases(&self) -> &[CaseTag] {
        &self.finally_outward_cases
    }

    pub fn outward_emissions(&self) -> &[LateLoweredStepCaseEmission] {
        &self.outward_emissions
    }

    pub fn outward_emission(&self, case_tag: CaseTag) -> Option<&LateLoweredStepCaseEmission> {
        self.outward_emissions
            .iter()
            .find(|emission| emission.case_tag() == case_tag)
    }

    pub fn pending_completions(&self) -> &[LateLoweredHandlePendingCompletion] {
        &self.pending_completions
    }

    pub fn state_regions(&self) -> &[LateLoweredHandleStateRegionEntry] {
        &self.state_regions
    }

    pub fn state_region(&self, state_id: StateId) -> LateLoweredHandleStateRegion {
        self.state_regions
            .iter()
            .find(|entry| entry.state_id() == state_id)
            .map(LateLoweredHandleStateRegionEntry::region)
            .unwrap_or(LateLoweredHandleStateRegion::OutsideHandle)
    }

    pub fn boundary_routings(&self) -> &[LateLoweredHandleBoundaryRouting] {
        &self.boundary_routings
    }

    pub fn boundary_routing(
        &self,
        boundary_id: BoundaryId,
    ) -> Option<&LateLoweredHandleBoundaryRouting> {
        self.boundary_routings
            .iter()
            .find(|route| route.boundary_id() == boundary_id)
    }

    pub fn needs_completion_state(&self) -> bool {
        !self.pending_completions.is_empty()
    }

    pub fn abandon_target(&self) -> Option<StateId> {
        self.abandon_target
    }
}

// `HandleDispatch` 承载的是单个 handle site 的完整 published contract；
// 这里保持按值内联，避免为了枚举大小把阶段 handoff 再拆成额外 Box 层。
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LateLoweredStateTerminator {
    Suspend {
        boundary_ids: Vec<BoundaryId>,
        resume_state: StateId,
        local_runtime_error_states: Vec<StateId>,
        cleanup_state: Option<StateId>,
        drop_state: Option<StateId>,
    },
    Goto {
        target: StateId,
    },
    Branch {
        cond_local: LocalId,
        then_state: StateId,
        else_state: StateId,
    },
    Return {
        value_local: Option<LocalId>,
        complete_state: StateId,
    },
    HandleDispatch {
        site_id: SiteId,
        body_state: StateId,
        arm_states: Vec<StateId>,
        finally_state: Option<StateId>,
        exit_state: StateId,
        contract: LateLoweredHandleDispatchContract,
        boundary_ids: Vec<BoundaryId>,
        drop_state: Option<StateId>,
    },
    LocalRuntimeError {
        payload_tuple_ty: TypeId,
        terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
    },
    ResumeUnwind,
    Unreachable,
    Abandon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredPublishedRuntimeEntry {
    RuntimeErrorFatal,
}

impl LateLoweredPublishedRuntimeEntry {
    pub fn symbol_name(&self) -> &'static str {
        match self {
            Self::RuntimeErrorFatal => "scoop_runtime_error_fatal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredLocalRuntimeErrorTerminalAction {
    RuntimeFatal {
        runtime_entry: LateLoweredPublishedRuntimeEntry,
    },
}

impl LateLoweredLocalRuntimeErrorTerminalAction {
    pub fn runtime_entry(&self) -> LateLoweredPublishedRuntimeEntry {
        match self {
            Self::RuntimeFatal { runtime_entry } => *runtime_entry,
        }
    }
}

impl LateLoweredStateTerminator {
    pub fn successors(&self) -> Vec<StateId> {
        match self {
            Self::Suspend {
                resume_state,
                local_runtime_error_states,
                cleanup_state,
                ..
            } => {
                let mut successors = vec![*resume_state];
                successors.extend(local_runtime_error_states.iter().copied());
                if let Some(cleanup_state) = cleanup_state {
                    successors.push(*cleanup_state);
                }
                successors
            }
            Self::Goto { target } => vec![*target],
            Self::Branch {
                then_state,
                else_state,
                ..
            } => vec![*then_state, *else_state],
            Self::Return { complete_state, .. } => vec![*complete_state],
            Self::HandleDispatch {
                body_state,
                arm_states,
                finally_state,
                ..
            } => {
                let mut successors = vec![*body_state];
                successors.extend(arm_states.iter().copied());
                if let Some(finally_state) = finally_state {
                    successors.push(*finally_state);
                }
                successors
            }
            Self::LocalRuntimeError { .. }
            | Self::ResumeUnwind
            | Self::Unreachable
            | Self::Abandon => Vec::new(),
        }
    }

    pub(crate) fn with_drop_state(self, drop_state: Option<StateId>) -> Self {
        match self {
            Self::Suspend {
                boundary_ids,
                resume_state,
                local_runtime_error_states,
                cleanup_state,
                ..
            } => Self::Suspend {
                boundary_ids,
                resume_state,
                local_runtime_error_states,
                cleanup_state,
                drop_state,
            },
            Self::HandleDispatch {
                site_id,
                body_state,
                arm_states,
                finally_state,
                exit_state,
                contract,
                boundary_ids,
                ..
            } => Self::HandleDispatch {
                site_id,
                body_state,
                arm_states,
                finally_state,
                exit_state,
                contract: contract.with_abandon_target(drop_state),
                boundary_ids,
                drop_state,
            },
            other => other,
        }
    }
}

/// state graph 中的单个 state shell。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredState {
    state_id: StateId,
    role: LateLoweredStateRole,
    source_slices: Vec<LateLoweredStateSlice>,
    terminator: LateLoweredStateTerminator,
    successors: Vec<StateId>,
}

impl LateLoweredState {
    pub(crate) fn new(
        state_id: StateId,
        role: LateLoweredStateRole,
        source_slices: Vec<LateLoweredStateSlice>,
        terminator: LateLoweredStateTerminator,
    ) -> Self {
        let successors = terminator.successors();
        Self {
            state_id,
            role,
            source_slices,
            terminator,
            successors,
        }
    }

    pub fn state_id(&self) -> StateId {
        self.state_id
    }

    pub fn role(&self) -> LateLoweredStateRole {
        self.role
    }

    pub fn source_slices(&self) -> &[LateLoweredStateSlice] {
        &self.source_slices
    }

    pub fn terminator(&self) -> &LateLoweredStateTerminator {
        &self.terminator
    }

    pub fn successors(&self) -> &[StateId] {
        &self.successors
    }

    pub(crate) fn with_drop_state(self, drop_state: Option<StateId>) -> Self {
        Self::new(
            self.state_id,
            self.role,
            self.source_slices,
            self.terminator.with_drop_state(drop_state),
        )
    }
}

/// late-lowered callable 的 state graph 壳层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredStateGraph {
    entry_state: StateId,
    complete_state: StateId,
    cleanup_state: Option<StateId>,
    drop_state: Option<StateId>,
    states: Vec<LateLoweredState>,
}

impl LateLoweredStateGraph {
    pub(crate) fn new(
        entry_state: StateId,
        complete_state: StateId,
        cleanup_state: Option<StateId>,
        drop_state: Option<StateId>,
        states: Vec<LateLoweredState>,
    ) -> Self {
        Self {
            entry_state,
            complete_state,
            cleanup_state,
            drop_state,
            states,
        }
    }

    pub(crate) fn minimal_shell() -> Self {
        let entry_state = StateId::new(0);
        let complete_state = StateId::new(1);
        Self::new(
            entry_state,
            complete_state,
            None,
            None,
            vec![
                LateLoweredState::new(
                    entry_state,
                    LateLoweredStateRole::Entry,
                    Vec::new(),
                    LateLoweredStateTerminator::Goto {
                        target: complete_state,
                    },
                ),
                LateLoweredState::new(
                    complete_state,
                    LateLoweredStateRole::Complete,
                    Vec::new(),
                    LateLoweredStateTerminator::Unreachable,
                ),
            ],
        )
    }

    pub fn entry_state(&self) -> StateId {
        self.entry_state
    }

    pub fn complete_state(&self) -> StateId {
        self.complete_state
    }

    pub fn cleanup_state(&self) -> Option<StateId> {
        self.cleanup_state
    }

    pub fn drop_state(&self) -> Option<StateId> {
        self.drop_state
    }

    pub fn states(&self) -> &[LateLoweredState] {
        &self.states
    }

    pub fn state(&self, state_id: StateId) -> Option<&LateLoweredState> {
        self.states
            .iter()
            .find(|state| state.state_id() == state_id)
    }

    pub(crate) fn with_drop_state(self, drop_state: Option<StateId>) -> Self {
        let states = self
            .states
            .into_iter()
            .map(|state| state.with_drop_state(drop_state))
            .collect();
        Self::new(
            self.entry_state,
            self.complete_state,
            self.cleanup_state,
            drop_state,
            states,
        )
    }
}

/// boundary 对应的 source category。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundarySiteKind {
    Call,
    Perform,
    Resume,
    Handle,
}

/// boundary 必须能稳定映射回 `SiteId` 或 boundary kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateLoweredBoundarySource {
    Site {
        site_id: SiteId,
        kind: BoundarySiteKind,
    },
    RuntimeError {
        origin_site: SiteId,
    },
}

/// 构造 outward `Step_F` case 的显式 P5 contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredStepCaseEmission {
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    continuation_contract: LateLoweredContinuationContract,
    continuation_object: ContinuationObjectId,
}

impl LateLoweredStepCaseEmission {
    pub(crate) fn new(
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        payload_tuple_ty: TypeId,
        continuation_contract: LateLoweredContinuationContract,
        continuation_object: ContinuationObjectId,
    ) -> Self {
        Self {
            case_tag,
            concrete_op_key,
            payload_tuple_ty,
            continuation_contract,
            continuation_object,
        }
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn continuation_contract(&self) -> LateLoweredContinuationContract {
        self.continuation_contract
    }

    pub fn continuation_object(&self) -> ContinuationObjectId {
        self.continuation_object
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredCompleteStepDispatch {
    answer_ty: TypeId,
    target_state: StateId,
    result_local: Option<LocalId>,
}

impl LateLoweredCompleteStepDispatch {
    pub(crate) fn new(
        answer_ty: TypeId,
        target_state: StateId,
        result_local: Option<LocalId>,
    ) -> Self {
        Self {
            answer_ty,
            target_state,
            result_local,
        }
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn target_state(&self) -> StateId {
        self.target_state
    }

    pub fn result_local(&self) -> Option<LocalId> {
        self.result_local
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredStepCaseForwarding {
    input_case_tag: CaseTag,
    input_concrete_op_key: ConcreteOpKey,
    emission: LateLoweredStepCaseEmission,
}

impl LateLoweredStepCaseForwarding {
    pub(crate) fn new(
        input_case_tag: CaseTag,
        input_concrete_op_key: ConcreteOpKey,
        emission: LateLoweredStepCaseEmission,
    ) -> Self {
        Self {
            input_case_tag,
            input_concrete_op_key,
            emission,
        }
    }

    pub fn input_case_tag(&self) -> CaseTag {
        self.input_case_tag
    }

    pub fn input_concrete_op_key(&self) -> &ConcreteOpKey {
        &self.input_concrete_op_key
    }

    pub fn emission(&self) -> &LateLoweredStepCaseEmission {
        &self.emission
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredStepDispatchPlan {
    input_step_schema: StepSchemaId,
    complete: LateLoweredCompleteStepDispatch,
    outward_cases: Vec<LateLoweredStepCaseForwarding>,
}

impl LateLoweredStepDispatchPlan {
    pub(crate) fn new(
        input_step_schema: StepSchemaId,
        complete: LateLoweredCompleteStepDispatch,
        outward_cases: Vec<LateLoweredStepCaseForwarding>,
    ) -> Self {
        Self {
            input_step_schema,
            complete,
            outward_cases,
        }
    }

    pub fn input_step_schema(&self) -> StepSchemaId {
        self.input_step_schema
    }

    pub fn complete(&self) -> &LateLoweredCompleteStepDispatch {
        &self.complete
    }

    pub fn outward_cases(&self) -> &[LateLoweredStepCaseForwarding] {
        &self.outward_cases
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredConsumedRuntimeErrorCase {
    input_case_tag: CaseTag,
    input_concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
    target_state: StateId,
}

impl LateLoweredConsumedRuntimeErrorCase {
    pub(crate) fn new(
        input_case_tag: CaseTag,
        input_concrete_op_key: ConcreteOpKey,
        payload_tuple_ty: TypeId,
        terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
        target_state: StateId,
    ) -> Self {
        Self {
            input_case_tag,
            input_concrete_op_key,
            payload_tuple_ty,
            terminal_action,
            target_state,
        }
    }

    pub fn input_case_tag(&self) -> CaseTag {
        self.input_case_tag
    }

    pub fn input_concrete_op_key(&self) -> &ConcreteOpKey {
        &self.input_concrete_op_key
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn terminal_action(&self) -> LateLoweredLocalRuntimeErrorTerminalAction {
        self.terminal_action
    }

    pub fn target_state(&self) -> StateId {
        self.target_state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredCallBoundaryOperandContract {
    source_consumption: LateLoweredBoundarySourceConsumption,
    carrier_source: Option<LateLoweredOperandSource>,
    arg_sources: Vec<LateLoweredOperandSource>,
}

impl LateLoweredCallBoundaryOperandContract {
    pub(crate) fn new(
        source_consumption: LateLoweredBoundarySourceConsumption,
        carrier_source: Option<LateLoweredOperandSource>,
        arg_sources: Vec<LateLoweredOperandSource>,
    ) -> Self {
        Self {
            source_consumption,
            carrier_source,
            arg_sources,
        }
    }

    pub fn source_consumption(&self) -> LateLoweredBoundarySourceConsumption {
        self.source_consumption
    }

    pub fn carrier_source(&self) -> Option<&LateLoweredOperandSource> {
        self.carrier_source.as_ref()
    }

    pub fn arg_sources(&self) -> &[LateLoweredOperandSource] {
        &self.arg_sources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredPerformBoundaryOperandContract {
    source_consumption: LateLoweredBoundarySourceConsumption,
    payload_sources: Vec<LateLoweredOperandSource>,
}

impl LateLoweredPerformBoundaryOperandContract {
    pub(crate) fn new(
        source_consumption: LateLoweredBoundarySourceConsumption,
        payload_sources: Vec<LateLoweredOperandSource>,
    ) -> Self {
        Self {
            source_consumption,
            payload_sources,
        }
    }

    pub fn source_consumption(&self) -> LateLoweredBoundarySourceConsumption {
        self.source_consumption
    }

    pub fn payload_sources(&self) -> &[LateLoweredOperandSource] {
        &self.payload_sources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredResumeBoundaryOperandContract {
    source_consumption: LateLoweredBoundarySourceConsumption,
    continuation_source: LateLoweredOperandSource,
    arg_sources: Vec<LateLoweredOperandSource>,
    underlying_continuation_route: Option<LateLoweredContinuationRoute>,
}

impl LateLoweredResumeBoundaryOperandContract {
    pub(crate) fn new(
        source_consumption: LateLoweredBoundarySourceConsumption,
        continuation_source: LateLoweredOperandSource,
        arg_sources: Vec<LateLoweredOperandSource>,
        underlying_continuation_route: Option<LateLoweredContinuationRoute>,
    ) -> Self {
        Self {
            source_consumption,
            continuation_source,
            arg_sources,
            underlying_continuation_route,
        }
    }

    pub fn source_consumption(&self) -> LateLoweredBoundarySourceConsumption {
        self.source_consumption
    }

    pub fn continuation_source(&self) -> &LateLoweredOperandSource {
        &self.continuation_source
    }

    pub fn arg_sources(&self) -> &[LateLoweredOperandSource] {
        &self.arg_sources
    }

    pub fn underlying_continuation_route(&self) -> Option<&LateLoweredContinuationRoute> {
        self.underlying_continuation_route.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredCallBoundaryLowering {
    facts: CallSiteEffectFacts,
    result_local: LocalId,
    operand_contract: Box<LateLoweredCallBoundaryOperandContract>,
    dispatch: LateLoweredStepDispatchPlan,
    consumed_runtime_error_case: Option<LateLoweredConsumedRuntimeErrorCase>,
}

impl LateLoweredCallBoundaryLowering {
    pub(crate) fn new(
        facts: CallSiteEffectFacts,
        result_local: LocalId,
        operand_contract: LateLoweredCallBoundaryOperandContract,
        dispatch: LateLoweredStepDispatchPlan,
        consumed_runtime_error_case: Option<LateLoweredConsumedRuntimeErrorCase>,
    ) -> Self {
        Self {
            facts,
            result_local,
            operand_contract: Box::new(operand_contract),
            dispatch,
            consumed_runtime_error_case,
        }
    }

    pub fn facts(&self) -> &CallSiteEffectFacts {
        &self.facts
    }

    pub fn result_local(&self) -> LocalId {
        self.result_local
    }

    pub fn operand_contract(&self) -> &LateLoweredCallBoundaryOperandContract {
        &self.operand_contract
    }

    pub fn dispatch(&self) -> &LateLoweredStepDispatchPlan {
        &self.dispatch
    }

    pub fn consumed_runtime_error_case(&self) -> Option<&LateLoweredConsumedRuntimeErrorCase> {
        self.consumed_runtime_error_case.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredPerformBoundaryLowering {
    facts: PerformSiteEffectFacts,
    operand_contract: Box<LateLoweredPerformBoundaryOperandContract>,
    emitted_step: LateLoweredStepCaseEmission,
}

impl LateLoweredPerformBoundaryLowering {
    pub(crate) fn new(
        facts: PerformSiteEffectFacts,
        operand_contract: LateLoweredPerformBoundaryOperandContract,
        emitted_step: LateLoweredStepCaseEmission,
    ) -> Self {
        Self {
            facts,
            operand_contract: Box::new(operand_contract),
            emitted_step,
        }
    }

    pub fn facts(&self) -> &PerformSiteEffectFacts {
        &self.facts
    }

    pub fn operand_contract(&self) -> &LateLoweredPerformBoundaryOperandContract {
        &self.operand_contract
    }

    pub fn emitted_step(&self) -> &LateLoweredStepCaseEmission {
        &self.emitted_step
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredResumeBoundaryLowering {
    facts: ResumeSiteEffectFacts,
    result_local: LocalId,
    runtime_error_boundary: BoundaryId,
    operand_contract: Box<LateLoweredResumeBoundaryOperandContract>,
    dispatch: LateLoweredStepDispatchPlan,
}

impl LateLoweredResumeBoundaryLowering {
    pub(crate) fn new(
        facts: ResumeSiteEffectFacts,
        result_local: LocalId,
        runtime_error_boundary: BoundaryId,
        operand_contract: LateLoweredResumeBoundaryOperandContract,
        dispatch: LateLoweredStepDispatchPlan,
    ) -> Self {
        Self {
            facts,
            result_local,
            runtime_error_boundary,
            operand_contract: Box::new(operand_contract),
            dispatch,
        }
    }

    pub fn facts(&self) -> &ResumeSiteEffectFacts {
        &self.facts
    }

    pub fn result_local(&self) -> LocalId {
        self.result_local
    }

    pub fn runtime_error_boundary(&self) -> BoundaryId {
        self.runtime_error_boundary
    }

    pub fn operand_contract(&self) -> &LateLoweredResumeBoundaryOperandContract {
        &self.operand_contract
    }

    pub fn dispatch(&self) -> &LateLoweredStepDispatchPlan {
        &self.dispatch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredRuntimeErrorBoundaryLowering {
    origin_site: SiteId,
    resume_boundary: BoundaryId,
    emitted_step: LateLoweredStepCaseEmission,
}

impl LateLoweredRuntimeErrorBoundaryLowering {
    pub(crate) fn new(
        origin_site: SiteId,
        resume_boundary: BoundaryId,
        emitted_step: LateLoweredStepCaseEmission,
    ) -> Self {
        Self {
            origin_site,
            resume_boundary,
            emitted_step,
        }
    }

    pub fn origin_site(&self) -> SiteId {
        self.origin_site
    }

    pub fn resume_boundary(&self) -> BoundaryId {
        self.resume_boundary
    }

    pub fn emitted_step(&self) -> &LateLoweredStepCaseEmission {
        &self.emitted_step
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredHandleBoundaryLowering {
    facts: HandleSiteEffectFacts,
    outward_emissions: Vec<LateLoweredStepCaseEmission>,
}

impl LateLoweredHandleBoundaryLowering {
    pub(crate) fn new(
        facts: HandleSiteEffectFacts,
        outward_emissions: Vec<LateLoweredStepCaseEmission>,
    ) -> Self {
        Self {
            facts,
            outward_emissions,
        }
    }

    pub fn facts(&self) -> &HandleSiteEffectFacts {
        &self.facts
    }

    pub fn outward_emissions(&self) -> &[LateLoweredStepCaseEmission] {
        &self.outward_emissions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LateLoweredBoundaryLowering {
    Call(LateLoweredCallBoundaryLowering),
    Perform(LateLoweredPerformBoundaryLowering),
    Resume(LateLoweredResumeBoundaryLowering),
    RuntimeError(LateLoweredRuntimeErrorBoundaryLowering),
    Handle(LateLoweredHandleBoundaryLowering),
}

/// boundary shell。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredBoundary {
    boundary_id: BoundaryId,
    source: LateLoweredBoundarySource,
    owner_state: StateId,
    resume_state: StateId,
    lowering: Option<LateLoweredBoundaryLowering>,
}

impl LateLoweredBoundary {
    pub fn new(
        boundary_id: BoundaryId,
        source: LateLoweredBoundarySource,
        owner_state: StateId,
        resume_state: StateId,
    ) -> Self {
        Self {
            boundary_id,
            source,
            owner_state,
            resume_state,
            lowering: None,
        }
    }

    pub(crate) fn with_lowering(mut self, lowering: LateLoweredBoundaryLowering) -> Self {
        self.lowering = Some(lowering);
        self
    }

    pub fn boundary_id(&self) -> BoundaryId {
        self.boundary_id
    }

    pub fn source(&self) -> LateLoweredBoundarySource {
        self.source
    }

    pub fn owner_state(&self) -> StateId {
        self.owner_state
    }

    pub fn resume_state(&self) -> StateId {
        self.resume_state
    }

    pub fn lowering(&self) -> Option<&LateLoweredBoundaryLowering> {
        self.lowering.as_ref()
    }
}

/// `BoundaryId -> boundary shell` 的稳定容器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredBoundaryMap {
    entries: Vec<LateLoweredBoundary>,
}

impl LateLoweredBoundaryMap {
    pub fn new(entries: Vec<LateLoweredBoundary>) -> Self {
        Self { entries }
    }

    pub(crate) fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn entries(&self) -> &[LateLoweredBoundary] {
        &self.entries
    }

    pub fn boundary(&self, boundary_id: BoundaryId) -> Option<&LateLoweredBoundary> {
        self.entries
            .iter()
            .find(|boundary| boundary.boundary_id() == boundary_id)
    }
}

/// `BoundaryId -> resume state` 的稳定绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredResumeState {
    boundary_id: BoundaryId,
    state_id: StateId,
}

impl LateLoweredResumeState {
    pub fn new(boundary_id: BoundaryId, state_id: StateId) -> Self {
        Self {
            boundary_id,
            state_id,
        }
    }

    pub fn boundary_id(&self) -> BoundaryId {
        self.boundary_id
    }

    pub fn state_id(&self) -> StateId {
        self.state_id
    }
}

/// `BoundaryId -> resume state` 的稳定容器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredResumeStateMap {
    entries: Vec<LateLoweredResumeState>,
}

impl LateLoweredResumeStateMap {
    pub fn new(entries: Vec<LateLoweredResumeState>) -> Self {
        Self { entries }
    }

    pub(crate) fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn entries(&self) -> &[LateLoweredResumeState] {
        &self.entries
    }

    pub fn state_for(&self, boundary_id: BoundaryId) -> Option<StateId> {
        self.entries
            .iter()
            .find(|entry| entry.boundary_id() == boundary_id)
            .map(LateLoweredResumeState::state_id)
    }
}

/// 系统保留 frame 字段的分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemSlotKind {
    StateTag,
    ResumePayloadCarrier,
    CleanupFlag,
    OneShotFlag,
    CompletionTag,
}

/// frame slot 的稳定分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LateLoweredFrameSlotKind {
    SourceLocal(LocalId),
    CompilerTemporary(LocalId),
    JoinValue {
        local: LocalId,
        block: BasicBlockId,
        ordinal: u32,
    },
    HandleBinder {
        site_id: SiteId,
        local: LocalId,
        ordinal: u32,
    },
    ResumePayload {
        boundary: BoundaryId,
        case_tag: CaseTag,
    },
    BoundaryResult {
        boundary: BoundaryId,
        local: LocalId,
    },
    System(SystemSlotKind),
}

/// frame schema 内的单个 slot。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredFrameSlot {
    slot_id: FrameSlotId,
    kind: LateLoweredFrameSlotKind,
    ty: TypeId,
    write_points: Vec<StateId>,
    read_points: Vec<StateId>,
}

impl LateLoweredFrameSlot {
    pub fn new(
        slot_id: FrameSlotId,
        kind: LateLoweredFrameSlotKind,
        ty: TypeId,
        write_points: Vec<StateId>,
        read_points: Vec<StateId>,
    ) -> Self {
        Self {
            slot_id,
            kind,
            ty,
            write_points,
            read_points,
        }
    }

    pub fn slot_id(&self) -> FrameSlotId {
        self.slot_id
    }

    pub fn kind(&self) -> LateLoweredFrameSlotKind {
        self.kind
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn write_points(&self) -> &[StateId] {
        &self.write_points
    }

    pub fn read_points(&self) -> &[StateId] {
        &self.read_points
    }
}

/// late-lowered callable 的 frame schema 壳层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredFrameSchema {
    slots: Vec<LateLoweredFrameSlot>,
}

impl LateLoweredFrameSchema {
    pub fn new(slots: Vec<LateLoweredFrameSlot>) -> Self {
        Self { slots }
    }

    pub(crate) fn empty() -> Self {
        Self { slots: Vec::new() }
    }

    pub fn slots(&self) -> &[LateLoweredFrameSlot] {
        &self.slots
    }

    pub fn slot_for_kind(&self, kind: LateLoweredFrameSlotKind) -> Option<&LateLoweredFrameSlot> {
        self.slots.iter().find(|slot| slot.kind() == kind)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};
    use std::path::PathBuf;

    use super::*;
    use crate::effect_lowered::LateLoweredProgramBuilder;
    use crate::effect_refactor_pipeline::{
        load_effect_facts_stage_output_for_dump, load_effect_lowered_stage_output_for_dump,
    };
    use crate::mir::TemplateKey;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::ty::{NominalType, RefTypeKind, TypeKind, TypeStore};

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    struct RawProgramOutput {
        effect_facts_stage_output: crate::effect_refactor_pipeline::RefactorEffectFactsStageOutput,
        program: LateLoweredProgram,
    }

    impl RawProgramOutput {
        fn program(&self) -> &LateLoweredProgram {
            &self.program
        }

        fn types(&self) -> &TypeStore {
            self.effect_facts_stage_output.types()
        }
    }

    fn build_raw_output(source: &SourceFile) -> RawProgramOutput {
        let session = refactor_session();
        let effect_facts_output = load_effect_facts_stage_output_for_dump(&session, source)
            .expect("fixture 应可通过 refactor effect-facts stage");
        let program = LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_output.materialized_pass_view(),
            effect_facts_output.effect_facts(),
            effect_facts_output.types(),
        )
        .build()
        .expect("fixture 应可通过 raw late-lowering builder");
        RawProgramOutput {
            effect_facts_stage_output: effect_facts_output,
            program,
        }
    }

    fn build_raw_program(source: &SourceFile) -> LateLoweredProgram {
        build_raw_output(source).program
    }

    fn sample_instance_key(fqn: &str) -> InstanceKey {
        InstanceKey {
            template: TemplateKey {
                fqn: fqn.to_string(),
                source_path: PathBuf::from("<mem>/sample.scoop"),
                decl_span: Span::new(0, 0),
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        }
    }

    fn nominal_effect(types: &mut TypeStore, fqn: &str) -> TypeId {
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: fqn.to_string(),
            args: Vec::new(),
            eff: None,
        })))
    }

    fn sample_manual_program() -> LateLoweredProgram {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let invoke_args_tuple_ty = types.ty_tuple(vec![builtins.int]);
        let payload_tuple_ty = types.ty_tuple(vec![builtins.string]);
        let resume_tuple_ty = types.ty_tuple(vec![builtins.int]);
        let continuation_obj_ty = nominal_effect(&mut types, "sample.CompilerContinuation");
        let surface_ty0 = nominal_effect(&mut types, "sample.SurfaceContinuation0");
        let surface_ty1 = nominal_effect(&mut types, "sample.SurfaceContinuation1");
        let allowed_row = EffectRow::new(vec![nominal_effect(&mut types, "sample.Ping")]);
        let ping_family =
            crate::effect_facts::EffectFamilyKey::new("sample.Ping".to_string(), Vec::new());

        let step_schema = StepSchemaId::new(7);
        let case0 = CaseTag::new(0);
        let case1 = CaseTag::new(1);
        let cont_schema0 = ContinuationSchemaId::new(3);
        let cont_schema1 = ContinuationSchemaId::new(4);
        let contract0 = LateLoweredContinuationContract::new(
            cont_schema0,
            resume_tuple_ty,
            builtins.unit,
            step_schema,
            surface_ty0,
        );
        let contract1 = LateLoweredContinuationContract::new(
            cont_schema1,
            builtins.unit,
            builtins.unit,
            step_schema,
            surface_ty1,
        );

        let step_type = LateLoweredStepType::new(
            step_schema,
            invoke_args_tuple_ty,
            builtins.unit,
            continuation_obj_ty,
            vec![
                LateLoweredStepCase::new(
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    payload_tuple_ty,
                    contract0,
                ),
                LateLoweredStepCase::new(
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    builtins.unit,
                    contract1,
                ),
            ],
        );

        let interface_id = ResumeInterfaceId::new(0);
        let resume_interface = LateLoweredResumeInterface::new(
            interface_id,
            ping_family.clone(),
            step_schema,
            vec![
                LateLoweredResumeMethod::new(
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    contract0,
                ),
                LateLoweredResumeMethod::new(
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    contract1,
                ),
            ],
        );

        let version_key = LateLoweredBodyVersionKey::new(
            sample_instance_key("sample.worker"),
            allowed_row,
            ImplPlan::SingleCase(case0),
            true,
        );
        let continuation_object_id = ContinuationObjectId::new(0);
        let state0 = StateId::new(0);
        let state1 = StateId::new(1);
        let state2 = StateId::new(2);
        let state3 = StateId::new(3);
        let state4 = StateId::new(4);
        let boundary0 = BoundaryId::new(0);
        let slot0 = FrameSlotId::new(0);
        let slot1 = FrameSlotId::new(1);
        let slot2 = FrameSlotId::new(2);
        let slot3 = FrameSlotId::new(3);
        let slot4 = FrameSlotId::new(4);
        let slot5 = FrameSlotId::new(5);
        let slot6 = FrameSlotId::new(6);
        let slot7 = FrameSlotId::new(7);
        let slot8 = FrameSlotId::new(8);
        let slot9 = FrameSlotId::new(9);
        let slot10 = FrameSlotId::new(10);

        let continuation_object = LateLoweredContinuationObject::new(
            continuation_object_id,
            version_key.clone(),
            continuation_obj_ty,
            vec![interface_id],
            vec![
                LateLoweredContinuationCapture::FrameSlot(slot4),
                LateLoweredContinuationCapture::State(state2),
            ],
            vec![
                LateLoweredContinuationSurfaceResume::new(
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    contract0,
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
                    },
                ),
                LateLoweredContinuationSurfaceResume::new(
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    contract1,
                    LateLoweredContinuationResumeBody::Unreachable,
                ),
            ],
            vec![
                LateLoweredContinuationMethod::new(
                    interface_id,
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    contract0,
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
                    },
                ),
                LateLoweredContinuationMethod::new(
                    interface_id,
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    contract1,
                    LateLoweredContinuationResumeBody::Unreachable,
                ),
            ],
        );

        let callable = LateLoweredCallable::new(
            "sample.worker".to_string(),
            version_key,
            step_schema,
            vec![case0],
            LateLoweredDynamicInvokeEntry::new(invoke_args_tuple_ty, step_schema, state0, state1),
            LateLoweredStateGraph::new(
                state0,
                state1,
                Some(state3),
                Some(state4),
                vec![
                    LateLoweredState::new(
                        state0,
                        LateLoweredStateRole::Entry,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(0),
                            0,
                            1,
                            false,
                        )],
                        LateLoweredStateTerminator::Suspend {
                            boundary_ids: vec![boundary0],
                            resume_state: state2,
                            local_runtime_error_states: Vec::new(),
                            cleanup_state: Some(state3),
                            drop_state: Some(state4),
                        },
                    ),
                    LateLoweredState::new(
                        state2,
                        LateLoweredStateRole::Resume,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(0),
                            1,
                            1,
                            true,
                        )],
                        LateLoweredStateTerminator::Return {
                            value_local: Some(LocalId::from_raw(0)),
                            complete_state: state1,
                        },
                    ),
                    LateLoweredState::new(
                        state1,
                        LateLoweredStateRole::Complete,
                        Vec::new(),
                        LateLoweredStateTerminator::Unreachable,
                    ),
                    LateLoweredState::new(
                        state3,
                        LateLoweredStateRole::Cleanup,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(3),
                            0,
                            0,
                            true,
                        )],
                        LateLoweredStateTerminator::Goto { target: state1 },
                    ),
                    LateLoweredState::new(
                        state4,
                        LateLoweredStateRole::Drop,
                        Vec::new(),
                        LateLoweredStateTerminator::Abandon,
                    ),
                ],
            ),
            LateLoweredFrameSchema::new(vec![
                LateLoweredFrameSlot::new(
                    slot0,
                    LateLoweredFrameSlotKind::SourceLocal(LocalId::from_raw(0)),
                    builtins.int,
                    vec![state0],
                    vec![state2],
                ),
                LateLoweredFrameSlot::new(
                    slot1,
                    LateLoweredFrameSlotKind::CompilerTemporary(LocalId::from_raw(1)),
                    builtins.string,
                    vec![state0],
                    vec![state2],
                ),
                LateLoweredFrameSlot::new(
                    slot2,
                    LateLoweredFrameSlotKind::JoinValue {
                        local: LocalId::from_raw(2),
                        block: BasicBlockId::from_raw(4),
                        ordinal: 1,
                    },
                    builtins.int,
                    vec![state0],
                    vec![state2],
                ),
                LateLoweredFrameSlot::new(
                    slot3,
                    LateLoweredFrameSlotKind::HandleBinder {
                        site_id: SiteId::from_raw(2),
                        local: LocalId::from_raw(3),
                        ordinal: 0,
                    },
                    builtins.string,
                    vec![state2],
                    vec![state3],
                ),
                LateLoweredFrameSlot::new(
                    slot4,
                    LateLoweredFrameSlotKind::ResumePayload {
                        boundary: boundary0,
                        case_tag: case0,
                    },
                    resume_tuple_ty,
                    vec![state2],
                    vec![state2],
                ),
                LateLoweredFrameSlot::new(
                    slot10,
                    LateLoweredFrameSlotKind::BoundaryResult {
                        boundary: boundary0,
                        local: LocalId::from_raw(4),
                    },
                    builtins.int,
                    vec![state2],
                    vec![state3],
                ),
                LateLoweredFrameSlot::new(
                    slot5,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::StateTag),
                    builtins.int,
                    Vec::new(),
                    Vec::new(),
                ),
                LateLoweredFrameSlot::new(
                    slot6,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::ResumePayloadCarrier),
                    payload_tuple_ty,
                    Vec::new(),
                    Vec::new(),
                ),
                LateLoweredFrameSlot::new(
                    slot7,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::CleanupFlag),
                    builtins.bool_,
                    Vec::new(),
                    Vec::new(),
                ),
                LateLoweredFrameSlot::new(
                    slot8,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::OneShotFlag),
                    builtins.bool_,
                    Vec::new(),
                    Vec::new(),
                ),
                LateLoweredFrameSlot::new(
                    slot9,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::CompletionTag),
                    builtins.int,
                    Vec::new(),
                    Vec::new(),
                ),
            ]),
            LateLoweredBoundaryMap::new(vec![LateLoweredBoundary::new(
                boundary0,
                LateLoweredBoundarySource::Site {
                    site_id: SiteId::from_raw(1),
                    kind: BoundarySiteKind::Perform,
                },
                state0,
                state2,
            )]),
            LateLoweredResumeStateMap::new(vec![LateLoweredResumeState::new(boundary0, state2)]),
            continuation_object_id,
            vec![interface_id],
        );

        LateLoweredProgram::new(
            vec![step_type],
            vec![resume_interface],
            vec![continuation_object],
            vec![callable],
        )
    }

    #[test]
    fn refactor_body_version_key_keeps_allowed_row_in_identity() {
        let mut types = TypeStore::new();
        let alpha = nominal_effect(&mut types, "sample.Alpha");
        let beta = nominal_effect(&mut types, "sample.Beta");
        let instance = sample_instance_key("sample.callValue");

        let alpha_key = LateLoweredBodyVersionKey::new(
            instance.clone(),
            EffectRow::new(vec![alpha]),
            ImplPlan::CanonicalFull,
            true,
        );
        let beta_key = LateLoweredBodyVersionKey::new(
            instance,
            EffectRow::new(vec![beta]),
            ImplPlan::CanonicalFull,
            true,
        );

        assert_ne!(alpha_key, beta_key);
        assert_eq!(HashSet::from([alpha_key, beta_key]).len(), 2);
    }

    #[test]
    fn refactor_body_version_key_distinguishes_single_case_and_canonical_full_versions() {
        let mut types = TypeStore::new();
        let alpha = nominal_effect(&mut types, "sample.Alpha");
        let allowed_row = EffectRow::new(vec![alpha]);
        let instance = sample_instance_key("sample.resumeBoom");

        let no_outward = LateLoweredBodyVersionKey::new(
            instance.clone(),
            allowed_row.clone(),
            ImplPlan::NoOutward,
            false,
        );
        let single_case = LateLoweredBodyVersionKey::new(
            instance.clone(),
            allowed_row.clone(),
            ImplPlan::SingleCase(CaseTag::new(0)),
            true,
        );
        let another_single_case = LateLoweredBodyVersionKey::new(
            instance.clone(),
            allowed_row.clone(),
            ImplPlan::SingleCase(CaseTag::new(1)),
            true,
        );
        let canonical =
            LateLoweredBodyVersionKey::new(instance, allowed_row, ImplPlan::CanonicalFull, true);

        assert_ne!(no_outward, single_case);
        assert_ne!(single_case, another_single_case);
        assert_ne!(single_case, canonical);
        assert_eq!(
            HashSet::from([no_outward, single_case, another_single_case, canonical]).len(),
            4
        );
    }

    #[test]
    fn refactor_late_lowered_ir_step_materialization_shell_keeps_canonical_cases_for_single_case_versions()
     {
        let program = sample_manual_program();
        let callable = program
            .callable("sample.worker")
            .expect("sample program 应保留 callable shell");
        let step_type = program
            .step_type(callable.step_schema())
            .expect("callable 应能按 step schema 回查 canonical Step shell");

        assert_eq!(callable.impl_plan(), ImplPlan::SingleCase(CaseTag::new(0)));
        assert_eq!(step_type.cases().len(), 2);
        assert_eq!(step_type.cases()[0].case_tag(), CaseTag::new(0));
        assert_eq!(step_type.cases()[1].case_tag(), CaseTag::new(1));
        assert_eq!(
            callable.dynamic_invoke_entry().step_schema(),
            callable.step_schema(),
        );
    }

    #[test]
    fn refactor_late_lowered_ir_resume_interface_shell_records_complete_methods_and_reachability() {
        let program = sample_manual_program();
        let callable = program
            .callable("sample.worker")
            .expect("sample program 应保留 callable shell");
        let continuation_object = program
            .continuation_object(callable.continuation_object())
            .expect("callable 应能回查 continuation object shell");
        let resume_interface = program
            .resume_packing(callable.resume_packings()[0])
            .expect("callable 应能回查 resume interface shell");

        assert_eq!(
            resume_interface.return_step_schema(),
            callable.step_schema()
        );
        assert_eq!(resume_interface.effect_family().effect_fqn(), "sample.Ping");
        assert_eq!(resume_interface.methods().len(), 2);
        assert_eq!(
            resume_interface.methods()[0].out_step_schema(),
            callable.step_schema()
        );
        assert_eq!(
            continuation_object.implemented_packings(),
            callable.resume_packings()
        );
        assert_eq!(continuation_object.methods().len(), 2);
        assert_eq!(continuation_object.surface_resumes().len(), 2);
        assert_eq!(
            continuation_object.methods()[0].out_step_schema(),
            callable.step_schema()
        );
        assert_eq!(
            continuation_object.methods()[0].reachability(),
            LateLoweredContinuationMethodReachability::Reachable,
        );
        assert_eq!(
            continuation_object.methods()[1].reachability(),
            LateLoweredContinuationMethodReachability::Unreachable,
        );
    }

    #[test]
    fn refactor_late_lowered_ir_stable_dump_exposes_frame_slot_categories() {
        let program = sample_manual_program();
        let dump = program.stable_dump();

        for needle in [
            "SourceLocal",
            "CompilerTemporary",
            "JoinValue",
            "HandleBinder",
            "ResumePayload",
            "StateTag",
            "ResumePayloadCarrier",
            "CleanupFlag",
            "OneShotFlag",
            "CompletionTag",
        ] {
            assert!(
                dump.contains(needle),
                "stable dump 应显式暴露 frame slot 分类: {needle}\n{dump}"
            );
        }
    }

    #[test]
    fn refactor_late_lowered_ir_stable_dump_demotes_packings_but_keeps_authoritative_cases_visible()
    {
        let program = sample_manual_program();
        let dump = program.stable_dump();

        for needle in [
            "resume_packing_interface_count: 1",
            "continuation_objects:",
            "authoritative_surface_resume_dispatch_inventory:",
            "resume_packing_interfaces:",
            "implemented_packings: [ri0]",
            "authoritative_surface_resumes:",
            "authoritative_internal_methods:",
            "resume_packing_interfaces: [ri0]",
            "case=c0 packed_by=ri0",
            "case=c1 packed_by=ri0",
            "concrete_op=sample.Ping.hit",
            "concrete_op=sample.Ping.pong",
        ] {
            assert!(
                dump.contains(needle),
                "stable dump 应把 authoritative contract 与 packing layer 清晰分开: {needle}\n{dump}"
            );
        }
    }

    #[test]
    fn refactor_late_lowered_ir_builder_materializes_program_shells_from_effect_facts() {
        let source = load_fixture("effect_facts", "single_case_impl_plan.scoop");
        let program = build_raw_program(&source);

        assert!(!program.step_types().is_empty());
        assert!(!program.resume_packings().is_empty());
        assert!(!program.continuation_objects().is_empty());

        let leaf = program
            .callable("sample.leaf")
            .expect("fixture 应发布 sample.leaf callable shell");
        let step_type = program
            .step_type(leaf.step_schema())
            .expect("callable 应能回查对应 Step shell");
        let continuation_object = program
            .continuation_object(leaf.continuation_object())
            .expect("callable 应能回查 continuation object shell");
        let resume_interfaces = leaf
            .resume_packings()
            .iter()
            .map(|interface_id| {
                program
                    .resume_packing(*interface_id)
                    .expect("callable 应能回查 resume interface shell")
            })
            .collect::<Vec<_>>();

        assert_eq!(step_type.cases().len(), 2);
        assert!(matches!(leaf.impl_plan(), ImplPlan::SingleCase(_)));
        assert_eq!(resume_interfaces.len(), 2);
        assert!(
            resume_interfaces
                .iter()
                .all(|interface| interface.return_step_schema() == leaf.step_schema())
        );
        assert_eq!(
            resume_interfaces
                .iter()
                .map(|interface| interface.methods().len())
                .sum::<usize>(),
            step_type.cases().len()
        );
        assert_eq!(continuation_object.methods().len(), step_type.cases().len());
        assert_eq!(
            continuation_object.surface_resumes().len(),
            step_type.cases().len()
        );
        assert_eq!(
            continuation_object.implemented_packings(),
            leaf.resume_packings(),
        );
        assert_eq!(
            resume_interfaces
                .iter()
                .map(|interface| interface.effect_family().effect_fqn().to_string())
                .collect::<BTreeSet<_>>(),
            ["sample.Ping".to_string(), "scoop.core.Raise".to_string()]
                .into_iter()
                .collect()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().invoke_args_tuple_ty(),
            step_type.invoke_args_tuple_ty(),
        );
    }

    fn step_case_fqns(step_type: &LateLoweredStepType) -> BTreeSet<String> {
        step_type
            .cases()
            .iter()
            .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
            .collect()
    }

    #[test]
    fn refactor_resume_interface_uses_out_step_schema_not_surface_ty_for_runtime_error_case() {
        let source = load_fixture("effect_facts", "dispatch_and_resume_call.scoop");
        let output = build_raw_output(&source);
        let method = output
            .program()
            .resume_packings()
            .iter()
            .flat_map(|interface| interface.methods().iter())
            .find(|method| {
                output.types().display(method.surface_ty()).to_string()
                    == "scoop.core.Continuation<Int, Unit, eff fixtures.mir.Boom>"
            })
            .expect("应找到 surface row 仍为 Boom 的 resume method shell");
        let out_step = output
            .program()
            .step_type(method.out_step_schema())
            .expect("resume method 的 out_step_schema 应对应已物化的 Step shell");

        assert_eq!(
            output.types().display(method.surface_ty()).to_string(),
            "scoop.core.Continuation<Int, Unit, eff fixtures.mir.Boom>"
        );
        assert_eq!(
            step_case_fqns(out_step),
            [
                "fixtures.mir.Boom.next".to_string(),
                "scoop.core.Raise.raise".to_string(),
            ]
            .into_iter()
            .collect(),
            "late-lowered resume interface 必须从 out_step_schema 继承 one-shot runtime-error upper bound，而不是从 surface_ty 推断"
        );
    }

    #[test]
    fn refactor_continuation_object_one_shot_runtime_error_preserves_surface_row_in_shell() {
        let session = refactor_session();
        let source = load_fixture("effect_facts", "single_case_impl_plan.scoop");
        let output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 refactor late-lowering stage");
        let leaf = output
            .program()
            .callable("sample.leaf")
            .expect("fixture 应发布 sample.leaf callable shell");
        let step_type = output
            .program()
            .step_type(leaf.step_schema())
            .expect("callable 应能回查 Step shell");
        let continuation_object = output
            .program()
            .continuation_object(leaf.continuation_object())
            .expect("callable 应能回查 continuation object shell");
        let reachable_method = continuation_object
            .methods()
            .iter()
            .find(|method| {
                method.reachability() == LateLoweredContinuationMethodReachability::Reachable
            })
            .expect("single-case callable 应至少有一个 reachable continuation method");

        assert_eq!(
            output
                .types()
                .display(reachable_method.surface_ty())
                .to_string(),
            "scoop.core.Continuation<Unit, Unit, eff sample.Ping>"
        );
        assert_eq!(reachable_method.out_step_schema(), leaf.step_schema());
        assert_eq!(
            step_case_fqns(step_type),
            [
                "sample.Ping.hit".to_string(),
                "scoop.core.Raise.raise".to_string(),
            ]
            .into_iter()
            .collect(),
            "callable Step shell 仍应保留 compiler-generated runtime-error case"
        );
        assert!(
            !output
                .types()
                .display(reachable_method.surface_ty())
                .to_string()
                .contains("RuntimeError"),
            "source-visible continuation surface 不应因 one-shot runtime-error upper bound 被无端扩大"
        );
    }

    #[test]
    fn refactor_effect_lowered_stage_surface_ty_does_not_control_runtime_error_case_contracts() {
        let session = refactor_session();
        let source = load_fixture("effect_facts", "dispatch_and_resume_call.scoop");
        let output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 refactor late-lowering stage");
        let widened_surface_method = output
            .program()
            .resume_packings()
            .iter()
            .flat_map(|interface| interface.methods().iter())
            .find(|method| {
                output.types().display(method.surface_ty()).to_string()
                    == "scoop.core.Continuation<Int, Unit, eff (scoop.core.Raise<scoop.core.RuntimeError> + fixtures.mir.Boom)>"
            })
            .expect("应保留 source residual row 本就含 runtime error 的 resume method shell");
        let dump = output.stable_dump();

        assert_eq!(
            output
                .types()
                .display(widened_surface_method.surface_ty())
                .to_string(),
            "scoop.core.Continuation<Int, Unit, eff (scoop.core.Raise<scoop.core.RuntimeError> + fixtures.mir.Boom)>"
        );
        assert!(dump.contains("surface_ty="));
        assert!(dump.contains("out_step_schema=s"));
        assert!(dump.contains("answer_ty="));
    }
}
