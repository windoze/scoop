use super::{DefaultBodyNestedAuthority, DefaultBodyValidationInputV1, authority::PublicAuthority};

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
use scoop_wire::WirePath;

mod expression;
mod statement;

impl ExportDefaultBodyV1 {
    /// Validates every nested callable descriptor and closes all local-callable
    /// uses over declarations in this exact default body.
    pub fn validate_nested_callable_abi_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        DefaultBodyValidationInputV1::from(template).validate_nested_callable_abi(
            self,
            &mut PublicAuthority {
                template,
                authority,
            },
            path,
        )
    }
}

impl DefaultBodyValidationInputV1<'_> {
    pub(crate) fn validate_nested_callable_abi<A: DefaultBodyNestedAuthority<E>, E>(
        self,
        body: &ExportDefaultBodyV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        Validator::new(self, authority, path).validate_body(body)
    }
}

impl<A, E> Validator<'_, A, E>
where
    A: DefaultBodyNestedAuthority<E>,
{
    fn validate_body(
        &mut self,
        body: &ExportDefaultBodyV1,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.next_site = super::DefaultNestedCallableSiteV1::Body { ordinal: 0 };
        let mut pending = Vec::new();
        scoop_wire::allocation::try_reserve(&mut pending, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        pending.push(BodyNode::Body(body));

        while let Some(work) = pending.pop() {
            self.process_node(work, &mut pending)?;
        }

        debug_assert!(self.local_scopes.is_empty());
        Ok(())
    }

    fn process_node<'body>(
        &mut self,
        node: BodyNode<'body>,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match node {
            BodyNode::LeaveLocalScope => self.leave_local_scope(),
            BodyNode::StatementBlock(statements) => {
                self.enter_local_scope()?;
                self.push_child(pending, BodyNode::LeaveLocalScope)?;
                self.push_statements(pending, statements)
            }
            BodyNode::Evaluation { setup, value } => {
                self.enter_local_scope()?;
                self.push_child(pending, BodyNode::LeaveLocalScope)?;
                self.push_child(pending, BodyNode::Expression(value))?;
                self.push_statements(pending, setup)
            }
            BodyNode::Body(body) => self.process_body(body, pending),
            BodyNode::Statement(statement) => self.process_statement(statement, pending),
            BodyNode::Expression(expression) => self.process_expression(expression, pending),
            BodyNode::AssignTarget(target) => self.process_assign_target(target, pending),
            BodyNode::When(value) => self.process_when(value, pending),
            BodyNode::WhenArm(arm) => self.process_when_arm(arm, pending),
            BodyNode::WhenGuard(guard) => self.process_when_guard(guard, pending),
            BodyNode::WhenFallback(fallback) => self.process_when_fallback(fallback, pending),
            BodyNode::Try(value) => self.process_try(value, pending),
            BodyNode::Catch(catch) => self.process_catch(catch, pending),
            BodyNode::For(plan) => self.process_for(plan, pending),
            BodyNode::BindingAction(action) => self.process_binding_action(action, pending),
            BodyNode::LocalFunction(function) => {
                self.validate_local_function(function)?;
                self.record_local_declaration(function.declaration())
            }
            BodyNode::Lambda(lambda) => self.validate_lambda(lambda),
            BodyNode::AnonymousFunction(function) => self.validate_anonymous(function),
            BodyNode::CallableReference(reference) => {
                self.validate_callable_reference(reference)?;
                self.process_callable_reference(reference, pending)
            }
        }
    }

    fn process_body<'body>(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.enter_local_scope()?;
        self.push_child(pending, BodyNode::LeaveLocalScope)?;
        self.push_child(pending, BodyNode::Expression(body.value()))?;
        self.push_statements(pending, body.statements())
    }

    pub(super) fn push_child<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        node: BodyNode<'body>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        scoop_wire::allocation::try_reserve(pending, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        pending.push(node);
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) enum BodyNode<'a> {
    LeaveLocalScope,
    StatementBlock(&'a [DefaultStatementV1]),
    Evaluation {
        setup: &'a [DefaultStatementV1],
        value: &'a DefaultExpressionV1,
    },
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
