use crate::effect_facts::{CaseTag, ImplPlan, StepSchemaId};
use crate::mir::InstanceKey;

/// P5 late-lowering 阶段的顶层中间表示。
///
/// P5-T01 先固定一个独立、稳定的容器边界；P5-T02 及之后的任务会继续把 version key、state
/// graph、frame schema、boundary/resume mapping 等最终形状补进来，而不是再把这些信息散落回
/// P3/P4 的 direct-style MIR 或 effect facts side table。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredProgram {
    callables: Vec<LateLoweredCallable>,
}

impl LateLoweredProgram {
    pub(crate) fn new(callables: Vec<LateLoweredCallable>) -> Self {
        Self { callables }
    }

    pub fn callables(&self) -> &[LateLoweredCallable] {
        &self.callables
    }

    pub fn callable(&self, root_fqn: &str) -> Option<&LateLoweredCallable> {
        self.callables
            .iter()
            .find(|callable| callable.root_fqn() == root_fqn)
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

/// 单个 callable family 在 late-lowering 入口处对应的稳定边界记录。
///
/// 当前先把 P4 已经确定的 callable-level contract 显式挂到独立 IR 上，后续任务会在同一类型上
/// 继续扩展 version/state/frame 等结构，而不是另起第二套容器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateLoweredCallable {
    root_fqn: String,
    instance_key: InstanceKey,
    step_schema: StepSchemaId,
    impl_plan: ImplPlan,
    needs_reentry: bool,
    resolved_outward_cases: Vec<CaseTag>,
}

impl LateLoweredCallable {
    pub(crate) fn new(
        root_fqn: String,
        instance_key: InstanceKey,
        step_schema: StepSchemaId,
        impl_plan: ImplPlan,
        needs_reentry: bool,
        resolved_outward_cases: Vec<CaseTag>,
    ) -> Self {
        Self {
            root_fqn,
            instance_key,
            step_schema,
            impl_plan,
            needs_reentry,
            resolved_outward_cases,
        }
    }

    pub fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub fn instance_key(&self) -> &InstanceKey {
        &self.instance_key
    }

    pub fn step_schema(&self) -> StepSchemaId {
        self.step_schema
    }

    pub fn impl_plan(&self) -> ImplPlan {
        self.impl_plan
    }

    pub fn needs_reentry(&self) -> bool {
        self.needs_reentry
    }

    pub fn resolved_outward_cases(&self) -> &[CaseTag] {
        &self.resolved_outward_cases
    }
}
