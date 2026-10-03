use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_statement(
        &mut self,
        source: &hir::Statement,
        context: &mut InstantiationContext,
    ) -> hir::Statement {
        let kind = match &source.kind {
            hir::StatementKind::GenericDelegateEnsure(reference) => {
                hir::StatementKind::GenericDelegateEnsure(
                    self.instantiate_delegate_reference(reference, context),
                )
            }
            hir::StatementKind::InitializationEnsure(unit) => {
                hir::StatementKind::InitializationEnsure(*unit)
            }
            hir::StatementKind::Expr(value) => {
                hir::StatementKind::Expr(self.instantiate_default_expr(value, context))
            }
            hir::StatementKind::LocalFunction(function) => hir::StatementKind::LocalFunction(
                self.instantiate_default_local_function(*function, context),
            ),
            hir::StatementKind::Return { value } => hir::StatementKind::Return {
                value: value
                    .as_ref()
                    .map(|value| self.instantiate_default_expr(value, context)),
            },
            hir::StatementKind::ValDecl { pattern, init } => hir::StatementKind::ValDecl {
                pattern: self.instantiate_default_pattern(pattern, context),
                init: self.instantiate_default_expr(init, context),
            },
            hir::StatementKind::Assign { target, value } => hir::StatementKind::Assign {
                target: self.instantiate_default_assign_target(target, context),
                value: self.instantiate_default_expr(value, context),
            },
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => hir::StatementKind::If {
                cond: self.instantiate_default_expr(cond, context),
                then_body: then_body
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
                else_body: else_body.as_ref().map(|body| {
                    body.iter()
                        .map(|statement| self.instantiate_default_statement(statement, context))
                        .collect()
                }),
            },
            hir::StatementKind::While {
                target,
                condition_setup,
                cond,
                body,
            } => {
                let mapped_target = self.fresh_loop();
                context.loop_targets.push((*target, mapped_target));
                let condition_setup = condition_setup
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect();
                let cond = self.instantiate_default_expr(cond, context);
                let body = body
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect();
                assert_eq!(context.loop_targets.pop(), Some((*target, mapped_target)));
                hir::StatementKind::While {
                    target: mapped_target,
                    condition_setup,
                    cond,
                    body,
                }
            }
            hir::StatementKind::Break { target } => {
                let &(source, mapped) = context
                    .loop_targets
                    .last()
                    .expect("a default-template break has an active loop target");
                assert_eq!(
                    source, *target,
                    "an unlabelled break targets the innermost loop"
                );
                hir::StatementKind::Break { target: mapped }
            }
            hir::StatementKind::Continue { target } => {
                let &(source, mapped) = context
                    .loop_targets
                    .last()
                    .expect("a default-template continue has an active loop target");
                assert_eq!(
                    source, *target,
                    "an unlabelled continue targets the innermost loop"
                );
                hir::StatementKind::Continue { target: mapped }
            }
            hir::StatementKind::When(when) => hir::StatementKind::When(hir::When {
                subject: self.instantiate_default_expr(&when.subject, context),
                arms: when
                    .arms
                    .iter()
                    .map(|arm| hir::WhenArm {
                        pattern: self.instantiate_default_pattern(&arm.pattern, context),
                        guard: arm.guard.as_ref().map(|guard| hir::WhenGuard {
                            setup: guard
                                .setup
                                .iter()
                                .map(|statement| {
                                    self.instantiate_default_statement(statement, context)
                                })
                                .collect(),
                            condition: self.instantiate_default_expr(&guard.condition, context),
                        }),
                        body: arm
                            .body
                            .iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                        span: context.statement_span,
                    })
                    .collect(),
                fallback: match &when.fallback {
                    hir::WhenFallback::Else(body) => hir::WhenFallback::Else(
                        body.iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                    ),
                    hir::WhenFallback::Impossible(proof) => {
                        let proof = match proof {
                            hir::ExhaustivenessProof::IrrefutableArm { subject_ty } => {
                                hir::ExhaustivenessProof::IrrefutableArm {
                                    subject_ty: self
                                        .instantiate_method_ty(*subject_ty, &context.bindings),
                                }
                            }
                            hir::ExhaustivenessProof::PatternMatrix { subject_ty } => {
                                hir::ExhaustivenessProof::PatternMatrix {
                                    subject_ty: self
                                        .instantiate_method_ty(*subject_ty, &context.bindings),
                                }
                            }
                            hir::ExhaustivenessProof::EnumPatternMatrix { subject_ty } => {
                                hir::ExhaustivenessProof::EnumPatternMatrix {
                                    subject_ty: self
                                        .instantiate_method_ty(*subject_ty, &context.bindings),
                                }
                            }
                        };
                        hir::WhenFallback::Impossible(proof)
                    }
                },
            }),
            hir::StatementKind::Try(value) => hir::StatementKind::Try(hir::Try {
                body: value
                    .body
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
                catches: value
                    .catches
                    .iter()
                    .map(|catch| hir::CatchClause {
                        local: mapped_local(context, catch.local),
                        ty: self.instantiate_method_ty(catch.ty, &context.bindings),
                        body: catch
                            .body
                            .iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                        span: context.statement_span,
                    })
                    .collect(),
                finally_body: value.finally_body.as_ref().map(|body| {
                    body.iter()
                        .map(|statement| self.instantiate_default_statement(statement, context))
                        .collect()
                }),
            }),
            hir::StatementKind::Throw(value) => {
                hir::StatementKind::Throw(self.instantiate_default_expr(value, context))
            }
        };
        hir::Statement {
            kind,
            span: context.statement_span,
        }
    }
}
