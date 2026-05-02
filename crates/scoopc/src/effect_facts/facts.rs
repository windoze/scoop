use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::mir::{BasicBlockId, InstanceKey, MaterializedMirPassView, SiteId};
use crate::ty::{EffectRow, TypeId};

use super::schema::{
    CaseSet, CaseTag, ContinuationSchema, ContinuationSchemaId, ImplPlan, StepSchema, StepSchemaId,
};

/// `MaterializedEffectFacts` 当前绑定到哪一种 canonical MIR 查询面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalMirQuerySurface {
    PassView,
}

/// facts 与当前 canonical materialized MIR snapshot 的绑定信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirSnapshotBinding {
    query_surface: CanonicalMirQuerySurface,
    instance_count: usize,
    canonical_body_fqns: Vec<String>,
}

impl MirSnapshotBinding {
    pub fn query_surface(&self) -> CanonicalMirQuerySurface {
        self.query_surface
    }

    pub fn instance_count(&self) -> usize {
        self.instance_count
    }

    pub fn canonical_body_fqns(&self) -> &[String] {
        &self.canonical_body_fqns
    }

    pub(crate) fn from_pass_view(pass_view: &MaterializedMirPassView<'_>) -> Self {
        let mut canonical_body_fqns = BTreeSet::new();
        for family in pass_view.instances() {
            for fun in family.callable_bodies() {
                canonical_body_fqns.insert(fun.fqn.clone());
            }
        }
        Self {
            query_surface: CanonicalMirQuerySurface::PassView,
            instance_count: pass_view.len(),
            canonical_body_fqns: canonical_body_fqns.into_iter().collect(),
        }
    }
}

/// callable-level facts 的最终 public 形状。
///
/// builder 会先种下保守壳层，随后由 P4 的 solver 基于 body/site facts 完成
/// `resolved_outward_cases` / `needs_reentry` / `impl_plan` 的 finalization。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableEffectFacts {
    declared_row: EffectRow,
    invoke_args_tuple_ty: TypeId,
    step_schema: StepSchemaId,
    resolved_outward_cases: CaseSet,
    needs_reentry: bool,
    impl_plan: ImplPlan,
}

impl CallableEffectFacts {
    pub fn new(
        declared_row: EffectRow,
        invoke_args_tuple_ty: TypeId,
        step_schema: StepSchemaId,
        resolved_outward_cases: CaseSet,
        needs_reentry: bool,
        impl_plan: ImplPlan,
    ) -> Self {
        Self {
            declared_row,
            invoke_args_tuple_ty,
            step_schema,
            resolved_outward_cases,
            needs_reentry,
            impl_plan,
        }
    }

    pub fn declared_row(&self) -> &EffectRow {
        &self.declared_row
    }

    pub fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub fn resolved_outward_cases(&self) -> &CaseSet {
        &self.resolved_outward_cases
    }

    pub fn needs_reentry(&self) -> bool {
        self.needs_reentry
    }

    pub fn impl_plan(&self) -> ImplPlan {
        self.impl_plan
    }
}

/// site-level effect facts 当前的精度来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectPrecision {
    Precise,
    Widened,
    SignatureFallback,
}

/// 当前 call site 的 target 解析档位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTargetMode {
    KnownInstance,
    CandidateSet,
    DynamicFallback,
}

/// 当前 call site 对外暴露的 target 身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallSiteTarget {
    KnownInstance(InstanceKey),
    CandidateSet(Vec<InstanceKey>),
    DynamicFallback,
}

impl CallSiteTarget {
    pub fn mode(&self) -> CallTargetMode {
        match self {
            Self::KnownInstance(_) => CallTargetMode::KnownInstance,
            Self::CandidateSet(_) => CallTargetMode::CandidateSet,
            Self::DynamicFallback => CallTargetMode::DynamicFallback,
        }
    }
}

/// `Rvalue::Call` 在 MIR 上的语言级调用形态分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallSiteKind {
    Direct,
    Closure,
    FunValue,
    Virtual,
    Interface,
}

