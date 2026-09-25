use crate::{
    DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingActionViewV1, DefaultCatchV1,
    DefaultForIterationPlanV1, DefaultStatementKindV1, DefaultStatementV1, DefaultTryV1,
    DefaultWhenArmV1, DefaultWhenFallbackV1, DefaultWhenFallbackViewV1, DefaultWhenGuardV1,
    DefaultWhenV1, OptionalDefaultStatementListViewV1,
};

use super::BodyNode;
use crate::cross_cone_interface::default_templates::body::nested::semantics::{
    DefaultBodyNestedAuthority, DefaultNestedCallableAbiValidationError, Validator,
};

impl<A, E> Validator<'_, A, E>
where
    A: DefaultBodyNestedAuthority<E>,
{
    pub(super) fn process_statement<'body>(
        &mut self,
        statement: &'body DefaultStatementV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match statement.kind() {
            DefaultStatementKindV1::Expr(expression)
            | DefaultStatementKindV1::Throw(expression) => {
                self.push_child(pending, BodyNode::Expression(expression))
            }
            DefaultStatementKindV1::InitializationEnsure(_)
            | DefaultStatementKindV1::Break
            | DefaultStatementKindV1::Continue => Ok(()),
            DefaultStatementKindV1::LocalFunction(function) => {
                self.push_child(pending, BodyNode::LocalFunction(function))
            }
            DefaultStatementKindV1::Return(value) => match value.as_ref() {
                Some(expression) => self.push_child(pending, BodyNode::Expression(expression)),
                None => Ok(()),
            },
            DefaultStatementKindV1::ValDecl { init, .. } => {
                self.push_child(pending, BodyNode::Expression(init))
            }
            DefaultStatementKindV1::Assign { target, value } => {
                self.push_child(pending, BodyNode::Expression(value))?;
                self.push_child(pending, BodyNode::AssignTarget(target))
            }
            DefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => {
                if let OptionalDefaultStatementListViewV1::Present(statements) = else_body.view() {
                    self.push_child(pending, BodyNode::StatementBlock(statements))?;
                }
                self.push_child(pending, BodyNode::StatementBlock(then_body))?;
                self.push_child(pending, BodyNode::Expression(condition))
            }
            DefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => {
                self.push_child(pending, BodyNode::StatementBlock(body))?;
                self.push_child(
                    pending,
                    BodyNode::Evaluation {
                        setup: condition_setup,
                        value: condition,
                    },
                )
            }
            DefaultStatementKindV1::For(plan) => self.push_child(pending, BodyNode::For(plan)),
            DefaultStatementKindV1::When(value) => self.push_child(pending, BodyNode::When(value)),
            DefaultStatementKindV1::Try(value) => self.push_child(pending, BodyNode::Try(value)),
        }
    }

    pub(super) fn process_assign_target<'body>(
        &mut self,
        target: &'body DefaultAssignTargetV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match target {
            DefaultAssignTargetV1::Local { .. } | DefaultAssignTargetV1::Global { .. } => Ok(()),
            DefaultAssignTargetV1::Index { array, index } => {
                self.push_child(pending, BodyNode::Expression(index))?;
                self.push_child(pending, BodyNode::Expression(array))
            }
            DefaultAssignTargetV1::Field { receiver, .. } => {
                self.push_child(pending, BodyNode::Expression(receiver))
            }
        }
    }

    pub(super) fn process_when<'body>(
        &mut self,
        value: &'body DefaultWhenV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.push_child(pending, BodyNode::WhenFallback(value.fallback()))?;
        for arm in value.arms().iter().rev() {
            self.push_child(pending, BodyNode::WhenArm(arm))?;
        }
        self.push_child(pending, BodyNode::Expression(value.subject()))
    }

    pub(super) fn process_when_arm<'body>(
        &mut self,
        arm: &'body DefaultWhenArmV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.push_child(pending, BodyNode::StatementBlock(arm.body()))?;
        if let Some(guard) = arm.guard().as_ref() {
            self.push_child(pending, BodyNode::WhenGuard(guard))?;
        }
        Ok(())
    }

    pub(super) fn process_when_guard<'body>(
        &mut self,
        guard: &'body DefaultWhenGuardV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.push_child(
            pending,
            BodyNode::Evaluation {
                setup: guard.setup(),
                value: guard.condition(),
            },
        )
    }

    pub(super) fn process_when_fallback<'body>(
        &mut self,
        fallback: &'body DefaultWhenFallbackV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match fallback.view() {
            DefaultWhenFallbackViewV1::Else(statements) => {
                self.push_child(pending, BodyNode::StatementBlock(statements))
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { .. }
            | DefaultWhenFallbackViewV1::PatternMatrix { .. }
            | DefaultWhenFallbackViewV1::EnumPatternMatrix { .. } => Ok(()),
        }
    }

    pub(super) fn process_try<'body>(
        &mut self,
        value: &'body DefaultTryV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        if let OptionalDefaultStatementListViewV1::Present(statements) = value.finally_body().view()
        {
            self.push_child(pending, BodyNode::StatementBlock(statements))?;
        }
        for catch in value.catches().iter().rev() {
            self.push_child(pending, BodyNode::Catch(catch))?;
        }
        self.push_child(pending, BodyNode::StatementBlock(value.body()))
    }

    pub(super) fn process_catch<'body>(
        &mut self,
        catch: &'body DefaultCatchV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.push_child(pending, BodyNode::StatementBlock(catch.body()))
    }

    pub(super) fn process_for<'body>(
        &mut self,
        plan: &'body DefaultForIterationPlanV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.push_child(pending, BodyNode::StatementBlock(plan.body()))?;
        for action in plan.binding().actions().iter().rev() {
            self.push_child(pending, BodyNode::BindingAction(action))?;
        }
        self.push_child(
            pending,
            BodyNode::Evaluation {
                setup: plan.iterator_setup(),
                value: plan.iterator_call(),
            },
        )?;
        self.push_child(
            pending,
            BodyNode::Evaluation {
                setup: plan.source_setup(),
                value: plan.source_init(),
            },
        )
    }

    pub(super) fn process_binding_action<'body>(
        &mut self,
        action: &'body DefaultBindingActionV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        match action.view() {
            DefaultBindingActionViewV1::Component { setup, call, .. } => {
                self.push_child(pending, BodyNode::Evaluation { setup, value: call })
            }
            DefaultBindingActionViewV1::Project { .. }
            | DefaultBindingActionViewV1::Bind { .. } => Ok(()),
        }
    }

    pub(super) fn push_statements<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        statements: &'body [DefaultStatementV1],
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        for statement in statements.iter().rev() {
            self.push_child(pending, BodyNode::Statement(statement))?;
        }
        Ok(())
    }
}
