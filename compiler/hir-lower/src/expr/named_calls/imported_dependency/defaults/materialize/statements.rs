use scoop_hir as hir;
use scoop_identity::LocalValueSelector;

use super::{ImportedDefaultContext, ImportedDefaultMaterializationError};
use crate::Lowerer;

impl Lowerer {
    pub(super) fn materialize_imported_default_statement(
        &mut self,
        source: &hir::DefaultStatementV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::Statement, ImportedDefaultMaterializationError> {
        use hir::DefaultStatementKindV1 as Kind;
        let kind = match source.kind() {
            Kind::ContextScope { value, body } => hir::StatementKind::ContextScope {
                value: self.materialize_imported_default_expression(value, context)?,
                body: body
                    .iter()
                    .map(|statement| {
                        self.materialize_imported_default_statement(statement, context)
                    })
                    .collect::<Result<_, _>>()?,
            },
            Kind::GenericDelegateEnsure(reference) => hir::StatementKind::GenericDelegateEnsure(
                self.materialize_imported_delegate_reference(reference, context)?,
            ),
            Kind::LocalFunction(descriptor) => {
                let origin =
                    self.imported_default_definition_origin(source.definition_origin(), context)?;
                hir::StatementKind::LocalFunction(
                    self.materialize_imported_local_declaration(descriptor, origin, context)?,
                )
            }
            Kind::Expr(value) => hir::StatementKind::Expr(
                self.materialize_imported_default_expression(value, context)?,
            ),
            Kind::Return(value) => hir::StatementKind::Return {
                value: value
                    .as_ref()
                    .map(|value| self.materialize_imported_default_expression(value, context))
                    .transpose()?,
            },
            Kind::ValDecl { pattern, init } => hir::StatementKind::ValDecl {
                pattern: self.materialize_imported_default_pattern(pattern, context)?,
                init: self.materialize_imported_default_expression(init, context)?,
            },
            Kind::Assign { target, value } => hir::StatementKind::Assign {
                target: self.materialize_imported_default_assign_target(target, context)?,
                value: self.materialize_imported_default_expression(value, context)?,
            },
            Kind::If {
                condition,
                then_body,
                else_body,
            } => hir::StatementKind::If {
                cond: self.materialize_imported_default_expression(condition, context)?,
                then_body: self.materialize_imported_default_statements(then_body, context)?,
                else_body: match else_body.view() {
                    hir::OptionalDefaultStatementListViewV1::Absent => None,
                    hir::OptionalDefaultStatementListViewV1::Present(body) => {
                        Some(self.materialize_imported_default_statements(body, context)?)
                    }
                },
            },
            Kind::While {
                condition_setup,
                condition,
                body,
            } => {
                let target = self.fresh_loop();
                context.loop_targets.push(target);
                let materialized: Result<_, ImportedDefaultMaterializationError> = (|| {
                    Ok(hir::StatementKind::While {
                        target,
                        condition_setup: self
                            .materialize_imported_default_statements(condition_setup, context)?,
                        cond: self.materialize_imported_default_expression(condition, context)?,
                        body: self.materialize_imported_default_statements(body, context)?,
                    })
                })(
                );
                let popped = context.loop_targets.pop();
                debug_assert_eq!(popped, Some(target));
                materialized?
            }
            Kind::Break => hir::StatementKind::Break {
                target: context.loop_targets.last().copied().ok_or(
                    ImportedDefaultMaterializationError::InvalidControlFlow(
                        "break without an enclosing loop",
                    ),
                )?,
            },
            Kind::Continue => hir::StatementKind::Continue {
                target: context.loop_targets.last().copied().ok_or(
                    ImportedDefaultMaterializationError::InvalidControlFlow(
                        "continue without an enclosing loop",
                    ),
                )?,
            },
            Kind::When(value) => {
                hir::StatementKind::When(self.materialize_imported_default_when(value, context)?)
            }
            Kind::Try(value) => {
                hir::StatementKind::Try(self.materialize_imported_default_try(value, context)?)
            }
            Kind::Throw(value) => hir::StatementKind::Throw(
                self.materialize_imported_default_expression(value, context)?,
            ),
            Kind::InitializationEnsure(_) => {
                return Err(ImportedDefaultMaterializationError::Plan(
                    "preflight admitted an unsupported dependency default statement".to_owned(),
                ));
            }
        };
        let span = self.imported_statement_span(source.definition_origin(), context)?;
        Ok(hir::Statement { kind, span })
    }

    fn imported_statement_span(
        &mut self,
        source: &hir::ExportDefinitionSourceV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<scoop_ast::Span, ImportedDefaultMaterializationError> {
        Ok(self
            .imported_default_definition_origin(source, context)?
            .span)
    }

    fn materialize_imported_default_statements(
        &mut self,
        statements: &[hir::DefaultStatementV1],
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<Vec<hir::Statement>, ImportedDefaultMaterializationError> {
        statements
            .iter()
            .map(|statement| self.materialize_imported_default_statement(statement, context))
            .collect()
    }

    fn materialize_imported_default_try(
        &mut self,
        value: &hir::DefaultTryV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::Try, ImportedDefaultMaterializationError> {
        let body = self.materialize_imported_default_statements(value.body(), context)?;
        let catches = value
            .catches()
            .iter()
            .map(|catch| {
                Ok(hir::CatchClause {
                    local: self.materialized_imported_default_local(catch.local(), context)?,
                    ty: self.materialize_imported_default_type(catch.value_type(), context)?,
                    body: self.materialize_imported_default_statements(catch.body(), context)?,
                    span: self.imported_statement_span(catch.definition_origin(), context)?,
                })
            })
            .collect::<Result<_, ImportedDefaultMaterializationError>>()?;
        let finally_body = match value.finally_body().view() {
            hir::OptionalDefaultStatementListViewV1::Absent => None,
            hir::OptionalDefaultStatementListViewV1::Present(body) => {
                Some(self.materialize_imported_default_statements(body, context)?)
            }
        };
        Ok(hir::Try {
            body,
            catches,
            finally_body,
        })
    }

    fn materialize_imported_default_assign_target(
        &mut self,
        target: &hir::DefaultAssignTargetV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::AssignTarget, ImportedDefaultMaterializationError> {
        match target {
            hir::DefaultAssignTargetV1::GenericDelegateStorage(reference) => {
                Ok(hir::AssignTarget::GenericDelegateStorage(
                    self.materialize_imported_delegate_reference(reference, context)?,
                ))
            }
            hir::DefaultAssignTargetV1::Local { local } => self
                .materialized_imported_default_local(local, context)
                .map(hir::AssignTarget::Local),
            hir::DefaultAssignTargetV1::Field { receiver, field } => Ok(hir::AssignTarget::Field {
                receiver: Box::new(
                    self.materialize_imported_default_expression(receiver, context)?,
                ),
                field: self.materialize_imported_field_ref(field, context.bindings)?,
            }),
            hir::DefaultAssignTargetV1::Global { .. }
            | hir::DefaultAssignTargetV1::Index { .. } => {
                Err(ImportedDefaultMaterializationError::Plan(
                    "preflight admitted an unsupported dependency default assignment".to_owned(),
                ))
            }
        }
    }

    fn materialize_imported_default_when(
        &mut self,
        value: &hir::DefaultWhenV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::When, ImportedDefaultMaterializationError> {
        let subject = value
            .subject()
            .map(|subject| self.materialize_imported_default_expression(subject, context))
            .transpose()?;
        let mut arms = Vec::with_capacity(value.arms().len());
        for arm in value.arms() {
            let condition = match arm.condition() {
                hir::DefaultWhenConditionV1::Case(pattern) => hir::WhenCondition::Case(
                    self.materialize_imported_default_pattern(pattern, context)?,
                ),
                hir::DefaultWhenConditionV1::Predicate(condition) => {
                    hir::WhenCondition::Predicate(hir::WhenGuard {
                        setup: self
                            .materialize_imported_default_statements(condition.setup(), context)?,
                        condition: self.materialize_imported_default_expression(
                            condition.condition(),
                            context,
                        )?,
                    })
                }
                hir::DefaultWhenConditionV1::Always => hir::WhenCondition::Always,
            };
            let guard = if let Some(guard) = arm.guard().as_ref() {
                Some(hir::WhenGuard {
                    setup: self.materialize_imported_default_statements(guard.setup(), context)?,
                    condition: self
                        .materialize_imported_default_expression(guard.condition(), context)?,
                })
            } else {
                None
            };
            let body = self.materialize_imported_default_statements(arm.body(), context)?;
            let span = self.imported_statement_span(arm.definition_origin(), context)?;
            arms.push(hir::WhenArm {
                condition,
                guard,
                body,
                span,
            });
        }
        let fallback = match value.fallback().view() {
            hir::DefaultWhenFallbackViewV1::Fallthrough => hir::WhenFallback::Fallthrough,
            hir::DefaultWhenFallbackViewV1::Else(body) => hir::WhenFallback::Else(
                self.materialize_imported_default_statements(body, context)?,
            ),
            hir::DefaultWhenFallbackViewV1::IrrefutableArm { subject_type } => {
                hir::WhenFallback::Impossible(hir::ExhaustivenessProof::IrrefutableArm {
                    subject_ty: self.materialize_imported_default_type(subject_type, context)?,
                })
            }
            hir::DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => {
                hir::WhenFallback::Impossible(hir::ExhaustivenessProof::PatternMatrix {
                    subject_ty: self.materialize_imported_default_type(subject_type, context)?,
                })
            }
            hir::DefaultWhenFallbackViewV1::EnumPatternMatrix { subject_type, .. } => {
                hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix {
                    subject_ty: self.materialize_imported_default_type(subject_type, context)?,
                })
            }
        };
        Ok(hir::When {
            subject,
            arms,
            fallback,
        })
    }

    pub(super) fn materialize_imported_default_type(
        &mut self,
        source: &scoop_identity::SignatureTypeKey,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::TypeId, ImportedDefaultMaterializationError> {
        self.imported_default_type_with_bindings(source, context.bindings)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))
    }

    pub(super) fn materialized_imported_default_local(
        &self,
        source: &LocalValueSelector,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::LocalId, ImportedDefaultMaterializationError> {
        let value = context
            .locals
            .get(source)
            .ok_or_else(|| ImportedDefaultMaterializationError::UnknownLocal(source.clone()))?;
        match value.kind {
            hir::ExprKind::Local(local) => Ok(local),
            _ => {
                Err(ImportedDefaultMaterializationError::ExpectedMaterializedLocal(source.clone()))
            }
        }
    }
}