/// 普通 call site 的结构化 effect facts。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSiteEffectFacts {
    kind: CallSiteKind,
    target_mode: CallTargetMode,
    target: CallSiteTarget,
    invoke_args_tuple_ty: TypeId,
    callee_schema: StepSchemaId,
    resolved_cases: CaseSet,
    precision: EffectPrecision,
}

impl CallSiteEffectFacts {
    pub fn new(
        kind: CallSiteKind,
        target: CallSiteTarget,
        invoke_args_tuple_ty: TypeId,
        callee_schema: StepSchemaId,
        resolved_cases: CaseSet,
        precision: EffectPrecision,
    ) -> Self {
        let target_mode = target.mode();
        Self {
            kind,
            target_mode,
            target,
            invoke_args_tuple_ty,
            callee_schema,
            resolved_cases,
            precision,
        }
    }

    pub fn kind(&self) -> CallSiteKind {
        self.kind
    }

    pub fn target_mode(&self) -> CallTargetMode {
        self.target_mode
    }

    pub fn target(&self) -> &CallSiteTarget {
        &self.target
    }

    pub fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub fn callee_schema(&self) -> StepSchemaId {
        self.callee_schema
    }

    pub fn resolved_cases(&self) -> &CaseSet {
        &self.resolved_cases
    }

    pub fn precision(&self) -> EffectPrecision {
        self.precision
    }
}

/// `perform` site 的 emitted-case / captured continuation contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformSiteEffectFacts {
    emitted_case: CaseTag,
    payload_tuple_ty: TypeId,
    captured_cont_schema: ContinuationSchemaId,
}

impl PerformSiteEffectFacts {
    pub fn new(
        emitted_case: CaseTag,
        payload_tuple_ty: TypeId,
        captured_cont_schema: ContinuationSchemaId,
    ) -> Self {
        Self {
            emitted_case,
            payload_tuple_ty,
            captured_cont_schema,
        }
    }

    pub fn emitted_case(&self) -> CaseTag {
        self.emitted_case
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn captured_cont_schema(&self) -> ContinuationSchemaId {
        self.captured_cont_schema
    }
}

/// `resume` site 的 continuation contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeSiteEffectFacts {
    continuation_schema: ContinuationSchemaId,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
    resolved_cases: CaseSet,
}

impl ResumeSiteEffectFacts {
    pub fn new(
        continuation_schema: ContinuationSchemaId,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        out_step_schema: StepSchemaId,
        resolved_cases: CaseSet,
    ) -> Self {
        Self {
            continuation_schema,
            resume_tuple_ty,
            answer_ty,
            out_step_schema,
            resolved_cases,
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

    pub fn resolved_cases(&self) -> &CaseSet {
        &self.resolved_cases
    }
}

/// nested `handle` 是否会把 suspension/outward 继续暴露给外层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedHandleClassification {
    SelfContained,
    MaySuspendOutward,
}

/// 单个 handle arm 的结构化 effect facts。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleArmEffectFacts {
    handled_case: CaseTag,
    payload_tuple_ty: TypeId,
    continuation_schema: ContinuationSchemaId,
    arm_outward_cases: CaseSet,
}

impl HandleArmEffectFacts {
    pub fn new(
        handled_case: CaseTag,
        payload_tuple_ty: TypeId,
        continuation_schema: ContinuationSchemaId,
        arm_outward_cases: CaseSet,
    ) -> Self {
        Self {
            handled_case,
            payload_tuple_ty,
            continuation_schema,
            arm_outward_cases,
        }
    }

    pub fn handled_case(&self) -> CaseTag {
        self.handled_case
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }

    pub fn arm_outward_cases(&self) -> &CaseSet {
        &self.arm_outward_cases
    }
}

/// `handle` site 的结构化 contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleSiteEffectFacts {
    result_ty: TypeId,
    handled_cases: CaseSet,
    body_outward_cases: CaseSet,
    arm_facts: Vec<HandleArmEffectFacts>,
    finally_outward_cases: CaseSet,
    nested_handle_classification: NestedHandleClassification,
}

