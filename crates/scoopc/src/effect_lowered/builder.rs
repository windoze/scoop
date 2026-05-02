use crate::effect_facts::MaterializedEffectFacts;
use crate::mir::MaterializedMirPassView;

use super::{EffectLoweringError, LateLoweredCallable, LateLoweredProgram};

/// 把 canonical MIR snapshot + P4 facts 组装成独立 `LateLoweredProgram` 的统一入口。
pub(crate) struct LateLoweredProgramBuilder<'a> {
    pass_view: MaterializedMirPassView<'a>,
    effect_facts: &'a MaterializedEffectFacts,
}

impl<'a> LateLoweredProgramBuilder<'a> {
    pub(crate) fn from_canonical_inputs(
        pass_view: MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
    ) -> Self {
        Self {
            pass_view,
            effect_facts,
        }
    }

    pub(crate) fn build(self) -> Result<LateLoweredProgram, EffectLoweringError> {
        let pass_view = self.pass_view;
        let effect_facts = self.effect_facts;
        let snapshot_instances = pass_view.len();
        let callable_facts_count = effect_facts.callable_facts().len();

        if snapshot_instances != callable_facts_count {
            return Err(EffectLoweringError::SnapshotCallableCountMismatch {
                snapshot_instances,
                callable_facts: callable_facts_count,
            });
        }

        let mut callables = Vec::with_capacity(snapshot_instances);
        for family in pass_view.instances() {
            let root_fqn = family.root_fqn().to_string();
            let callable_facts =
                effect_facts
                    .callable_facts()
                    .get(family.key())
                    .ok_or_else(|| EffectLoweringError::MissingCallableFacts {
                        root_fqn: root_fqn.clone(),
                    })?;

            if !effect_facts
                .step_schemas()
                .contains_key(&callable_facts.step_schema())
            {
                return Err(EffectLoweringError::MissingStepSchema {
                    root_fqn,
                    step_schema: callable_facts.step_schema().as_u32(),
                });
            }

            callables.push(LateLoweredCallable::new(
                family.root_fqn().to_string(),
                family.key().clone(),
                callable_facts.step_schema(),
                callable_facts.impl_plan(),
                callable_facts.needs_reentry(),
                callable_facts.resolved_outward_cases().tags().to_vec(),
            ));
        }

        Ok(LateLoweredProgram::new(callables))
    }
}
