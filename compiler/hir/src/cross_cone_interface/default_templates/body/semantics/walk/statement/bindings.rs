use crate::{
    DefaultAppliedOptionV1, DefaultBindingActionV1, DefaultBindingActionViewV1,
    DefaultBindingPlanV1, DefaultBindingProjectionV1, DefaultBindingProjectionViewV1,
    DefaultBindingShapeV1, DefaultBindingShapeViewV1, DefaultForIterationPlanV1,
    DefaultIteratorConformanceV1, DefaultIteratorNextV1, ExportDefinitionSourceV1,
};

use super::super::{BodyNode, BodyWalkMode, Validator, WorkItem};
use crate::{DefaultBodyOriginSiteV1, DefaultBodyProviderTypeSiteV1};

impl<M> Validator<'_, M>
where
    M: BodyWalkMode,
{
    pub(in super::super) fn process_for<'body>(
        &mut self,
        plan: &'body DefaultForIterationPlanV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_statements(pending, depth, plan.body())?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingPlan {
                plan: plan.binding(),
                definition_origin,
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
                temporary: plan.source(),
                definition_origin,
            },
        )?;
        self.push_statements(pending, depth, plan.source_setup())
    }

    pub(in super::super) fn process_binding_plan<'body>(
        &mut self,
        plan: &'body DefaultBindingPlanV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        for action in plan.actions().iter().rev() {
            self.push_child(pending, depth, BodyNode::BindingAction(action))?;
        }
        self.push_child(
            pending,
            depth,
            BodyNode::BindingShape {
                shape: plan.shape(),
                definition_origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                temporary: plan.subject(),
                definition_origin,
            },
        )
    }

    pub(in super::super) fn process_binding_action<'body>(
        &mut self,
        action: &'body DefaultBindingActionV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
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
                        definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        temporary: result,
                        definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        temporary: source,
                        definition_origin,
                    },
                )?;
                self.push_binding_action_origin(pending, depth, definition_origin)
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
                        temporary: result,
                        definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        temporary: source,
                        definition_origin,
                    },
                )?;
                self.push_binding_action_origin(pending, depth, definition_origin)
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
                        leaf: target,
                        definition_origin,
                    },
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::BindingTemporary {
                        temporary: source,
                        definition_origin,
                    },
                )?;
                self.push_binding_action_origin(pending, depth, definition_origin)
            }
        }
    }

    fn push_binding_action_origin<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: definition_origin,
                site: DefaultBodyOriginSiteV1::BindingAction,
            },
        )
    }

    pub(in super::super) fn process_binding_shape<'body>(
        &mut self,
        shape: &'body DefaultBindingShapeV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match shape.view() {
            DefaultBindingShapeViewV1::Binding(leaf) => self.push_child(
                pending,
                depth,
                BodyNode::BindingLeaf {
                    leaf,
                    definition_origin,
                },
            ),
            DefaultBindingShapeViewV1::Wildcard => Ok(()),
            DefaultBindingShapeViewV1::Tuple(elements) => {
                self.push_binding_shapes(pending, depth, elements, definition_origin)
            }
            DefaultBindingShapeViewV1::Struct { owner_type, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        depth,
                        BodyNode::BindingShape {
                            shape: field.shape(),
                            definition_origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    owner_type,
                    DefaultBodyProviderTypeSiteV1::BindingShapeOwner,
                    definition_origin,
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
                            definition_origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    owner_type,
                    DefaultBodyProviderTypeSiteV1::BindingShapeOwner,
                    definition_origin,
                )
            }
        }
    }

    fn push_binding_shapes<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        shapes: &'body [DefaultBindingShapeV1],
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        for shape in shapes.iter().rev() {
            self.push_child(
                pending,
                depth,
                BodyNode::BindingShape {
                    shape,
                    definition_origin,
                },
            )?;
        }
        Ok(())
    }

    pub(in super::super) fn process_binding_projection<'body>(
        &mut self,
        projection: &'body DefaultBindingProjectionV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match projection.view() {
            DefaultBindingProjectionViewV1::TupleIndex(_) => Ok(()),
            DefaultBindingProjectionViewV1::StructField { owner_type, .. } => self.push_type(
                pending,
                owner_type,
                DefaultBodyProviderTypeSiteV1::BindingProjectionOwner,
                definition_origin,
            ),
        }
    }

    pub(in super::super) fn process_iterator_conformance<'body>(
        &mut self,
        conformance: &'body DefaultIteratorConformanceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_type(
            pending,
            conformance.interface_type(),
            DefaultBodyProviderTypeSiteV1::IteratorInterface,
            conformance.definition_origin(),
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                temporary: conformance.iterator(),
                definition_origin: conformance.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                temporary: conformance.source(),
                definition_origin: conformance.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: conformance.definition_origin(),
                site: DefaultBodyOriginSiteV1::IteratorConformance,
            },
        )
    }

    pub(in super::super) fn process_iterator_next<'body>(
        &mut self,
        next: &'body DefaultIteratorNextV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                temporary: next.element(),
                definition_origin: next.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::AppliedOption {
                option: next.option(),
                definition_origin: next.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BindingTemporary {
                temporary: next.result(),
                definition_origin: next.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::CallableRef {
                callable: next.callable(),
                definition_origin: next.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: next.definition_origin(),
                site: DefaultBodyOriginSiteV1::IteratorNext,
            },
        )
    }

    pub(in super::super) fn process_applied_option<'body>(
        &mut self,
        option: &'body DefaultAppliedOptionV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_child(
            pending,
            depth,
            BodyNode::EnumVariantRef {
                variant: option.none(),
                definition_origin,
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::EnumVariantFieldRef {
                field: option.some_payload(),
                definition_origin,
            },
        )
    }
}
