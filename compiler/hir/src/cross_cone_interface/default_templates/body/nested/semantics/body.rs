use crate::{
    DefaultAnonymousFunctionV1, DefaultAssignTargetV1, DefaultBindingActionV1,
    DefaultCallableReferenceV1, DefaultCatchV1, DefaultExpressionV1, DefaultForIterationPlanV1,
    DefaultLambdaV1, DefaultLocalFunctionV1, DefaultStatementV1, DefaultTryV1, DefaultWhenArmV1,
    DefaultWhenFallbackV1, DefaultWhenGuardV1, DefaultWhenV1, ExportDefaultBodyV1,
    ExportDefaultTemplateV1,
};

use super::{
    DefaultNestedCallableAbiValidationError, DefaultNestedCallableSemanticAuthority, Validator,
};
use scoop_wire::{BudgetMeter, WirePath};

mod expression;
mod statement;

impl ExportDefaultBodyV1 {
    /// Validates every nested callable descriptor and closes all local-callable
    /// uses over declarations in this exact default body.
    pub fn validate_nested_callable_abi_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        Validator::new(template, authority, meter, path).validate_body(self)
    }
}

impl<A, E> Validator<'_, A, E>
where
    A: DefaultNestedCallableSemanticAuthority<E>,
{
    fn validate_body(
        &mut self,
        body: &ExportDefaultBodyV1,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        let mut pending = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut pending, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        pending.push(WorkItem {
            node: BodyNode::Body(body),
            depth: 1,
        });

        while let Some(work) = pending.pop() {
            self.process_node(work.node, work.depth, &mut pending)?;
        }

        self.validate_local_uses()
    }

    fn process_node<'body>(
        &mut self,
        node: BodyNode<'body>,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match node {
            BodyNode::Body(body) => {
                self.enter_node(depth)?;
                self.process_body(body, depth, pending)
            }
            BodyNode::Statement(statement) => {
                self.enter_node(depth)?;
                self.process_statement(statement, depth, pending)
            }
            BodyNode::Expression(expression) => {
                self.enter_node(depth)?;
                self.process_expression(expression, depth, pending)
            }
            BodyNode::AssignTarget(target) => {
                self.enter_node(depth)?;
                self.process_assign_target(target, depth, pending)
            }
            BodyNode::When(value) => {
                self.enter_node(depth)?;
                self.process_when(value, depth, pending)
            }
            BodyNode::WhenArm(arm) => {
                self.enter_node(depth)?;
                self.process_when_arm(arm, depth, pending)
            }
            BodyNode::WhenGuard(guard) => {
                self.enter_node(depth)?;
                self.process_when_guard(guard, depth, pending)
            }
            BodyNode::WhenFallback(fallback) => {
                self.enter_node(depth)?;
                self.process_when_fallback(fallback, depth, pending)
            }
            BodyNode::Try(value) => {
                self.enter_node(depth)?;
                self.process_try(value, depth, pending)
            }
            BodyNode::Catch(catch) => {
                self.enter_node(depth)?;
                self.process_catch(catch, depth, pending)
            }
            BodyNode::For(plan) => {
                self.enter_node(depth)?;
                self.process_for(plan, depth, pending)
            }
            BodyNode::BindingAction(action) => {
                self.enter_node(depth)?;
                self.process_binding_action(action, depth, pending)
            }
            BodyNode::LocalFunction(function) => {
                self.validate_local_function(function, depth)?;
                self.record_local_declaration(function.declaration())
            }
            BodyNode::Lambda(lambda) => self.validate_lambda(lambda, depth),
            BodyNode::AnonymousFunction(function) => self.validate_anonymous(function, depth),
            BodyNode::CallableReference(reference) => {
                self.validate_callable_reference(reference, depth)?;
                self.process_callable_reference(reference, depth, pending)
            }
        }
    }

    fn process_body<'body>(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.push_child(pending, depth, BodyNode::Expression(body.value()))?;
        self.push_statements(pending, depth, body.statements())
    }

    pub(super) fn push_child<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        parent_depth: u64,
        node: BodyNode<'body>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        let depth = self.child_depth(parent_depth)?;
        self.meter
            .try_reserve_collection_slots(pending, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        pending.push(WorkItem { node, depth });
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct WorkItem<'a> {
    node: BodyNode<'a>,
    depth: u64,
}

#[derive(Clone, Copy)]
pub(super) enum BodyNode<'a> {
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Expression(&'a DefaultExpressionV1),
    AssignTarget(&'a DefaultAssignTargetV1),
    When(&'a DefaultWhenV1),
    WhenArm(&'a DefaultWhenArmV1),
    WhenGuard(&'a DefaultWhenGuardV1),
    WhenFallback(&'a DefaultWhenFallbackV1),
    Try(&'a DefaultTryV1),
    Catch(&'a DefaultCatchV1),
    For(&'a DefaultForIterationPlanV1),
    BindingAction(&'a DefaultBindingActionV1),
    LocalFunction(&'a DefaultLocalFunctionV1),
    Lambda(&'a DefaultLambdaV1),
    AnonymousFunction(&'a DefaultAnonymousFunctionV1),
    CallableReference(&'a DefaultCallableReferenceV1),
}
