use super::super::super::{
    ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1, FieldTargetView,
};
use super::super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork};
use crate::{
    DefaultAppliedOptionV1, DefaultBindingActionV1, DefaultBindingActionViewV1,
    DefaultBindingPlanV1, DefaultBindingProjectionV1, DefaultBindingProjectionViewV1,
    DefaultBindingShapeV1, DefaultBindingShapeViewV1, DefaultBodyProviderTypeSiteV1,
    DefaultForIterationPlanV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    ExportDefinitionSourceV1,
};

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(in super::super) fn process_for(
        &mut self,
        plan: &'body DefaultForIterationPlanV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_statements(pending, plan.body())?;
        self.push_child(
            pending,
            BodyNode::BindingPlan {
                plan: plan.binding(),
                origin,
            },
        )?;
        self.push_child(pending, BodyNode::IteratorNext(plan.next()))?;
        self.push_child(pending, BodyNode::IteratorConformance(plan.conformance()))?;
        self.push_child(pending, BodyNode::Expression(plan.iterator_call()))?;
        self.push_statements(pending, plan.iterator_setup())?;
        self.push_child(pending, BodyNode::Expression(plan.source_init()))?;
        self.push_child(
            pending,
            BodyNode::BindingTemporary {
                value_type: plan.source().value_type(),
                origin,
            },
        )?;
        self.push_statements(pending, plan.source_setup())
    }

    pub(in super::super) fn process_binding_plan(
        &mut self,
        plan: &'body DefaultBindingPlanV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        for action in plan.actions().iter().rev() {
            self.push_child(pending, BodyNode::BindingAction(action))?;
        }
        self.push_child(
            pending,
            BodyNode::BindingShape {
                shape: plan.shape(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            BodyNode::BindingTemporary {
                value_type: plan.subject().value_type(),
                origin,
            },
        )
    }

    pub(in super::super) fn process_binding_action(
        &mut self,
        action: &'body DefaultBindingActionV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match action.view() {
            DefaultBindingActionViewV1::Project {
                source,
                result,
                projection,
                definition_origin,
            } => {
                self.push_child(
                    pending,
                    BodyNode::BindingProjection {
                        projection,
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    BodyNode::BindingTemporary {
                        value_type: result.value_type(),
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    BodyNode::BindingTemporary {
                        value_type: source.value_type(),
                        origin: definition_origin,
                    },
                )
            }
            DefaultBindingActionViewV1::Component {
                source,
                result,
                setup,
                call,
                definition_origin,
                ..
            } => {
                self.push_child(pending, BodyNode::Expression(call))?;
                self.push_statements(pending, setup)?;
                self.push_child(
                    pending,
                    BodyNode::BindingTemporary {
                        value_type: result.value_type(),
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    BodyNode::BindingTemporary {
                        value_type: source.value_type(),
                        origin: definition_origin,
                    },
                )
            }
            DefaultBindingActionViewV1::Bind {
                source,
                target,
                definition_origin,
            } => {
                self.push_child(
                    pending,
                    BodyNode::BindingLeaf {
                        value_type: target.value_type(),
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    BodyNode::BindingTemporary {
                        value_type: source.value_type(),
                        origin: definition_origin,
                    },
                )
            }
        }
    }

    pub(in super::super) fn process_binding_shape(
        &mut self,
        shape: &'body DefaultBindingShapeV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match shape.view() {
            DefaultBindingShapeViewV1::Binding(leaf) => self.push_child(
                pending,
                BodyNode::BindingLeaf {
                    value_type: leaf.value_type(),
                    origin,
                },
            ),
            DefaultBindingShapeViewV1::Wildcard => Ok(()),
            DefaultBindingShapeViewV1::Tuple(elements) => {
                self.push_binding_shapes(pending, elements, origin)
            }
            DefaultBindingShapeViewV1::Struct { owner_type, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        BodyNode::BindingShape {
                            shape: field.shape(),
                            origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::BindingShapeOwner,
                )
            }
            DefaultBindingShapeViewV1::Class {
                owner_type,
                components,
            } => {
                for component in components.iter().rev() {
                    self.push_child(
                        pending,
                        BodyNode::BindingShape {
                            shape: component.shape(),
                            origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::BindingShapeOwner,
                )
            }
        }
    }

    fn push_binding_shapes(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        shapes: &'body [DefaultBindingShapeV1],
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        for shape in shapes.iter().rev() {
            self.push_child(pending, BodyNode::BindingShape { shape, origin })?;
        }
        Ok(())
    }

    pub(in super::super) fn process_binding_projection(
        &mut self,
        projection: &'body DefaultBindingProjectionV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match projection.view() {
            DefaultBindingProjectionViewV1::TupleIndex(_) => Ok(()),
            DefaultBindingProjectionViewV1::StructField {
                declaration,
                owner_type,
            } => self.push_child(
                pending,
                BodyNode::FieldUse {
                    target: FieldTargetView::Struct {
                        declaration,
                        owner_type,
                    },
                    origin,
                    site: ExportDefaultReferenceOccurrenceSiteV1::BindingAction,
                },
            ),
        }
    }

    pub(in super::super) fn process_iterator_conformance(
        &mut self,
        conformance: &'body DefaultIteratorConformanceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        let origin = conformance.definition_origin();
        self.push_type(
            pending,
            conformance.interface_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::IteratorInterface,
        )?;
        self.push_child(
            pending,
            BodyNode::BindingTemporary {
                value_type: conformance.iterator().value_type(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            BodyNode::BindingTemporary {
                value_type: conformance.source().value_type(),
                origin,
            },
        )
    }

    pub(in super::super) fn process_iterator_next(
        &mut self,
        next: &'body DefaultIteratorNextV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        let origin = next.definition_origin();
        self.push_child(
            pending,
            BodyNode::BindingTemporary {
                value_type: next.element().value_type(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            BodyNode::AppliedOption {
                option: next.option(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            BodyNode::BindingTemporary {
                value_type: next.result().value_type(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            BodyNode::CallableUse {
                callable: next.callable(),
                origin,
            },
        )
    }

    pub(in super::super) fn process_applied_option(
        &mut self,
        option: &'body DefaultAppliedOptionV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(
            pending,
            BodyNode::ConstructorUse {
                target: ConstructorTargetView::Variant(option.none()),
                origin,
                site: ExportDefaultReferenceOccurrenceSiteV1::IteratorProtocol,
            },
        )?;
        self.push_child(
            pending,
            BodyNode::VariantFieldShape {
                field: option.some_payload(),
                origin,
                site: DefaultBodyProviderTypeSiteV1::EnumVariantFieldOwner,
            },
        )
    }
}
