//! Effect/control fact records that do not depend on MIR node definitions.

use std::collections::BTreeMap;

use scoopc_ids::{BodyBlockId, SiteId, StableEffectInstanceKey};
use scoopc_types::{EffectRow, TypeId};

use crate::schema::{CaseSet, CaseTag, ContinuationSchemaId, ImplPlan, StepSchemaId};

/// Canonical MIR query surface used to seed this effect-facts product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CanonicalMirQuerySurface {
    PassView,
}

/// Binding between effect facts and the canonical MIR snapshot they summarize.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectSnapshotBinding {
    query_surface: CanonicalMirQuerySurface,
    instance_count: usize,
    canonical_body_fqns: Vec<String>,
}

impl Default for EffectSnapshotBinding {
    fn default() -> Self {
        Self::new(CanonicalMirQuerySurface::PassView, 0, Vec::new())
    }
}

impl EffectSnapshotBinding {
    pub fn new(
        query_surface: CanonicalMirQuerySurface,
        instance_count: usize,
        canonical_body_fqns: Vec<String>,
    ) -> Self {
        Self {
            query_surface,
            instance_count,
            canonical_body_fqns,
        }
    }

    pub fn query_surface(&self) -> CanonicalMirQuerySurface {
        self.query_surface
    }

    pub fn instance_count(&self) -> usize {
        self.instance_count
    }

    pub fn canonical_body_fqns(&self) -> &[String] {
        &self.canonical_body_fqns
    }
}

/// Callable body or call-site ABI protocol selected by effect facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallableAbiKind {
    Plain,
    EffectStep,
}

/// Callable-level effect facts published after solving.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallableEffectFacts {
    declared_row: EffectRow,
    call_abi_kind: CallableAbiKind,
    invoke_args_tuple_ty: Option<TypeId>,
    step_schema: Option<StepSchemaId>,
    resolved_outward_cases: CaseSet,
    needs_reentry: bool,
    impl_plan: ImplPlan,
}

impl CallableEffectFacts {
    pub fn new(
        declared_row: EffectRow,
        call_abi_kind: CallableAbiKind,
        invoke_args_tuple_ty: Option<TypeId>,
        step_schema: Option<StepSchemaId>,
        resolved_outward_cases: CaseSet,
        needs_reentry: bool,
        impl_plan: ImplPlan,
    ) -> Self {
        Self {
            declared_row,
            call_abi_kind,
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

    pub fn call_abi_kind(&self) -> CallableAbiKind {
        self.call_abi_kind
    }

    pub fn invoke_args_tuple_ty_opt(&self) -> Option<TypeId> {
        self.invoke_args_tuple_ty
    }

    pub fn body_step_schema(&self) -> Option<StepSchemaId> {
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

/// Precision source for site-level effect facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EffectPrecision {
    Precise,
    Widened,
    SignatureFallback,
}

/// Current call target resolution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallTargetMode {
    KnownInstance,
    CandidateSet,
    DynamicFallback,
}

/// Stable target identity for a call site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallSiteTarget {
    KnownInstance(StableEffectInstanceKey),
    CandidateSet(Vec<StableEffectInstanceKey>),
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

/// Language-level call shape observed at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallSiteKind {
    Direct,
    Closure,
    FunValue,
    FunPtr,
    Virtual,
    Interface,
}

/// Structured facts for an ordinary call site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallSiteEffectFacts {
    kind: CallSiteKind,
    target_mode: CallTargetMode,
    target: CallSiteTarget,
    callee_abi_kind: CallableAbiKind,
    invoke_args_tuple_ty: TypeId,
    callee_schema: Option<StepSchemaId>,
    resolved_cases: CaseSet,
    precision: EffectPrecision,
}

impl CallSiteEffectFacts {
    pub fn new_with_abi(
        kind: CallSiteKind,
        target: CallSiteTarget,
        callee_abi_kind: CallableAbiKind,
        invoke_args_tuple_ty: TypeId,
        callee_schema: Option<StepSchemaId>,
        resolved_cases: CaseSet,
        precision: EffectPrecision,
    ) -> Self {
        let target_mode = target.mode();
        Self {
            kind,
            target_mode,
            target,
            callee_abi_kind,
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

    pub fn callee_abi_kind(&self) -> CallableAbiKind {
        self.callee_abi_kind
    }

    pub fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub fn callee_step_schema(&self) -> Option<StepSchemaId> {
        self.callee_schema
    }

    pub fn resolved_cases(&self) -> &CaseSet {
        &self.resolved_cases
    }

    pub fn precision(&self) -> EffectPrecision {
        self.precision
    }
}

/// Facts for a `perform` site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Facts for a class-constructor site with hidden init outward cases.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClassCtorSiteEffectFacts {
    emitted_cases: CaseSet,
}

impl ClassCtorSiteEffectFacts {
    pub fn new(emitted_cases: CaseSet) -> Self {
        Self { emitted_cases }
    }

    pub fn emitted_cases(&self) -> &CaseSet {
        &self.emitted_cases
    }
}

/// Facts for a `resume` site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Whether a nested `handle` can still suspend outward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NestedHandleClassification {
    SelfContained,
    MaySuspendOutward,
}

/// Facts for a single `handle` arm.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Structured contract for a `handle` site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Facts for one effect-relevant source/MIR site.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SiteEffectFacts {
    Call(CallSiteEffectFacts),
    ClassCtor(ClassCtorSiteEffectFacts),
    Perform(PerformSiteEffectFacts),
    Resume(ResumeSiteEffectFacts),
    Handle(HandleSiteEffectFacts),
}

/// Block-level effect summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Body-level effect facts keyed by stage-independent body-local IDs.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BodyEffectFacts {
    blocks: BTreeMap<BodyBlockId, BlockEffectFacts>,
    sites: BTreeMap<SiteId, SiteEffectFacts>,
    /// Owner step schema for plain bodies that still need local effect/control lowering.
    /// Must be present when any site in a Plain body can suspend through local control.
    local_control_step_schema: Option<StepSchemaId>,
}

impl BodyEffectFacts {
    pub fn new(
        blocks: BTreeMap<BodyBlockId, BlockEffectFacts>,
        sites: BTreeMap<SiteId, SiteEffectFacts>,
    ) -> Self {
        Self {
            blocks,
            sites,
            local_control_step_schema: None,
        }
    }

    pub fn with_local_control_step_schema(
        blocks: BTreeMap<BodyBlockId, BlockEffectFacts>,
        sites: BTreeMap<SiteId, SiteEffectFacts>,
        local_control_step_schema: Option<StepSchemaId>,
    ) -> Self {
        Self {
            blocks,
            sites,
            local_control_step_schema,
        }
    }

    pub fn blocks(&self) -> &BTreeMap<BodyBlockId, BlockEffectFacts> {
        &self.blocks
    }

    pub fn sites(&self) -> &BTreeMap<SiteId, SiteEffectFacts> {
        &self.sites
    }

    pub fn local_control_step_schema(&self) -> Option<StepSchemaId> {
        self.local_control_step_schema
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty() && self.sites.is_empty() && self.local_control_step_schema.is_none()
    }
}
