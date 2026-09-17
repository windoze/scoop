use crate::{
    CanonicalBooleanV1, DefaultBindingActionV1, DefaultBindingActionViewV1, DefaultBindingPlanV1,
    DefaultBindingProjectionViewV1, DefaultBindingShapeV1, DefaultBindingShapeViewV1,
    DefaultBindingTemporaryV1, DefaultFieldOperationKindV1, DefaultFieldRefV1,
    DefaultForIterationPlanV1, DefaultOperationEntityV1, DefaultOperationExpectedTypeShapeV1,
    DefaultOperationIntrinsicV1, DefaultOperationTypeRelationV1,
    DefaultOperationTypingSemanticAuthority, DefaultOperationValueRoleV1,
};

use super::{
    BodyNode, BodyValidator, BodyWork, DefaultBindingActionOperationV1,
    DefaultBindingShapeOperationV1, DefaultBodyOperationTypingProblemV1, DefaultBodyOperationV1,
    DefaultForOperationV1, ExportDefaultBodyOperationTypingValidationError, effect_is_ordinary,
};

impl<A, E> BodyValidator<'_, A, E>
where
    A: DefaultOperationTypingSemanticAuthority<E>,
{
    pub(super) fn process_for<'body>(
        &mut self,
        plan: &'body DefaultForIterationPlanV1,
        depth: u64,
        pending: &mut Vec<BodyWork<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let source_site = Self::site(
            DefaultBodyOperationV1::For(DefaultForOperationV1::Source),
            DefaultOperationValueRoleV1::Source,
        );
        let conformance_site = Self::site(
            DefaultBodyOperationV1::For(DefaultForOperationV1::Conformance),
            DefaultOperationValueRoleV1::Interface,
        );
        let next_site = Self::site(
            DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
            DefaultOperationValueRoleV1::Callable,
        );

        for temporary in [
            plan.source(),
            plan.conformance().source(),
            plan.conformance().iterator(),
            plan.next().result(),
            plan.next().element(),
        ] {
            self.validate_temporary(temporary, source_site)?;
        }

        self.expect_type(
            plan.source_init().result_type(),
            plan.source().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Source),
                DefaultOperationValueRoleV1::Initializer,
            ),
        )?;
        self.expect_type(
            plan.iterator_call().result_type(),
            plan.conformance().source().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::IteratorCall),
                DefaultOperationValueRoleV1::Result,
            ),
        )?;
        self.expect_type(
            plan.conformance().iterator().value_type(),
            plan.conformance().interface_type(),
            conformance_site,
        )?;
        self.expect_relation(
            DefaultOperationTypeRelationV1::IteratorConformance,
            plan.conformance().source().value_type(),
            plan.conformance().interface_type(),
            conformance_site,
        )?;

        let next_shape = self.callable_shape(
            DefaultOperationEntityV1::Callable(plan.next().callable()),
            next_site,
        )?;
        self.validate_callable_without_captures(&next_shape, next_site)?;
        let Some(receiver) = next_shape.receiver() else {
            return self.problem(
                Self::site(
                    DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                    DefaultOperationValueRoleV1::Receiver,
                ),
                DefaultBodyOperationTypingProblemV1::MissingCallableReceiver,
            );
        };
        self.expect_type(
            receiver,
            plan.conformance().iterator().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Receiver,
            ),
        )?;
        self.expect_arity(next_shape.parameters().len(), 0, next_site)?;
        if !effect_is_ordinary(next_shape.effect()) {
            return self.problem(
                next_site,
                DefaultBodyOperationTypingProblemV1::NonOrdinaryProtocolCallable,
            );
        }
        self.expect_type(
            next_shape.result(),
            plan.next().result().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Result,
            ),
        )?;

        let option = self.core_application(
            plan.next().result().value_type(),
            DefaultOperationExpectedTypeShapeV1::Option,
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Result,
            ),
        )?;
        self.expect_type(
            option.element(),
            plan.next().element().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Element,
            ),
        )?;

        let payload = self.variant_field_shape(
            DefaultOperationEntityV1::VariantField(plan.next().option().some_payload()),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Field { index: 0 },
            ),
        )?;
        self.expect_type(
            plan.next().option().some_payload().owner_type(),
            plan.next().result().value_type(),
            next_site,
        )?;
        self.expect_type(
            payload.owner_type(),
            plan.next().result().value_type(),
            next_site,
        )?;
        self.expect_type(
            payload.value_type(),
            plan.next().element().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Element,
            ),
        )?;

        let none = self.aggregate_shape(
            DefaultOperationEntityV1::Variant(plan.next().option().none()),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::Next),
                DefaultOperationValueRoleV1::Target,
            ),
        )?;
        self.expect_type(
            plan.next().option().none().owner_type(),
            plan.next().result().value_type(),
            next_site,
        )?;
        self.expect_type(
            none.owner_type(),
            plan.next().result().value_type(),
            next_site,
        )?;
        self.expect_arity(none.fields().len(), 0, next_site)?;
        self.validate_intrinsic(
            DefaultOperationIntrinsicV1::IteratorNext {
                conformance: plan.conformance(),
                next: plan.next(),
            },
            next_site,
        )?;

        self.expect_type(
            plan.binding().subject().value_type(),
            plan.next().element().value_type(),
            Self::site(
                DefaultBodyOperationV1::For(DefaultForOperationV1::BindingSubject),
                DefaultOperationValueRoleV1::Subject,
            ),
        )?;

        self.push_statements(pending, plan.body(), depth)?;
        self.push_node(pending, BodyNode::BindingPlan(plan.binding()), depth)?;
        self.push_node(pending, BodyNode::Expression(plan.iterator_call()), depth)?;
        self.push_statements(pending, plan.iterator_setup(), depth)?;
        self.push_node(pending, BodyNode::Expression(plan.source_init()), depth)?;
        self.push_statements(pending, plan.source_setup(), depth)
    }

    pub(super) fn process_binding_plan<'body>(
        &mut self,
        plan: &'body DefaultBindingPlanV1,
        depth: u64,
        pending: &mut Vec<BodyWork<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let site = Self::site(
            DefaultBodyOperationV1::For(DefaultForOperationV1::BindingSubject),
            DefaultOperationValueRoleV1::Subject,
        );
        self.validate_temporary(plan.subject(), site)?;
        self.push_node(
            pending,
            BodyNode::BindingShape {
                shape: plan.shape(),
                source: plan.subject(),
                actions: plan.actions(),
            },
            depth,
        )?;
        for (index, action) in plan.actions().iter().enumerate().rev() {
            self.push_node(
                pending,
                BodyNode::BindingAction {
                    action,
                    index,
                    actions: plan.actions(),
                },
                depth,
            )?;
        }
        Ok(())
    }

    pub(super) fn process_binding_action<'body>(
        &mut self,
        action: &'body DefaultBindingActionV1,
        index: usize,
        _actions: &'body [DefaultBindingActionV1],
        depth: u64,
        pending: &mut Vec<BodyWork<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        match action.view() {
            DefaultBindingActionViewV1::Project {
                source,
                result,
                projection,
                ..
            } => {
                let operation = DefaultBodyOperationV1::BindingAction {
                    index,
                    kind: DefaultBindingActionOperationV1::Project,
                };
                let source_site = Self::site(operation, DefaultOperationValueRoleV1::Source);
                let result_site = Self::site(operation, DefaultOperationValueRoleV1::Result);
                self.validate_temporary(source, source_site)?;
                self.validate_temporary(result, result_site)?;
                match projection.view() {
                    DefaultBindingProjectionViewV1::TupleIndex(field_index) => {
                        let scoop_identity::SignatureTypeKey::Tuple(elements) = source.value_type()
                        else {
                            return Err(
                                ExportDefaultBodyOperationTypingValidationError::TypeShape {
                                    site: source_site,
                                    expected: DefaultOperationExpectedTypeShapeV1::Tuple,
                                    actual: Box::new(source.value_type().clone()),
                                },
                            );
                        };
                        let Some(expected) = elements.as_slice().get(field_index as usize) else {
                            return self.problem(
                                Self::site(
                                    operation,
                                    DefaultOperationValueRoleV1::Field {
                                        index: field_index as usize,
                                    },
                                ),
                                DefaultBodyOperationTypingProblemV1::IndexOutOfBounds {
                                    index: field_index,
                                    length: elements.as_slice().len(),
                                },
                            );
                        };
                        self.expect_type(result.value_type(), expected, result_site)
                    }
                    DefaultBindingProjectionViewV1::StructField {
                        declaration,
                        owner_type,
                    } => {
                        let field = DefaultFieldRefV1::Struct {
                            declaration,
                            owner_type: owner_type.clone(),
                        };
                        let shape = self.field_shape(
                            DefaultOperationEntityV1::Field(&field),
                            Self::site(operation, DefaultOperationValueRoleV1::Target),
                        )?;
                        if shape.kind() != DefaultFieldOperationKindV1::Struct {
                            return self.problem(
                                Self::site(operation, DefaultOperationValueRoleV1::Target),
                                DefaultBodyOperationTypingProblemV1::FieldKindMismatch {
                                    expected: DefaultFieldOperationKindV1::Struct,
                                    actual: shape.kind(),
                                },
                            );
                        }
                        self.expect_type(source.value_type(), owner_type, source_site)?;
                        self.expect_type(shape.owner_type(), owner_type, source_site)?;
                        self.expect_type(result.value_type(), shape.value_type(), result_site)
                    }
                }
            }
            DefaultBindingActionViewV1::Component {
                source,
                index: component_index,
                result,
                setup,
                call,
                ..
            } => {
                let operation = DefaultBodyOperationV1::BindingAction {
                    index,
                    kind: DefaultBindingActionOperationV1::Component,
                };
                let source_site = Self::site(operation, DefaultOperationValueRoleV1::Source);
                let result_site = Self::site(operation, DefaultOperationValueRoleV1::Result);
                self.validate_temporary(source, source_site)?;
                self.validate_temporary(result, result_site)?;
                self.expect_type(call.result_type(), result.value_type(), result_site)?;
                self.validate_intrinsic(
                    DefaultOperationIntrinsicV1::BindingComponent {
                        source_type: source.value_type(),
                        index: component_index,
                        call,
                    },
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )?;
                self.push_node(pending, BodyNode::Expression(call), depth)?;
                self.push_statements(pending, setup, depth)
            }
            DefaultBindingActionViewV1::Bind { source, target, .. } => {
                let operation = DefaultBodyOperationV1::BindingAction {
                    index,
                    kind: DefaultBindingActionOperationV1::Bind,
                };
                let source_site = Self::site(operation, DefaultOperationValueRoleV1::Source);
                let target_site = Self::site(operation, DefaultOperationValueRoleV1::Target);
                self.validate_temporary(source, source_site)?;
                let record = self.local_record(target.local(), target_site)?;
                self.expect_type(record.value_type(), target.value_type(), target_site)?;
                self.expect_mutability(record.mutable(), target.mutable(), target_site)?;
                self.expect_mutability(target.mutable(), CanonicalBooleanV1::False, target_site)?;
                self.expect_type(source.value_type(), target.value_type(), target_site)
            }
        }
    }

    pub(super) fn process_binding_shape<'body>(
        &mut self,
        shape: &'body DefaultBindingShapeV1,
        source: &'body DefaultBindingTemporaryV1,
        actions: &'body [DefaultBindingActionV1],
        depth: u64,
        pending: &mut Vec<BodyWork<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        match shape.view() {
            DefaultBindingShapeViewV1::Binding(leaf) => {
                let operation =
                    DefaultBodyOperationV1::BindingShape(DefaultBindingShapeOperationV1::Binding);
                let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
                let record = self.local_record(leaf.local(), site)?;
                self.expect_type(record.value_type(), leaf.value_type(), site)?;
                self.expect_mutability(record.mutable(), leaf.mutable(), site)?;
                self.expect_mutability(leaf.mutable(), CanonicalBooleanV1::False, site)?;
                self.expect_type(source.value_type(), leaf.value_type(), site)
            }
            DefaultBindingShapeViewV1::Wildcard => Ok(()),
            DefaultBindingShapeViewV1::Tuple(elements) => {
                let operation =
                    DefaultBodyOperationV1::BindingShape(DefaultBindingShapeOperationV1::Tuple);
                let site = Self::site(operation, DefaultOperationValueRoleV1::Subject);
                let scoop_identity::SignatureTypeKey::Tuple(types) = source.value_type() else {
                    return Err(ExportDefaultBodyOperationTypingValidationError::TypeShape {
                        site,
                        expected: DefaultOperationExpectedTypeShapeV1::Tuple,
                        actual: Box::new(source.value_type().clone()),
                    });
                };
                self.expect_arity(elements.len(), types.as_slice().len(), site)?;
                for (index, shape) in elements.iter().enumerate().rev() {
                    if matches!(shape.view(), DefaultBindingShapeViewV1::Wildcard) {
                        continue;
                    }
                    let result = self.find_tuple_projection(actions, source, index as u32, site)?;
                    self.expect_type(
                        result.value_type(),
                        &types.as_slice()[index],
                        Self::site(operation, DefaultOperationValueRoleV1::Field { index }),
                    )?;
                    self.push_node(
                        pending,
                        BodyNode::BindingShape {
                            shape,
                            source: result,
                            actions,
                        },
                        depth,
                    )?;
                }
                Ok(())
            }
            DefaultBindingShapeViewV1::Struct { owner_type, fields } => {
                let operation =
                    DefaultBodyOperationV1::BindingShape(DefaultBindingShapeOperationV1::Struct);
                let site = Self::site(operation, DefaultOperationValueRoleV1::Subject);
                let aggregate =
                    self.aggregate_shape(DefaultOperationEntityV1::Struct(owner_type), site)?;
                self.expect_type(owner_type, aggregate.owner_type(), site)?;
                self.expect_type(source.value_type(), owner_type, site)?;
                self.expect_arity(fields.len(), aggregate.fields().len(), site)?;
                for (index, field) in fields.iter().enumerate().rev() {
                    let expected_index = index as u32;
                    if field.declaration_index() != expected_index {
                        return self.problem(
                            Self::site(operation, DefaultOperationValueRoleV1::Field { index }),
                            DefaultBodyOperationTypingProblemV1::NonCanonicalFieldIndex {
                                expected: expected_index,
                                actual: field.declaration_index(),
                            },
                        );
                    }
                    if matches!(field.shape().view(), DefaultBindingShapeViewV1::Wildcard) {
                        continue;
                    }
                    let result = self.find_struct_projection(
                        actions,
                        source,
                        owner_type,
                        expected_index,
                        site,
                    )?;
                    self.expect_type(
                        result.value_type(),
                        &aggregate.fields()[index],
                        Self::site(operation, DefaultOperationValueRoleV1::Field { index }),
                    )?;
                    self.push_node(
                        pending,
                        BodyNode::BindingShape {
                            shape: field.shape(),
                            source: result,
                            actions,
                        },
                        depth,
                    )?;
                }
                Ok(())
            }
            DefaultBindingShapeViewV1::Class {
                owner_type,
                components,
            } => {
                let operation =
                    DefaultBodyOperationV1::BindingShape(DefaultBindingShapeOperationV1::Class);
                let site = Self::site(operation, DefaultOperationValueRoleV1::Subject);
                let canonical =
                    self.type_shape(DefaultOperationEntityV1::Class(owner_type), site)?;
                self.expect_type(owner_type, &canonical, site)?;
                self.expect_type(source.value_type(), owner_type, site)?;
                for (position, component) in components.iter().enumerate().rev() {
                    let expected = u32::try_from(position + 1).map_err(|_| {
                        ExportDefaultBodyOperationTypingValidationError::Resource(
                            scoop_wire::WireError::new(
                                scoop_wire::WireErrorKind::IntegerOutOfRange,
                                self.path.clone(),
                                None,
                            ),
                        )
                    })?;
                    if component.index().get() != expected {
                        return self.problem(
                            Self::site(
                                operation,
                                DefaultOperationValueRoleV1::Part { index: position },
                            ),
                            DefaultBodyOperationTypingProblemV1::NonCanonicalFieldIndex {
                                expected,
                                actual: component.index().get(),
                            },
                        );
                    }
                    let result = self.find_component(actions, source, component.index(), site)?;
                    self.push_node(
                        pending,
                        BodyNode::BindingShape {
                            shape: component.shape(),
                            source: result,
                            actions,
                        },
                        depth,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn find_tuple_projection<'body>(
        &mut self,
        actions: &'body [DefaultBindingActionV1],
        source: &DefaultBindingTemporaryV1,
        expected_index: u32,
        site: super::DefaultBodyOperationTypingSiteV1,
    ) -> Result<&'body DefaultBindingTemporaryV1, ExportDefaultBodyOperationTypingValidationError<E>>
    {
        self.find_action_result(actions, site, |action| {
            matches!(
                action.view(),
                DefaultBindingActionViewV1::Project {
                    source: actual_source,
                    projection,
                    ..
                } if actual_source == source
                    && matches!(
                        projection.view(),
                        DefaultBindingProjectionViewV1::TupleIndex(actual)
                            if actual == expected_index
                    )
            )
        })
    }

    fn find_struct_projection<'body>(
        &mut self,
        actions: &'body [DefaultBindingActionV1],
        source: &DefaultBindingTemporaryV1,
        owner_type: &scoop_identity::SignatureTypeKey,
        declaration_index: u32,
        site: super::DefaultBodyOperationTypingSiteV1,
    ) -> Result<&'body DefaultBindingTemporaryV1, ExportDefaultBodyOperationTypingValidationError<E>>
    {
        let mut found = None;
        for action in actions {
            self.charge_work()?;
            let DefaultBindingActionViewV1::Project {
                source: actual_source,
                result,
                projection,
                ..
            } = action.view()
            else {
                continue;
            };
            if actual_source != source {
                continue;
            }
            let DefaultBindingProjectionViewV1::StructField {
                declaration,
                owner_type: actual_owner,
            } = projection.view()
            else {
                continue;
            };
            if actual_owner != owner_type {
                continue;
            }
            let field = DefaultFieldRefV1::Struct {
                declaration,
                owner_type: actual_owner.clone(),
            };
            let shape = self.field_shape(DefaultOperationEntityV1::Field(&field), site)?;
            if shape.kind() != DefaultFieldOperationKindV1::Struct
                || shape.owner_type() != owner_type
                || shape.declaration_index() != declaration_index
            {
                continue;
            }
            if found.replace(result).is_some() {
                return self.problem(
                    site,
                    DefaultBodyOperationTypingProblemV1::MultipleBindingActions,
                );
            }
        }
        found.ok_or(ExportDefaultBodyOperationTypingValidationError::Problem {
            site,
            problem: DefaultBodyOperationTypingProblemV1::MissingBindingAction,
        })
    }

    fn find_component<'body>(
        &mut self,
        actions: &'body [DefaultBindingActionV1],
        source: &DefaultBindingTemporaryV1,
        expected_index: std::num::NonZeroU32,
        site: super::DefaultBodyOperationTypingSiteV1,
    ) -> Result<&'body DefaultBindingTemporaryV1, ExportDefaultBodyOperationTypingValidationError<E>>
    {
        self.find_action_result(actions, site, |action| {
            matches!(
                action.view(),
                DefaultBindingActionViewV1::Component {
                    source: actual_source,
                    index,
                    ..
                } if actual_source == source && index == expected_index
            )
        })
    }

    fn find_action_result<'body>(
        &mut self,
        actions: &'body [DefaultBindingActionV1],
        site: super::DefaultBodyOperationTypingSiteV1,
        mut predicate: impl FnMut(&DefaultBindingActionV1) -> bool,
    ) -> Result<&'body DefaultBindingTemporaryV1, ExportDefaultBodyOperationTypingValidationError<E>>
    {
        let mut found = None;
        for action in actions {
            self.charge_work()?;
            if !predicate(action) {
                continue;
            }
            let result = match action.view() {
                DefaultBindingActionViewV1::Project { result, .. }
                | DefaultBindingActionViewV1::Component { result, .. } => result,
                DefaultBindingActionViewV1::Bind { .. } => continue,
            };
            if found.replace(result).is_some() {
                return self.problem(
                    site,
                    DefaultBodyOperationTypingProblemV1::MultipleBindingActions,
                );
            }
        }
        found.ok_or(ExportDefaultBodyOperationTypingValidationError::Problem {
            site,
            problem: DefaultBodyOperationTypingProblemV1::MissingBindingAction,
        })
    }
}
