//! Statement and structured-control-flow projection.

use crate::{
    DefaultAssignTargetV1, DefaultCatchV1, DefaultStatementKindV1, DefaultStatementV1,
    DefaultTryV1, DefaultWhenArmV1, DefaultWhenConditionV1, DefaultWhenFallbackV1,
    DefaultWhenGuardV1, DefaultWhenV1, OptionalDefaultStatementListV1, OptionalDefaultWhenGuardV1,
    Statement, StatementKind, WhenFallback,
};

use super::BodyProjection;

mod when;

impl BodyProjection<'_, '_> {
    pub(super) fn statement(
        &mut self,
        statement: &Statement,
    ) -> Result<DefaultStatementV1, super::super::DefaultBodyProjectionError> {
        let kind = self.statement_kind(&statement.kind)?;
        let origin = match statement.kind {
            StatementKind::LocalFunction(function) => self.local_function_origin(function)?,
            _ => self.span_origin(statement.span)?,
        };
        DefaultStatementV1::try_new(kind, origin)
            .map_err(super::super::DefaultBodyProjectionError::Statement)
    }

    fn statement_kind(
        &mut self,
        kind: &StatementKind,
    ) -> Result<DefaultStatementKindV1, super::super::DefaultBodyProjectionError> {
        Ok(match kind {
            StatementKind::MemberRequirementCall(call) => {
                DefaultStatementKindV1::MemberRequirementCall {
                    contract: Box::new(call.contract.key(|ty| self.type_key(ty))?),
                    binding: Box::new(
                        self.entities
                            .requirement_binding(&call.binding, self.binders)?,
                    ),
                    arguments: self.expressions(&call.arguments)?,
                    result: Box::new(
                        self.pattern(&crate::Pattern::Binding { local: call.result })?,
                    ),
                }
            }
            StatementKind::ContextScope { value, body } => DefaultStatementKindV1::ContextScope {
                value: Box::new(self.expression(value)?),
                body: self.statements(body)?,
            },
            StatementKind::GenericDelegateEnsure(reference) => {
                DefaultStatementKindV1::GenericDelegateEnsure(
                    self.generic_delegate_reference(reference)?,
                )
            }
            StatementKind::Expr(expression) => {
                DefaultStatementKindV1::Expr(Box::new(self.expression(expression)?))
            }
            StatementKind::InitializationEnsure(unit) => {
                DefaultStatementKindV1::InitializationEnsure(
                    self.entities.initialization_id(*unit)?,
                )
            }
            StatementKind::LocalFunction(function) => {
                DefaultStatementKindV1::LocalFunction(self.local_function(*function)?)
            }
            StatementKind::Return { value } => {
                DefaultStatementKindV1::Return(Box::new(self.expression(value)?))
            }
            StatementKind::ValDecl { pattern, init } => DefaultStatementKindV1::ValDecl {
                pattern: self.pattern(pattern)?,
                init: Box::new(self.expression(init)?),
            },
            StatementKind::Assign { target, value } => DefaultStatementKindV1::Assign {
                target: Box::new(self.assign_target(target)?),
                value: Box::new(self.expression(value)?),
            },
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => DefaultStatementKindV1::If {
                condition: Box::new(self.expression(cond)?),
                then_body: self.statements(then_body)?,
                else_body: self.optional_statements(else_body.as_deref())?,
            },
            StatementKind::While {
                loop_kind,
                target,
                condition_setup,
                cond,
                body,
            } => {
                self.loops.push(*target);
                let projected: Result<_, super::super::DefaultBodyProjectionError> = (|| {
                    Ok(DefaultStatementKindV1::While {
                        loop_kind: *loop_kind,
                        condition_setup: self.statements(condition_setup)?,
                        condition: Box::new(self.expression(cond)?),
                        body: self.statements(body)?,
                    })
                })(
                );
                self.loops.pop();
                projected?
            }
            StatementKind::Break { target } => {
                DefaultStatementKindV1::Break(self.loop_target_depth("break", *target)?)
            }
            StatementKind::Continue { target } => {
                DefaultStatementKindV1::Continue(self.loop_target_depth("continue", *target)?)
            }
            StatementKind::When(when) => DefaultStatementKindV1::When(Box::new(self.when(when)?)),
            StatementKind::Try(try_) => DefaultStatementKindV1::Try(Box::new(self.try_(try_)?)),
            StatementKind::Throw(value) => {
                DefaultStatementKindV1::Throw(Box::new(self.expression(value)?))
            }
        })
    }

