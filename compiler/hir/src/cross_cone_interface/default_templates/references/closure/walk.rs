use super::visitor::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceTargetV1 as Target,
    DefaultBodyReferenceVisitorV1, ReferenceWalker,
};
use crate::{DefaultStatementV1, ExportDefaultBodyV1};

mod model;
mod schedule;
pub(super) use model::BodyNode;
use model::{ScheduledWork, WorkItem};
mod expression;
mod nested;
mod statement;

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn walk_body(&mut self, body: &'body ExportDefaultBodyV1) -> Result<(), V::Error> {
        self.walk_nodes(std::iter::once(BodyNode::Body(body)))
    }

    pub(super) fn walk_statements(
        &mut self,
        statements: &'body [DefaultStatementV1],
    ) -> Result<(), V::Error> {
        self.walk_nodes(statements.iter().map(BodyNode::Statement))
    }

    fn walk_nodes(
        &mut self,
        nodes: impl DoubleEndedIterator<Item = BodyNode<'body>>,
    ) -> Result<(), V::Error> {
        let mut pending = Vec::new();
        for node in nodes.rev() {
            self.push_child(&mut pending, node)?;
        }

        while let Some(ScheduledWork { work, attachment }) = pending.pop() {
            self.current = attachment;
            match work {
                WorkItem::Body { node } => {
                    self.attach_node(node)?;
                    self.process_node(node, &mut pending)?;
                }
                WorkItem::Type {
                    target,
                    origin,
                    site,
                } => {
                    self.observe(Target::Type(target), origin, site)?;
                }
                WorkItem::Callable {
                    target,
                    origin,
                    site,
                } => {
                    self.observe(Target::Callable(target), origin, site)?;
                }
                WorkItem::Constructor {
                    target,
                    origin,
                    site,
                } => {
                    self.observe(Target::Constructor(target), origin, site)?;
                }
                WorkItem::Global {
                    target,
                    origin,
                    site,
                } => {
                    self.observe(Target::Global(target), origin, site)?;
                }
                WorkItem::Singleton {
                    target,
                    origin,
                    site,
                } => {
                    self.observe(Target::Singleton(target), origin, site)?;
                }
                WorkItem::Field {
                    target,
                    origin,
                    site,
                } => {
                    self.observe(Target::Field(target), origin, site)?;
                }
            }
        }
        Ok(())
    }

    fn process_node(
        &mut self,
        node: BodyNode<'body>,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match node {
            BodyNode::Body(body) => self.process_body(body, pending),
            BodyNode::Statement(statement) => self.process_statement(statement, pending),
            BodyNode::Expression(expression) => self.process_expression(expression, pending),
            BodyNode::Pattern { pattern, origin } => self.process_pattern(pattern, origin, pending),
            BodyNode::AssignTarget { target, origin } => {
                self.process_assign_target(target, origin, pending)
            }
            BodyNode::When { value, origin } => self.process_when(value, origin, pending),
            BodyNode::WhenArm(arm) => self.process_when_arm(arm, pending),
            BodyNode::WhenGuard(guard) => self.process_when_guard(guard, pending),
            BodyNode::WhenFallback { fallback, origin } => {
                self.process_when_fallback(fallback, origin, pending)
            }
            BodyNode::Try(value) => self.process_try(value, pending),
            BodyNode::Catch(catch) => self.process_catch(catch, pending),
            BodyNode::For { plan, origin } => self.process_for(plan, origin, pending),
            BodyNode::BindingPlan { plan, origin } => {
                self.process_binding_plan(plan, origin, pending)
            }
            BodyNode::BindingAction(action) => self.process_binding_action(action, pending),
            BodyNode::BindingShape { shape, origin } => {
                self.process_binding_shape(shape, origin, pending)
            }
            BodyNode::BindingProjection { projection, origin } => {
                self.process_binding_projection(projection, origin, pending)
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
                self.process_iterator_conformance(conformance, pending)
            }
            BodyNode::IteratorNext(next) => self.process_iterator_next(next, pending),
            BodyNode::AppliedOption { option, origin } => {
                self.process_applied_option(option, origin, pending)
            }
            BodyNode::LocalFunction { function, origin } => {
                self.process_local_function(function, origin, pending)
            }
            BodyNode::Lambda { lambda, origin } => self.process_lambda(lambda, origin, pending),
            BodyNode::Anonymous { function, origin } => {
                self.process_anonymous(function, origin, pending)
            }
            BodyNode::CallableReference { reference, origin } => {
                self.process_callable_reference(reference, origin, pending)
            }
            BodyNode::Capture(capture) => self.process_capture(capture, pending),
            BodyNode::CallableUse { callable, origin } => {
                self.process_callable_use(callable, origin, pending)
            }
            BodyNode::CallableShape { callable, origin } => {
                self.process_callable_shape(callable, origin, pending)
            }
            BodyNode::BoundCallableUse { callable, origin } => {
                self.process_bound_callable_use(callable, origin, pending)
            }
            BodyNode::BoundCallableShape { callable, origin } => {
                self.process_bound_callable_shape(callable, origin, pending)
            }
            BodyNode::MethodCallee { callee, origin } => {
                self.process_method_callee(callee, origin, pending)
            }
            BodyNode::MethodCalleeShape { callee, origin } => {
                self.process_method_callee_shape(callee, origin, pending)
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
                self.process_literal_equality(equality, origin, pending)
            }
            BodyNode::ArrayAssembly { assembly, origin } => {
                self.process_array_assembly(assembly, origin, pending)
            }
            BodyNode::IntegerArguments(arguments) => {
                self.process_integer_arguments(arguments, pending)
            }
        }
    }

    fn process_body(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(pending, BodyNode::Expression(body.value()))?;
        self.push_statements(pending, body.statements())
    }
}
