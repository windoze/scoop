use super::super::super::ExportDefaultReferenceClosureValidationError;
use super::super::super::{
    ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1, FieldTargetView,
};
use super::super::{BodyNode, Validator, WorkItem};
use crate::{
    DefaultAppliedOptionV1, DefaultBindingActionV1, DefaultBindingActionViewV1,
    DefaultBindingPlanV1, DefaultBindingProjectionV1, DefaultBindingProjectionViewV1,
    DefaultBindingShapeV1, DefaultBindingShapeViewV1, DefaultBodyProviderTypeSiteV1,
    DefaultForIterationPlanV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    ExportDefinitionSourceV1,
};

impl Validator<'_> {
    pub(in super::super) fn process_for<'body>(
        &mut self,
        plan: &'body DefaultForIterationPlanV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_statements(pending, depth, plan.body())?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingPlan {
                plan: plan.binding(),
                origin,
            },
        )?;
        self.push_child(pending, depth, BodyNode::IteratorNext(plan.next()))?;
        self.push_child(
            pending,
            depth,
            BodyNode::IteratorConformance(plan.conformance()),
        )?;
        self.push_child(pending, depth, BodyNode::Expression(plan.iterator_call()))?;
        self.push_statements(pending, depth, plan.iterator_setup())?;
        self.push_child(pending, depth, BodyNode::Expression(plan.source_init()))?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                value_type: plan.source().value_type(),
                origin,
            },
        )?;
        self.push_statements(pending, depth, plan.source_setup())
    }

    pub(in super::super) fn process_binding_plan<'body>(
        &mut self,
        plan: &'body DefaultBindingPlanV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for action in plan.actions().iter().rev() {
            self.push_child(pending, depth, BodyNode::BindingAction(action))?;
        }
        self.push_child(
            pending,
            depth,
            BodyNode::BindingShape {
                shape: plan.shape(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                value_type: plan.subject().value_type(),
                origin,
            },
        )
    }

    pub(in super::super) fn process_binding_action<'body>(
        &mut self,
        action: &'body DefaultBindingActionV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match action.view() {
            DefaultBindingActionViewV1::Project {
                source,
                result,
                projection,
                definition_origin,
            } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingProjection {
                        projection,
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        value_type: result.value_type(),
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
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
                self.push_child(pending, depth, BodyNode::Expression(call))?;
                self.push_statements(pending, depth, setup)?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        value_type: result.value_type(),
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
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
                    depth,
                    BodyNode::BindingLeaf {
                        value_type: target.value_type(),
                        origin: definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        value_type: source.value_type(),
                        origin: definition_origin,
                    },
                )
            }
        }
    }

    pub(in super::super) fn process_binding_shape<'body>(
        &mut self,
        shape: &'body DefaultBindingShapeV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match shape.view() {
            DefaultBindingShapeViewV1::Binding(leaf) => self.push_child(
                pending,
                depth,
                BodyNode::BindingLeaf {
                    value_type: leaf.value_type(),
                    origin,
                },
            ),
            DefaultBindingShapeViewV1::Wildcard => Ok(()),
            DefaultBindingShapeViewV1::Tuple(elements) => {
                self.push_binding_shapes(pending, depth, elements, origin)
            }
            DefaultBindingShapeViewV1::Struct { owner_type, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        depth,
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
                        depth,
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

    fn push_binding_shapes<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        shapes: &'body [DefaultBindingShapeV1],
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for shape in shapes.iter().rev() {
            self.push_child(pending, depth, BodyNode::BindingShape { shape, origin })?;
        }
        Ok(())
    }

    pub(in super::super) fn process_binding_projection<'body>(
        &mut self,
        projection: &'body DefaultBindingProjectionV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match projection.view() {
            DefaultBindingProjectionViewV1::TupleIndex(_) => Ok(()),
            DefaultBindingProjectionViewV1::StructField {
                declaration,
                owner_type,
            } => self.push_child(
                pending,
                depth,
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

    pub(in super::super) fn process_iterator_conformance<'body>(
        &mut self,
        conformance: &'body DefaultIteratorConformanceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let origin = conformance.definition_origin();
        self.push_type(
            pending,
            conformance.interface_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::IteratorInterface,
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                value_type: conformance.iterator().value_type(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                value_type: conformance.source().value_type(),
                origin,
            },
        )
    }

    pub(in super::super) fn process_iterator_next<'body>(
        &mut self,
        next: &'body DefaultIteratorNextV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let origin = next.definition_origin();
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                value_type: next.element().value_type(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::AppliedOption {
                option: next.option(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                value_type: next.result().value_type(),
                origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::CallableUse {
                callable: next.callable(),
                origin,
            },
        )
    }

    pub(in super::super) fn process_applied_option<'body>(
        &mut self,
        option: &'body DefaultAppliedOptionV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_child(
            pending,
            depth,
            BodyNode::ConstructorUse {
                target: ConstructorTargetView::Variant(option.none()),
                origin,
                site: ExportDefaultReferenceOccurrenceSiteV1::IteratorProtocol,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::VariantFieldShape {
                field: option.some_payload(),
                origin,
                site: DefaultBodyProviderTypeSiteV1::EnumVariantFieldOwner,
            },
        )
    }
}