    fn assign_target(
        &mut self,
        target: &crate::AssignTarget,
    ) -> Result<DefaultAssignTargetV1, super::super::DefaultBodyProjectionError> {
        Ok(match target {
            crate::AssignTarget::GenericDelegateStorage(reference) => {
                DefaultAssignTargetV1::GenericDelegateStorage(
                    self.generic_delegate_reference(reference)?,
                )
            }
            crate::AssignTarget::Local(local) => DefaultAssignTargetV1::Local {
                local: self.local(*local)?,
            },
            crate::AssignTarget::Global(global) => DefaultAssignTargetV1::Global {
                property: self.entities.global_property(*global)?,
            },
            crate::AssignTarget::SingletonPublishedRoot(_) => {
                return Err(
                    super::super::DefaultBodyProjectionError::UnsupportedAssignment(
                        "singleton-published-root",
                    ),
                );
            }
            crate::AssignTarget::Index { array, index } => DefaultAssignTargetV1::Index {
                array: Box::new(self.expression(array)?),
                index: Box::new(self.expression(index)?),
            },
            crate::AssignTarget::Field { receiver, field } => DefaultAssignTargetV1::Field {
                receiver: Box::new(self.expression(receiver)?),
                field: self.entities.field(*field, self.binders)?,
            },
            crate::AssignTarget::InitializingClassField { field, origin } => {
                let (receiver, field) =
                    self.initializing_class_field(*field, origin.definition())?;
                DefaultAssignTargetV1::Field {
                    receiver: Box::new(receiver),
                    field,
                }
            }
        })
    }

    fn try_(
        &mut self,
        try_: &crate::Try,
    ) -> Result<DefaultTryV1, super::super::DefaultBodyProjectionError> {
        let body = self.statements(&try_.body)?;

        let mut catches = Vec::with_capacity(try_.catches.len());
        for catch in &try_.catches {
            catches.push(
                DefaultCatchV1::try_new(
                    self.local(catch.local)?,
                    self.type_key(catch.ty)?,
                    self.statements(&catch.body)?,
                    self.span_origin(catch.span)?,
                )
                .map_err(super::super::DefaultBodyProjectionError::ControlFlow)?,
            );
        }
        DefaultTryV1::try_new(
            body,
            catches,
            self.optional_statements(try_.finally_body.as_deref())?,
        )
        .map_err(super::super::DefaultBodyProjectionError::ControlFlow)
    }

    fn optional_statements(
        &mut self,
        statements: Option<&[Statement]>,
    ) -> Result<OptionalDefaultStatementListV1, super::super::DefaultBodyProjectionError> {
        match statements {
            Some(statements) => {
                OptionalDefaultStatementListV1::try_present(self.statements(statements)?)
                    .map_err(super::super::DefaultBodyProjectionError::ControlFlow)
            }
            None => Ok(OptionalDefaultStatementListV1::absent()),
        }
    }

    fn loop_target_depth(
        &self,
        control: &'static str,
        target: crate::LoopId,
    ) -> Result<crate::DefaultLoopTargetV1, super::super::DefaultBodyProjectionError> {
        let depth = self
            .loops
            .iter()
            .rev()
            .position(|active| *active == target)
            .ok_or(
                super::super::DefaultBodyProjectionError::UnknownLoopTarget {
                    control,
                    target: target.into_raw(),
                },
            )?;
        Ok(crate::DefaultLoopTargetV1::new(
            u32::try_from(depth).expect("loop nesting cannot exceed the LoopId space"),
        ))
    }
}
