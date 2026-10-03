//! Statement and structured-control-flow projection.

use crate::{
    DefaultAssignTargetV1, DefaultCatchV1, DefaultStatementKindV1, DefaultStatementV1,
    DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1, DefaultWhenGuardV1, DefaultWhenV1,
    ExhaustivenessProof, OptionalDefaultExpressionV1, OptionalDefaultStatementListV1,
    OptionalDefaultWhenGuardV1, Statement, StatementKind, WhenFallback,
};

use super::BodyProjection;

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
            StatementKind::Return { value } => DefaultStatementKindV1::Return(
                value
                    .as_ref()
                    .map(|value| {
                        self.expression(value)
                            .map(OptionalDefaultExpressionV1::present)
                    })
                    .transpose()?
                    .unwrap_or_else(OptionalDefaultExpressionV1::absent),
            ),
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
                target,
                condition_setup,
                cond,
                body,
            } => {
                self.loops.push(*target);
                let projected: Result<_, super::super::DefaultBodyProjectionError> = (|| {
                    Ok(DefaultStatementKindV1::While {
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
                self.require_innermost_loop("break", *target)?;
                DefaultStatementKindV1::Break
            }
            StatementKind::Continue { target } => {
                self.require_innermost_loop("continue", *target)?;
                DefaultStatementKindV1::Continue
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

    fn when(
        &mut self,
        when: &crate::When,
    ) -> Result<DefaultWhenV1, super::super::DefaultBodyProjectionError> {
        let subject = self.expression(&when.subject)?;

        let mut arms = Vec::with_capacity(when.arms.len());
        for arm in &when.arms {
            let guard = match &arm.guard {
                Some(guard) => OptionalDefaultWhenGuardV1::present(
                    DefaultWhenGuardV1::try_new(
                        self.statements(&guard.setup)?,
                        self.expression(&guard.condition)?,
                    )
                    .map_err(super::super::DefaultBodyProjectionError::ControlFlow)?,
                ),
                None => OptionalDefaultWhenGuardV1::Absent,
            };
            arms.push(
                DefaultWhenArmV1::try_new(
                    self.pattern(&arm.pattern)?,
                    guard,
                    self.statements(&arm.body)?,
                    self.span_origin(arm.span)?,
                )
                .map_err(super::super::DefaultBodyProjectionError::ControlFlow)?,
            );
        }
        DefaultWhenV1::try_new(subject, arms, self.when_fallback(&when.fallback)?)
            .map_err(super::super::DefaultBodyProjectionError::ControlFlow)
    }

    fn when_fallback(
        &mut self,
        fallback: &WhenFallback,
    ) -> Result<DefaultWhenFallbackV1, super::super::DefaultBodyProjectionError> {
        Ok(match fallback {
            WhenFallback::Else(body) => DefaultWhenFallbackV1::try_else(self.statements(body)?)
                .map_err(super::super::DefaultBodyProjectionError::ControlFlow)?,
            WhenFallback::Impossible(ExhaustivenessProof::IrrefutableArm { subject_ty }) => {
                DefaultWhenFallbackV1::irrefutable_arm(self.type_key(*subject_ty)?)
            }
            WhenFallback::Impossible(ExhaustivenessProof::PatternMatrix { subject_ty }) => {
                DefaultWhenFallbackV1::pattern_matrix(self.type_key(*subject_ty)?)
            }
            WhenFallback::Impossible(ExhaustivenessProof::EnumPatternMatrix { subject_ty }) => {
                let subject = self.type_key(*subject_ty)?;
                DefaultWhenFallbackV1::enum_pattern_matrix(subject.clone(), subject)
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

    fn require_innermost_loop(
        &self,
        control: &'static str,
        target: crate::LoopId,
    ) -> Result<(), super::super::DefaultBodyProjectionError> {
        let Some(expected) = self.loops.last() else {
            return Err(super::super::DefaultBodyProjectionError::LoopControlOutsideLoop(control));
        };
        if *expected != target {
            return Err(super::super::DefaultBodyProjectionError::NonInnermostLoop {
                control,
                expected: expected.into_raw(),
                actual: target.into_raw(),
            });
        }
        Ok(())
    }
}