impl HandleSiteEffectFacts {
    pub fn new(
        result_ty: TypeId,
        handled_cases: CaseSet,
        body_outward_cases: CaseSet,
        arm_facts: Vec<HandleArmEffectFacts>,
        finally_outward_cases: CaseSet,
        nested_handle_classification: NestedHandleClassification,
    ) -> Self {
        Self {
            result_ty,
            handled_cases,
            body_outward_cases,
            arm_facts,
            finally_outward_cases,
            nested_handle_classification,
        }
    }

    pub fn result_ty(&self) -> TypeId {
        self.result_ty
    }

    pub fn handled_cases(&self) -> &CaseSet {
        &self.handled_cases
    }

    pub fn body_outward_cases(&self) -> &CaseSet {
        &self.body_outward_cases
    }

    pub fn arm_facts(&self) -> &[HandleArmEffectFacts] {
        &self.arm_facts
    }

    pub fn finally_outward_cases(&self) -> &CaseSet {
        &self.finally_outward_cases
    }

    pub fn nested_handle_classification(&self) -> NestedHandleClassification {
        self.nested_handle_classification
    }
}

/// body 内单个 site 的 facts 变体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteEffectFacts {
    Call(CallSiteEffectFacts),
    Perform(PerformSiteEffectFacts),
    Resume(ResumeSiteEffectFacts),
    Handle(HandleSiteEffectFacts),
}

/// 单个 basic block 的结构化 effect facts。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEffectFacts {
    ambient_cases: CaseSet,
    outward_cases: CaseSet,
    has_suspend_boundary: bool,
    has_handle_boundary: bool,
}

impl BlockEffectFacts {
    pub fn new(
        ambient_cases: CaseSet,
        outward_cases: CaseSet,
        has_suspend_boundary: bool,
        has_handle_boundary: bool,
    ) -> Self {
        Self {
            ambient_cases,
            outward_cases,
            has_suspend_boundary,
            has_handle_boundary,
        }
    }

    pub fn ambient_cases(&self) -> &CaseSet {
        &self.ambient_cases
    }

    pub fn outward_cases(&self) -> &CaseSet {
        &self.outward_cases
    }

    pub fn has_suspend_boundary(&self) -> bool {
        self.has_suspend_boundary
    }

    pub fn has_handle_boundary(&self) -> bool {
        self.has_handle_boundary
    }
}

/// solver 在当前 body 上完成 callable/block finalization 所需的结构输入。
#[derive(Debug, Clone, Default)]
pub(crate) struct BodyEffectSolverFacts {
    block_successors: BTreeMap<BasicBlockId, Vec<BasicBlockId>>,
    block_sites: BTreeMap<BasicBlockId, Vec<SiteId>>,
    block_handled_cases: BTreeMap<BasicBlockId, CaseSet>,
}

impl BodyEffectSolverFacts {
    pub(crate) fn new(
        block_successors: BTreeMap<BasicBlockId, Vec<BasicBlockId>>,
        block_sites: BTreeMap<BasicBlockId, Vec<SiteId>>,
        block_handled_cases: BTreeMap<BasicBlockId, CaseSet>,
    ) -> Self {
        Self {
            block_successors,
            block_sites,
            block_handled_cases,
        }
    }

    pub(crate) fn block_successors(&self) -> &BTreeMap<BasicBlockId, Vec<BasicBlockId>> {
        &self.block_successors
    }

    pub(crate) fn block_sites(&self) -> &BTreeMap<BasicBlockId, Vec<SiteId>> {
        &self.block_sites
    }

    pub(crate) fn handled_cases_for_block(&self, block: BasicBlockId) -> Option<&CaseSet> {
        self.block_handled_cases.get(&block)
    }
}

/// 当前 materialized callable body 的局部 effect facts。
#[derive(Debug, Clone, Default)]
pub struct BodyEffectFacts {
    blocks: BTreeMap<BasicBlockId, BlockEffectFacts>,
    sites: BTreeMap<SiteId, SiteEffectFacts>,
    solver_facts: BodyEffectSolverFacts,
}

