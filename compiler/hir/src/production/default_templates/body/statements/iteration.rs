//! Projection of the complete source `for` protocol plan.

use crate::{
    DefaultAppliedOptionV1, DefaultForIterationPlanV1, DefaultIteratorConformanceV1,
    DefaultIteratorNextV1, ForIterationPlan,
};

use super::super::BodyProjection;

impl BodyProjection<'_, '_, '_, '_> {
    pub(super) fn for_iteration(
        &mut self,
        plan: &ForIterationPlan,
    ) -> Result<DefaultForIterationPlanV1, super::super::super::DefaultBodyProjectionError> {
        let source_setup = self.statements(plan.source_setup())?;
        let source = self.binding_temporary(plan.source())?;
        let source_init = self.expression(plan.source_init())?;
        let iterator_setup = self.statements(plan.iterator_setup())?;
        let iterator_call = self.expression(plan.iterator_call())?;

        let conformance = plan.conformance();
        let application = super::super::super::arena_get(
            &self.entities.export().interface_applications,
            conformance.application(),
        )
        .ok_or(super::super::super::DefaultEntityProjectionError::Unknown {
            kind: "iterator interface application",
            index: super::super::super::raw_index(conformance.application()),
        })?;
        let conformance = DefaultIteratorConformanceV1::new(
            self.binding_temporary(conformance.source())?,
            self.binding_temporary(conformance.iterator())?,
            self.type_key(application.canonical_type)?,
            self.origin(conformance.origin().definition())?,
        );

        let next = plan.next();
        let option = next.option();
        let next = DefaultIteratorNextV1::new(
            self.entities
                .callable(crate::Callable::Method(next.callable()), self.binders)?,
            self.binding_temporary(next.result())?,
            DefaultAppliedOptionV1::new(
                self.entities
                    .variant_field(option.some_payload(), self.binders)?,
                self.entities.variant(option.none(), self.binders)?,
            ),
            self.binding_temporary(next.element())?,
            self.origin(next.origin().definition())?,
        );

        self.entities.resources.collection::<crate::LoopId>(1)?;
        self.loops.push(plan.target());
        let projected: Result<_, super::super::super::DefaultBodyProjectionError> = (|| {
            Ok((
                self.binding_plan(plan.binding())?,
                self.statements(plan.body())?,
            ))
        })();
        self.loops.pop();
        let (binding, body) = projected?;

        DefaultForIterationPlanV1::try_new(
            source_setup,
            source,
            source_init,
            iterator_setup,
            iterator_call,
            conformance,
            next,
            binding,
            body,
        )
        .map_err(super::super::super::DefaultBodyProjectionError::ForPlan)
    }
}
