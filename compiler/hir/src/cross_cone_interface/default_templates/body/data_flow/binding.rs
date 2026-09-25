use crate::{
    CanonicalBooleanV1, DefaultBindingActionV1, DefaultBindingActionViewV1, DefaultBindingLeafV1,
    DefaultBindingPlanV1, DefaultBindingProjectionViewV1, DefaultBindingShapeV1,
    DefaultBindingShapeViewV1, DefaultBindingTemporaryV1, DefaultForIterationPlanV1,
};

use super::{
    DefaultBindingActionLocalRoleV1, DefaultBindingShapeActionKindV1,
    DefaultBindingShapeDataFlowValidationError, DefaultForTemporaryRoleV1,
    DefaultLocalDataFlowLocalError, DefaultLocalDataFlowSiteV1, DefaultLoopControlV1,
    DefinitionOwner, ExportDefaultLocalDataFlowValidationError, Flow, Region, Validator,
};

impl<A, E> Validator<'_, A, E>
where
    A: super::DefaultBodyDataFlowAuthority<E>,
{
    pub(super) fn validate_for(
        &mut self,
        plan: &DefaultForIterationPlanV1,
        available: Vec<bool>,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        let plan_owner = self.next_plan_owner()?;
        let reserved = [
            (plan.source(), DefaultForTemporaryRoleV1::Source),
            (
                plan.conformance().source(),
                DefaultForTemporaryRoleV1::ConformanceSource,
            ),
            (
                plan.conformance().iterator(),
                DefaultForTemporaryRoleV1::Iterator,
            ),
            (plan.next().result(), DefaultForTemporaryRoleV1::NextResult),
            (
                plan.next().element(),
                DefaultForTemporaryRoleV1::NextElement,
            ),
        ];
        self.validate_reserved_temporaries(&reserved, plan_owner)?;
        self.claim_action_outputs(plan.binding(), plan_owner)?;
        if plan.binding().subject() != plan.next().element() {
            return Err(ExportDefaultLocalDataFlowValidationError::BindingSubject {
                expected: Box::new(plan.next().element().clone()),
                actual: Box::new(plan.binding().subject().clone()),
            });
        }
        self.check_temporary(
            plan.binding().subject(),
            DefaultLocalDataFlowSiteV1::BindingSubject,
        )?;

        let incoming = self.copy_bits(&available)?;
        let prefix = Region {
            owner: plan_owner,

            ..region
        };
        let source = self.validate_statements(
            plan.source_setup(),
            Flow::falling_through(available),
            reachable,
            prefix,
        )?;
        self.validate_expression(
            plan.source_init(),
            &source.available,
            reachable && source.falls_through,
        )?;
        let mut source_available = source.available;
        self.define_temporary(
            plan.source(),
            DefaultLocalDataFlowSiteV1::ForTemporary(DefaultForTemporaryRoleV1::Source),
            &mut source_available,
            plan_owner,
        )?;

        let iterator = self.validate_statements(
            plan.iterator_setup(),
            Flow::falling_through(source_available),
            reachable && source.falls_through,
            prefix,
        )?;
        self.validate_expression(
            plan.iterator_call(),
            &iterator.available,
            reachable && source.falls_through && iterator.falls_through,
        )?;
        let mut iteration_available = iterator.available;
        for (temporary, role) in reserved.iter().copied().skip(1) {
            self.define_temporary(
                temporary,
                DefaultLocalDataFlowSiteV1::ForTemporary(role),
                &mut iteration_available,
                plan_owner,
            )?;
        }

        let reaches_iteration = reachable && source.falls_through && iterator.falls_through;
        let loop_depth = region.loop_depth.checked_add(1).ok_or_else(|| {
            ExportDefaultLocalDataFlowValidationError::Resource(scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })?;
        let action_region = Region {
            owner: plan_owner,
            loop_depth,
        };
        let mut successful = Flow::falling_through(iteration_available);
        self.validate_binding_actions(
            plan.binding().actions(),
            &mut successful,
            reaches_iteration,
            action_region,
        )?;
        self.validate_binding_shape(plan.binding())?;

        let body_reachable = reaches_iteration && successful.falls_through;
        let body = self.validate_statements(
            plan.body(),
            Flow::falling_through(successful.available),
            body_reachable,
            Region {
                owner: DefinitionOwner::Ordinary,
                ..action_region
            },
        )?;
        if body_reachable {
            self.merge_abrupt_outcomes(&mut successful.abrupt, body.abrupt)?;
            successful.falls_through = body.falls_through;
        }

        let mut abrupt = source.abrupt;
        if source.falls_through {
            self.merge_abrupt_outcomes(&mut abrupt, iterator.abrupt)?;
        }
        if reaches_iteration {
            successful.abrupt.take(DefaultLoopControlV1::Break);
            successful.abrupt.take(DefaultLoopControlV1::Continue);
            self.merge_abrupt_outcomes(&mut abrupt, successful.abrupt)?;
        }
        Ok(Flow {
            available: incoming,
            falls_through: reaches_iteration,
            abrupt,
        })
    }

    fn validate_reserved_temporaries(
        &mut self,
        reserved: &[(&DefaultBindingTemporaryV1, DefaultForTemporaryRoleV1)],
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for (index, (temporary, role)) in reserved.iter().copied().enumerate() {
            let site = DefaultLocalDataFlowSiteV1::ForTemporary(role);
            self.claim_local(
                temporary.local(),
                Some(temporary.value_type()),
                Some(CanonicalBooleanV1::False),
                site,
                owner,
            )?;
            for (previous, previous_role) in reserved[..index].iter().copied() {
                if previous.local() == temporary.local() {
                    return Err(
                        ExportDefaultLocalDataFlowValidationError::ForTemporaryAlias {
                            first: previous_role,
                            second: role,
                            selector: Box::new(temporary.local().clone()),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn claim_action_outputs(
        &mut self,
        plan: &DefaultBindingPlanV1,
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for (index, action) in plan.actions().iter().enumerate() {
            match action.view() {
                DefaultBindingActionViewV1::Project { result, .. }
                | DefaultBindingActionViewV1::Component { result, .. } => {
                    self.claim_local(
                        result.local(),
                        Some(result.value_type()),
                        Some(CanonicalBooleanV1::False),
                        DefaultLocalDataFlowSiteV1::BindingAction {
                            index,
                            role: DefaultBindingActionLocalRoleV1::Result,
                        },
                        owner,
                    )?;
                }
                DefaultBindingActionViewV1::Bind { target, .. } => {
                    self.claim_binding_leaf(target, index, owner)?;
                }
            }
        }
        Ok(())
    }

    fn claim_binding_leaf(
        &mut self,
        leaf: &DefaultBindingLeafV1,
        index: usize,
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let site = DefaultLocalDataFlowSiteV1::BindingAction {
            index,
            role: DefaultBindingActionLocalRoleV1::Target,
        };
        let local_index = self.claim_local(
            leaf.local(),
            Some(leaf.value_type()),
            Some(leaf.mutable()),
            site,
            owner,
        )?;
        if leaf.mutable() != CanonicalBooleanV1::False {
            return Err(self.local_error(
                site,
                self.local_record(local_index).selector(),
                DefaultLocalDataFlowLocalError::Mutability {
                    expected: CanonicalBooleanV1::False,
                    actual: leaf.mutable(),
                },
            ));
        }
        Ok(())
    }

    fn validate_binding_actions(
        &mut self,
        actions: &[DefaultBindingActionV1],
        flow: &mut Flow,
        reachable: bool,
        region: Region,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for (index, action) in actions.iter().enumerate() {
            let action_reachable = reachable && flow.falls_through;
            match action.view() {
                DefaultBindingActionViewV1::Project { source, result, .. } => {
                    self.use_temporary(source, index, &flow.available, action_reachable)?;
                    self.define_temporary(
                        result,
                        DefaultLocalDataFlowSiteV1::BindingAction {
                            index,
                            role: DefaultBindingActionLocalRoleV1::Result,
                        },
                        &mut flow.available,
                        region.owner,
                    )?;
                }
                DefaultBindingActionViewV1::Component {
                    source,
                    result,
                    setup,
                    call,
                    ..
                } => {
                    self.use_temporary(source, index, &flow.available, action_reachable)?;
                    let setup = self.validate_statements(
                        setup,
                        Flow::falling_through(std::mem::take(&mut flow.available)),
                        action_reachable,
                        Region { ..region },
                    )?;
                    self.validate_expression(
                        call,
                        &setup.available,
                        action_reachable && setup.falls_through,
                    )?;
                    if action_reachable {
                        self.merge_abrupt_outcomes(&mut flow.abrupt, setup.abrupt)?;
                        flow.falls_through = setup.falls_through;
                    }
                    flow.available = setup.available;
                    self.define_temporary(
                        result,
                        DefaultLocalDataFlowSiteV1::BindingAction {
                            index,
                            role: DefaultBindingActionLocalRoleV1::Result,
                        },
                        &mut flow.available,
                        region.owner,
                    )?;
                }
                DefaultBindingActionViewV1::Bind { source, target, .. } => {
                    self.use_temporary(source, index, &flow.available, action_reachable)?;
                    self.define_local(
                        target.local(),
                        Some(target.value_type()),
                        Some(target.mutable()),
                        DefaultLocalDataFlowSiteV1::BindingAction {
                            index,
                            role: DefaultBindingActionLocalRoleV1::Target,
                        },
                        &mut flow.available,
                        region.owner,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn check_temporary(
        &mut self,
        temporary: &DefaultBindingTemporaryV1,
        site: DefaultLocalDataFlowSiteV1,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError<E>> {
        let index = self.local_index(temporary.local(), site)?;
        self.check_local_shape(
            index,
            Some(temporary.value_type()),
            Some(CanonicalBooleanV1::False),
            site,
        )?;
        Ok(index)
    }

    fn use_temporary(
        &mut self,
        temporary: &DefaultBindingTemporaryV1,
        action_index: usize,
        available: &[bool],
        reachable: bool,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let site = DefaultLocalDataFlowSiteV1::BindingAction {
            index: action_index,
            role: DefaultBindingActionLocalRoleV1::Source,
        };
        let index = self.use_local(
            temporary.local(),
            Some(temporary.value_type()),
            site,
            available,
            reachable,
        )?;
        self.check_local_shape(index, None, Some(CanonicalBooleanV1::False), site)
    }

    fn define_temporary(
        &mut self,
        temporary: &DefaultBindingTemporaryV1,
        site: DefaultLocalDataFlowSiteV1,
        available: &mut [bool],
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        self.define_local(
            temporary.local(),
            Some(temporary.value_type()),
            Some(CanonicalBooleanV1::False),
            site,
            available,
            owner,
        )?;
        Ok(())
    }

    fn validate_binding_shape(
        &mut self,
        plan: &DefaultBindingPlanV1,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let actions = plan.actions();
        let mut struct_indices = Vec::new();
        scoop_wire::allocation::try_reserve(&mut struct_indices, actions.len(), self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        for (action_index, action) in actions.iter().enumerate() {
            let index = match action.view() {
                DefaultBindingActionViewV1::Project { projection, .. } => match projection.view() {
                    DefaultBindingProjectionViewV1::StructField {
                        declaration,
                        owner_type,
                    } => Some(
                        self.authority
                            .default_binding_struct_field_index(declaration, owner_type)
                            .map_err(|error| {
                                ExportDefaultLocalDataFlowValidationError::BindingShape(Box::new(
                                    DefaultBindingShapeDataFlowValidationError::StructProjection {
                                        action_index,
                                        error: Box::new(error),
                                    },
                                ))
                            })?,
                    ),
                    DefaultBindingProjectionViewV1::TupleIndex(_) => None,
                },
                DefaultBindingActionViewV1::Component { .. }
                | DefaultBindingActionViewV1::Bind { .. } => None,
            };
            struct_indices.push(index);
        }

        let mut consumed = Vec::new();
        scoop_wire::allocation::try_reserve(&mut consumed, actions.len(), self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        consumed.resize(actions.len(), false);

        let mut pending = Vec::new();
        scoop_wire::allocation::try_reserve(&mut pending, 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;

        pending.push(ShapeWork {
            shape: plan.shape(),
            source: plan.subject(),
        });

        while let Some(work) = pending.pop() {
            match work.shape.view() {
                DefaultBindingShapeViewV1::Binding(leaf) => {
                    let index = self.find_action(
                        actions,
                        DefaultBindingShapeActionKindV1::Bind,
                        |_, action| {
                            matches!(
                                action.view(),
                                DefaultBindingActionViewV1::Bind { source, target, .. }
                                    if source == work.source && target == leaf
                            )
                        },
                    )?;
                    self.consume_action(&mut consumed, index)?;
                }
                DefaultBindingShapeViewV1::Wildcard => {}
                DefaultBindingShapeViewV1::Tuple(elements) => {
                    for (index, shape) in elements.iter().enumerate().rev() {
                        if matches!(shape.view(), DefaultBindingShapeViewV1::Wildcard) {
                            continue;
                        }
                        let declaration_index = u32::try_from(index).map_err(|_| {
                            ExportDefaultLocalDataFlowValidationError::Resource(
                                scoop_wire::WireError::new(
                                    scoop_wire::WireErrorKind::IntegerOutOfRange,
                                    self.path.clone(),
                                    None,
                                ),
                            )
                        })?;
                        let action_index = self.find_action(
                            actions,
                            DefaultBindingShapeActionKindV1::TupleProjection,
                            |_, action| {
                                matches!(
                                    action.view(),
                                    DefaultBindingActionViewV1::Project {
                                        source,
                                        projection,
                                        ..
                                    } if source == work.source
                                        && matches!(
                                            projection.view(),
                                            DefaultBindingProjectionViewV1::TupleIndex(actual)
                                                if actual == declaration_index
                                        )
                                )
                            },
                        )?;
                        self.consume_action(&mut consumed, action_index)?;
                        let result = action_result(&actions[action_index]).ok_or_else(|| {
                            ExportDefaultLocalDataFlowValidationError::BindingShape(Box::new(
                                DefaultBindingShapeDataFlowValidationError::ActionKind {
                                    index: action_index,
                                    expected: DefaultBindingShapeActionKindV1::TupleProjection,
                                },
                            ))
                        })?;
                        self.push_shape(&mut pending, shape, result)?;
                    }
                }
                DefaultBindingShapeViewV1::Struct { owner_type, fields } => {
                    for field in fields.iter().rev() {
                        let expected = field.declaration_index();
                        if matches!(field.shape().view(), DefaultBindingShapeViewV1::Wildcard) {
                            continue;
                        }
                        let action_index = self.find_action(
                            actions,
                            DefaultBindingShapeActionKindV1::StructProjection,
                            |index, action| {
                                matches!(
                                    action.view(),
                                    DefaultBindingActionViewV1::Project {
                                        source,
                                        projection,
                                        ..
                                    }
                                        if source == work.source
                                            && struct_indices[index] == Some(expected)
                                            && matches!(
                                                projection.view(),
                                                DefaultBindingProjectionViewV1::StructField {
                                                    owner_type: projection_owner,
                                                    ..
                                                } if projection_owner == owner_type
                                            )
                                )
                            },
                        )?;
                        self.consume_action(&mut consumed, action_index)?;
                        let result = action_result(&actions[action_index]).ok_or_else(|| {
                            ExportDefaultLocalDataFlowValidationError::BindingShape(Box::new(
                                DefaultBindingShapeDataFlowValidationError::ActionKind {
                                    index: action_index,
                                    expected: DefaultBindingShapeActionKindV1::StructProjection,
                                },
                            ))
                        })?;
                        self.push_shape(&mut pending, field.shape(), result)?;
                    }
                }
                DefaultBindingShapeViewV1::Class { components, .. } => {
                    for (position, component) in components.iter().enumerate().rev() {
                        let expected = u32::try_from(position + 1).map_err(|_| {
                            ExportDefaultLocalDataFlowValidationError::Resource(
                                scoop_wire::WireError::new(
                                    scoop_wire::WireErrorKind::IntegerOutOfRange,
                                    self.path.clone(),
                                    None,
                                ),
                            )
                        })?;
                        if component.index().get() != expected {
                            return Err(ExportDefaultLocalDataFlowValidationError::BindingShape(
                                Box::new(
                                    DefaultBindingShapeDataFlowValidationError::ClassComponentOrder {
                                        position,
                                        expected,
                                        actual: component.index().get(),
                                    },
                                ),
                            ));
                        }
                        let action_index = self.find_action(
                            actions,
                            DefaultBindingShapeActionKindV1::Component,
                            |_, action| {
                                matches!(
                                    action.view(),
                                    DefaultBindingActionViewV1::Component {
                                        source,
                                        index,
                                        ..
                                    } if source == work.source && index == component.index()
                                )
                            },
                        )?;
                        self.consume_action(&mut consumed, action_index)?;
                        let result = action_result(&actions[action_index]).ok_or_else(|| {
                            ExportDefaultLocalDataFlowValidationError::BindingShape(Box::new(
                                DefaultBindingShapeDataFlowValidationError::ActionKind {
                                    index: action_index,
                                    expected: DefaultBindingShapeActionKindV1::Component,
                                },
                            ))
                        })?;
                        self.push_shape(&mut pending, component.shape(), result)?;
                    }
                }
            }
        }
        for (index, consumed) in consumed.into_iter().enumerate() {
            if !consumed {
                return Err(ExportDefaultLocalDataFlowValidationError::BindingShape(
                    Box::new(DefaultBindingShapeDataFlowValidationError::ExtraAction { index }),
                ));
            }
        }
        Ok(())
    }

    fn find_action(
        &mut self,
        actions: &[DefaultBindingActionV1],
        kind: DefaultBindingShapeActionKindV1,
        mut predicate: impl FnMut(usize, &DefaultBindingActionV1) -> bool,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut found = None;
        for (index, action) in actions.iter().enumerate() {
            if !predicate(index, action) {
                continue;
            }
            if found.is_some() {
                return Err(ExportDefaultLocalDataFlowValidationError::BindingShape(
                    Box::new(DefaultBindingShapeDataFlowValidationError::MultipleActions { kind }),
                ));
            }
            found = Some(index);
        }
        found.ok_or_else(|| {
            ExportDefaultLocalDataFlowValidationError::BindingShape(Box::new(
                DefaultBindingShapeDataFlowValidationError::MissingAction { kind },
            ))
        })
    }

    fn consume_action(
        &mut self,
        consumed: &mut [bool],
        index: usize,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        if std::mem::replace(&mut consumed[index], true) {
            Err(ExportDefaultLocalDataFlowValidationError::BindingShape(
                Box::new(DefaultBindingShapeDataFlowValidationError::ReusedAction { index }),
            ))
        } else {
            Ok(())
        }
    }

    fn push_shape<'body>(
        &mut self,
        pending: &mut Vec<ShapeWork<'body>>,
        shape: &'body DefaultBindingShapeV1,
        source: &'body DefaultBindingTemporaryV1,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        scoop_wire::allocation::try_reserve(pending, 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;

        pending.push(ShapeWork { shape, source });
        Ok(())
    }
}

fn action_result(action: &DefaultBindingActionV1) -> Option<&DefaultBindingTemporaryV1> {
    match action.view() {
        DefaultBindingActionViewV1::Project { result, .. }
        | DefaultBindingActionViewV1::Component { result, .. } => Some(result),
        DefaultBindingActionViewV1::Bind { .. } => None,
    }
}

struct ShapeWork<'a> {
    shape: &'a DefaultBindingShapeV1,
    source: &'a DefaultBindingTemporaryV1,
}