impl BodyEffectFacts {
    pub fn new(
        blocks: BTreeMap<BasicBlockId, BlockEffectFacts>,
        sites: BTreeMap<SiteId, SiteEffectFacts>,
    ) -> Self {
        Self {
            blocks,
            sites,
            solver_facts: BodyEffectSolverFacts::default(),
        }
    }

    pub(crate) fn with_solver_facts(
        blocks: BTreeMap<BasicBlockId, BlockEffectFacts>,
        sites: BTreeMap<SiteId, SiteEffectFacts>,
        solver_facts: BodyEffectSolverFacts,
    ) -> Self {
        Self {
            blocks,
            sites,
            solver_facts,
        }
    }

    pub fn blocks(&self) -> &BTreeMap<BasicBlockId, BlockEffectFacts> {
        &self.blocks
    }

    pub fn block(&self, block: BasicBlockId) -> Option<&BlockEffectFacts> {
        self.blocks.get(&block)
    }

    pub fn sites(&self) -> &BTreeMap<SiteId, SiteEffectFacts> {
        &self.sites
    }

    pub fn site(&self, site: SiteId) -> Option<&SiteEffectFacts> {
        self.sites.get(&site)
    }

    pub(crate) fn solver_facts(&self) -> &BodyEffectSolverFacts {
        &self.solver_facts
    }
}

/// refactor 主线的 authoritative effect-facts 容器。
///
/// 生命周期规则：
/// - 与当前 canonical materialized MIR snapshot 一一对应；
/// - 结构性 rewrite 后必须基于新的 snapshot 重建；
/// - 不对外暴露“部分 body 已更新、部分 body 仍过期”的混合状态。
pub(crate) type MaterializedEffectFactsParts = (
    MirSnapshotBinding,
    BTreeMap<StepSchemaId, StepSchema>,
    BTreeMap<ContinuationSchemaId, ContinuationSchema>,
    HashMap<InstanceKey, CallableEffectFacts>,
    HashMap<InstanceKey, BodyEffectFacts>,
);

#[derive(Debug, Clone)]
pub struct MaterializedEffectFacts {
    snapshot_binding: MirSnapshotBinding,
    step_schemas: BTreeMap<StepSchemaId, StepSchema>,
    continuation_schemas: BTreeMap<ContinuationSchemaId, ContinuationSchema>,
    callable_facts: HashMap<InstanceKey, CallableEffectFacts>,
    bodies: HashMap<InstanceKey, BodyEffectFacts>,
}

impl MaterializedEffectFacts {
    pub(crate) fn new(
        snapshot_binding: MirSnapshotBinding,
        step_schemas: BTreeMap<StepSchemaId, StepSchema>,
        continuation_schemas: BTreeMap<ContinuationSchemaId, ContinuationSchema>,
        callable_facts: HashMap<InstanceKey, CallableEffectFacts>,
        bodies: HashMap<InstanceKey, BodyEffectFacts>,
    ) -> Self {
        Self {
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callable_facts,
            bodies,
        }
    }

    pub fn snapshot_binding(&self) -> &MirSnapshotBinding {
        &self.snapshot_binding
    }

    pub fn step_schemas(&self) -> &BTreeMap<StepSchemaId, StepSchema> {
        &self.step_schemas
    }

    pub fn continuation_schemas(&self) -> &BTreeMap<ContinuationSchemaId, ContinuationSchema> {
        &self.continuation_schemas
    }

    pub fn callable_facts(&self) -> &HashMap<InstanceKey, CallableEffectFacts> {
        &self.callable_facts
    }

    pub fn bodies(&self) -> &HashMap<InstanceKey, BodyEffectFacts> {
        &self.bodies
    }

    pub fn body(&self, key: &InstanceKey) -> Option<&BodyEffectFacts> {
        self.bodies.get(key)
    }

    pub(crate) fn into_parts(self) -> MaterializedEffectFactsParts {
        (
            self.snapshot_binding,
            self.step_schemas,
            self.continuation_schemas,
            self.callable_facts,
            self.bodies,
        )
    }

    pub fn stable_dump(&self) -> String {
        super::dump::render_materialized_effect_facts(self)
    }
}
