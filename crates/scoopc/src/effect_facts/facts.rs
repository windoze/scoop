use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::mir::{InstanceKey, MaterializedMirPassView};
use crate::ty::{EffectRow, TypeId};

use super::schema::{
    CaseSet, ContinuationSchema, ContinuationSchemaId, ImplPlan, StepSchema, StepSchemaId,
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
/// `resolved_outward_cases` / `needs_reentry` / `impl_plan` 当前仍由 builder 以保守值初始化；
/// `resolved_outward_cases` 的 SCC/dataflow 精化在 P4-T04 完成。
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

/// body-level facts 外壳；最终 block/site 结构在 P4-T03/P4-T04 补齐。
#[derive(Debug, Clone, Default)]
pub struct BodyEffectFacts {}

/// refactor 主线的 authoritative effect-facts 容器。
///
/// 生命周期规则：
/// - 与当前 canonical materialized MIR snapshot 一一对应；
/// - 结构性 rewrite 后必须基于新的 snapshot 重建；
/// - 不对外暴露“部分 body 已更新、部分 body 仍过期”的混合状态。
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

    pub fn stable_dump(&self) -> String {
        super::dump::render_materialized_effect_facts(self)
    }
}
