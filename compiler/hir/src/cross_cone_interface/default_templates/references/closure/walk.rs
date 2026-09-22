use super::visitor::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceTargetV1 as Target,
    DefaultBodyReferenceVisitorV1, ReferenceWalker,
};
use crate::ExportDefaultBodyV1;

mod model;
mod schedule;
pub(super) use model::BodyNode;
use model::{ScheduledWork, WorkItem};
mod expression;
mod nested;
mod statement;

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn walk_body(&mut self, body: &'body ExportDefaultBodyV1) -> Result<(), V::Error> {
        let mut pending = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut pending, 1, self.path)
            .map_err(V::Error::from)?;
        pending.push(ScheduledWork {
            work: WorkItem::Body {
                node: BodyNode::Body(body),
                depth: 1,
            },
            attachment: self.current,
        });

        while let Some(ScheduledWork { work, attachment }) = pending.pop() {
            self.current = attachment;
            match work {
                WorkItem::Body { node, depth } => {
                    self.enter_node(depth)?;
                    self.attach_node(node)?;
                    self.process_node(node, depth, &mut pending)?;
                }
                WorkItem::Type {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_node(1)?;
                    self.observe(Target::Type(target), origin, site)?;
                }
                WorkItem::Callable {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_node(1)?;
                    self.observe(Target::Callable(target), origin, site)?;
                }
                WorkItem::Constructor {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_node(1)?;
                    self.observe(Target::Constructor(target), origin, site)?;
                }
                WorkItem::Global {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_node(1)?;
                    self.observe(Target::Global(target), origin, site)?;
                }
                WorkItem::Singleton {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_node(1)?;
                    self.observe(Target::Singleton(target), origin, site)?;
                }
                WorkItem::Field {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_node(1)?;
                    self.observe(Target::Field(target), origin, site)?;
                }
            }
        }
        Ok(())
    }

    fn process_node(
        &mut self,
        node: BodyNode<'body>,
        depth: u64,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match node {
            BodyNode::Body(body) => self.process_body(body, depth, pending),
            BodyNode::Statement(statement) => self.process_statement(statement, depth, pending),
            BodyNode::Expression(expression) => self.process_expression(expression, depth, pending),
            BodyNode::Pattern { pattern, origin } => {
                self.process_pattern(pattern, origin, depth, pending)
            }
            BodyNode::AssignTarget { target, origin } => {
                self.process_assign_target(target, origin, depth, pending)
            }
            BodyNode::When { value, origin } => self.process_when(value, origin, depth, pending),
            BodyNode::WhenArm(arm) => self.process_when_arm(arm, depth, pending),
            BodyNode::WhenGuard(guard) => self.process_when_guard(guard, depth, pending),
            BodyNode::WhenFallback { fallback, origin } => {
                self.process_when_fallback(fallback, origin, depth, pending)
            }
            BodyNode::Try(value) => self.process_try(value, depth, pending),
            BodyNode::Catch(catch) => self.process_catch(catch, depth, pending),
            BodyNode::For { plan, origin } => self.process_for(plan, origin, depth, pending),
            BodyNode::BindingPlan { plan, origin } => {
                self.process_binding_plan(plan, origin, depth, pending)
            }
            BodyNode::BindingAction(action) => self.process_binding_action(action, depth, pending),
            BodyNode::BindingShape { shape, origin } => {
                self.process_binding_shape(shape, origin, depth, pending)
            }
            BodyNode::BindingProjection { projection, origin } => {
                self.process_binding_projection(projection, origin, depth, pending)
            }
            BodyNode::BindingTemporary { value_type, origin } => self.push_type(
                pending,
                value_type,
                origin,
                crate::DefaultBodyProviderTypeSiteV1::BindingTemporaryValue,
            ),
            BodyNode::BindingLeaf { value_type, origin } => self.push_type(
                pending,
                value_type,
                origin,
                crate::DefaultBodyProviderTypeSiteV1::BindingLeafValue,
            ),
            BodyNode::IteratorConformance(conformance) => {
                self.process_iterator_conformance(conformance, depth, pending)
            }
            BodyNode::IteratorNext(next) => self.process_iterator_next(next, depth, pending),
            BodyNode::AppliedOption { option, origin } => {
                self.process_applied_option(option, origin, depth, pending)
            }
            BodyNode::LocalFunction { function, origin } => {
                self.process_local_function(function, origin, depth, pending)
            }
            BodyNode::Lambda { lambda, origin } => {
                self.process_lambda(lambda, origin, depth, pending)
            }
            BodyNode::Anonymous { function, origin } => {
                self.process_anonymous(function, origin, depth, pending)
            }
            BodyNode::CallableReference { reference, origin } => {
                self.process_callable_reference(reference, origin, depth, pending)
            }
            BodyNode::Capture(capture) => self.process_capture(capture, pending),
            BodyNode::CallableUse { callable, origin } => {
                self.process_callable_use(callable, origin, depth, pending)
            }
            BodyNode::CallableShape { callable, origin } => {
                self.process_callable_shape(callable, origin, pending)
            }
            BodyNode::BoundCallableUse { callable, origin } => {
                self.process_bound_callable_use(callable, origin, depth, pending)
            }
            BodyNode::BoundCallableShape { callable, origin } => {
                self.process_bound_callable_shape(callable, origin, depth, pending)
            }
            BodyNode::MethodCallee { callee, origin } => {
                self.process_method_callee(callee, origin, depth, pending)
            }
            BodyNode::MethodCalleeShape { callee, origin } => {
                self.process_method_callee_shape(callee, origin, depth, pending)
            }
            BodyNode::ConstructorUse {
                target,
                origin,
                site,
            } => self.process_constructor_use(target, origin, site, pending),
            BodyNode::VariantFieldShape {
                field,
                origin,
                site,
            } => self.push_type(pending, field.owner_type(), origin, site),
            BodyNode::FieldUse {
                target,
                origin,
                site,
            } => self.process_field_use(target, origin, site, pending),
            BodyNode::LiteralEquality { equality, origin } => {
                self.process_literal_equality(equality, origin, depth, pending)
            }
            BodyNode::ArrayAssembly { assembly, origin } => {
                self.process_array_assembly(assembly, origin, depth, pending)
            }
            BodyNode::IntegerArguments(arguments) => {
                self.process_integer_arguments(arguments, depth, pending)
            }
        }
    }

    fn process_body(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        depth: u64,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(pending, depth, BodyNode::Expression(body.value()))?;
        self.push_statements(pending, depth, body.statements())
    }
}
