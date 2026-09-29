use super::*;
mod expression;
mod pattern;

#[derive(Default)]
pub(super) struct LoopRemap {
    active: Vec<(export::LoopId, concrete::LoopId)>,
}

impl Concretizer<'_> {
    pub(super) fn lower_body(
        &mut self,
        source: &export::Body,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Body, Vec<concrete::LocalId>) {
        let (locals, local_map) = self.lower_locals(&source.locals, substitution);
        let statements = self.lower_statement_region(&source.statements, substitution, &local_map);
        (concrete::Body { locals, statements }, local_map)
    }

    pub(super) fn lower_locals(
        &mut self,
        source: &Arena<export::Local>,
        substitution: &[concrete::TypeId],
    ) -> (Arena<concrete::Local>, Vec<concrete::LocalId>) {
        let mut locals = Arena::new();
        let mut local_map = Vec::with_capacity(source.len());
        for (source_id, source_local) in source.iter() {
            let id = locals.alloc(concrete::Local {
                binding: concrete::BindingId::from_raw(source_local.binding.into_raw()),
                selector: source_local.selector.clone(),
                definition: source_local.definition,
                name: source_local.name.clone(),
                ty: self.lower_type(source_local.ty, substitution),
                mutable: source_local.mutable,
            });
            assert_eq!(id.into_raw(), source_id.into_raw());
            local_map.push(id);
        }
        (locals, local_map)
    }

    pub(super) fn lower_statement(
        &mut self,
        source: &export::Statement,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
        out: &mut Vec<concrete::Statement>,
    ) {
        let kind = match &source.kind {
            export::StatementKind::Expr(expr) => {
                concrete::StatementKind::Expr(self.lower_expr(expr, substitution, locals))
            }
            export::StatementKind::InitializationEnsure(unit) => {
                concrete::StatementKind::InitializationEnsure(
                    self.request_initialization_unit(*unit),
                )
            }
            export::StatementKind::GenericDelegateEnsure(reference) => {
                let specialization = self.request_generic_delegate(reference, substitution);
                concrete::StatementKind::InitializationEnsure(
                    self.generic_delegate_specializations[specialization].initialization,
                )
            }
            // This marker has no runtime semantics. Concrete local-function
            // entities are requested by direct calls/references instead.
            export::StatementKind::LocalFunction(_) => return,
            export::StatementKind::Return { value } => {
                let value = value
                    .as_ref()
                    .map(|value| self.lower_expr(value, substitution, locals));
                let value = match value {
                    Some(value)
                        if matches!(self.types[value.ty].kind, concrete::TypeKind::Unit) =>
                    {
                        // A checked generic return may become Unit only after
                        // substitution. Preserve its evaluation before restoring
                        // the LocalConcrete HIR bare-Unit-return invariant.
                        out.push(concrete::Statement {
                            kind: concrete::StatementKind::Expr(value),
                            span: source.span,
                        });
                        None
                    }
                    value => value,
                };
                concrete::StatementKind::Return { value }
            }
            export::StatementKind::ValDecl { pattern, init } => {
                let init = self.lower_expr(init, substitution, locals);
                let pattern = self.lower_pattern(pattern, init.ty, substitution, locals);
                concrete::StatementKind::ValDecl { pattern, init }
            }
            export::StatementKind::Assign { target, value } => concrete::StatementKind::Assign {
                target: self.lower_assign_target(target, substitution, locals),
                value: self.lower_expr(value, substitution, locals),
            },
            export::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => concrete::StatementKind::If {
                cond: self.lower_expr(cond, substitution, locals),
                then_body: self.lower_statements(then_body, substitution, locals, loops),
                else_body: else_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals, loops)),
            },
            export::StatementKind::While {
                target,
                condition_setup,
                cond,
                body,
            } => {
                let mapped_target = self.fresh_loop();
                loops.active.push((*target, mapped_target));
                let condition_setup =
                    self.lower_statements(condition_setup, substitution, locals, loops);
                let cond = self.lower_expr(cond, substitution, locals);
                let body = self.lower_statements(body, substitution, locals, loops);
                assert_eq!(loops.active.pop(), Some((*target, mapped_target)));
                concrete::StatementKind::While {
                    target: mapped_target,
                    condition_setup,
                    cond,
                    body,
                }
            }
            export::StatementKind::Break { target } => concrete::StatementKind::Break {
                target: self.active_loop_target(loops, *target, "break"),
            },
            export::StatementKind::Continue { target } => concrete::StatementKind::Continue {
                target: self.active_loop_target(loops, *target, "continue"),
            },
            export::StatementKind::When(when) => {
                concrete::StatementKind::When(self.lower_when(when, substitution, locals, loops))
            }
            export::StatementKind::Try(try_) => concrete::StatementKind::Try(concrete::Try {
                body: self.lower_statements(&try_.body, substitution, locals, loops),
                catches: try_
                    .catches
                    .iter()
                    .map(|catch| concrete::CatchClause {
                        local: self.lower_local(catch.local, locals),
                        ty: self.lower_type(catch.ty, substitution),
                        body: self.lower_statements(&catch.body, substitution, locals, loops),
                        span: catch.span,
                    })
                    .collect(),
                finally_body: try_
                    .finally_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals, loops)),
            }),
            export::StatementKind::Throw(expr) => {
                concrete::StatementKind::Throw(self.lower_expr(expr, substitution, locals))
            }
        };
        out.push(concrete::Statement {
            kind,
            span: source.span,
        });
    }

    pub(super) fn lower_statements(
        &mut self,
        source: &[export::Statement],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
    ) -> Vec<concrete::Statement> {
        let mut out = Vec::with_capacity(source.len());
        for statement in source {
            self.lower_statement(statement, substitution, locals, loops, &mut out);
        }
        out
    }

    pub(super) fn lower_statement_region(
        &mut self,
        source: &[export::Statement],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Vec<concrete::Statement> {
        let mut loops = LoopRemap::default();
        let statements = self.lower_statements(source, substitution, locals, &mut loops);
        debug_assert!(loops.active.is_empty());
        statements
    }

    fn fresh_loop(&mut self) -> concrete::LoopId {
        let identity = self.next_loop_identity;
        self.next_loop_identity = identity
            .checked_add(1)
            .expect("LocalConcrete HIR loop identity space exhausted");
        concrete::LoopId::from_raw(identity)
    }

    fn active_loop_target(
        &self,
        loops: &LoopRemap,
        source: export::LoopId,
        operation: &str,
    ) -> concrete::LoopId {
        let &(expected, mapped) = loops
            .active
            .last()
            .unwrap_or_else(|| panic!("a concrete {operation} requires an active loop target"));
        assert_eq!(
            expected, source,
            "an unlabelled {operation} targets the innermost loop"
        );
        mapped
    }

    pub(super) fn lower_assign_target(
        &mut self,
        source: &export::AssignTarget,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AssignTarget {
        match source {
            export::AssignTarget::Local(local) => {
                concrete::AssignTarget::Local(self.lower_local(*local, locals))
            }
            export::AssignTarget::Global(global) => {
                concrete::AssignTarget::Global(self.global_map[global])
            }
            export::AssignTarget::GenericDelegateStorage(reference) => {
                let specialization = self.request_generic_delegate(reference, substitution);
                concrete::AssignTarget::Global(
                    self.generic_delegate_specializations[specialization].storage,
                )
            }
            export::AssignTarget::SingletonPublishedRoot(root) => {
                concrete::AssignTarget::SingletonPublishedRoot(self.lower_singleton_root(*root))
            }
            export::AssignTarget::Index { array, index } => concrete::AssignTarget::Index {
                array: Box::new(self.lower_expr(array, substitution, locals)),
                index: Box::new(self.lower_expr(index, substitution, locals)),
            },
            export::AssignTarget::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let field = self.lower_field_ref(*field, substitution);
                concrete::AssignTarget::Field {
                    receiver: Box::new(receiver),
                    field,
                }
            }
            export::AssignTarget::InitializingClassField { field, origin } => {
                let (receiver_ty, field) =
                    self.lower_initializing_class_field(*field, substitution);
                concrete::AssignTarget::Field {
                    receiver: Box::new(concrete::Expr {
                        kind: concrete::ExprKind::ConstructorReceiver,
                        ty: receiver_ty,
                        span: origin.concrete().evaluation.span,
                        origin: origin.concrete(),
                    }),
                    field,
                }
            }
        }
    }

    pub(super) fn lower_when(
        &mut self,
        source: &export::When,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
    ) -> concrete::When {
        let subject = self.lower_expr(&source.subject, substitution, locals);
        let arms = source
            .arms
            .iter()
            .map(|arm| concrete::WhenArm {
                pattern: self.lower_pattern(&arm.pattern, subject.ty, substitution, locals),
                guard: arm.guard.as_ref().map(|guard| concrete::WhenGuard {
                    setup: self.lower_statements(&guard.setup, substitution, locals, loops),
                    condition: self.lower_expr(&guard.condition, substitution, locals),
                }),
                body: self.lower_statements(&arm.body, substitution, locals, loops),
                span: arm.span,
            })
            .collect();
        let fallback = match &source.fallback {
            export::WhenFallback::Else(body) => concrete::WhenFallback::Else(
                self.lower_statements(body, substitution, locals, loops),
            ),
            export::WhenFallback::Impossible(proof) => {
                let proof = match proof {
                    export::ExhaustivenessProof::IrrefutableArm { subject_ty } => {
                        let subject_ty = self.lower_type(*subject_ty, substitution);
                        assert_eq!(
                            subject_ty, subject.ty,
                            "the checked irrefutable proof must match its concrete subject",
                        );
                        concrete::ExhaustivenessProof::IrrefutableArm { subject_ty }
                    }
                    export::ExhaustivenessProof::PatternMatrix { subject_ty } => {
                        let subject_ty = self.lower_type(*subject_ty, substitution);
                        assert_eq!(
                            subject_ty, subject.ty,
                            "the checked pattern-matrix proof must match its concrete subject",
                        );
                        concrete::ExhaustivenessProof::PatternMatrix { subject_ty }
                    }
                    export::ExhaustivenessProof::EnumPatternMatrix {
                        subject_ty,
                        application,
                    } => {
                        let subject_ty = self.lower_type(*subject_ty, substitution);
                        let enum_id = self.lower_enum_application(*application, substitution);
                        assert_eq!(
                            subject_ty, subject.ty,
                            "the checked enum proof must match its concrete subject",
                        );
                        assert_eq!(
                            self.types[subject_ty].kind,
                            concrete::TypeKind::Enum(enum_id),
                            "the checked enum proof must match its concrete subject",
                        );
                        concrete::ExhaustivenessProof::EnumPatternMatrix {
                            subject_ty,
                            enum_id,
                        }
                    }
                };
                concrete::WhenFallback::Impossible(proof)
            }
        };
        concrete::When {
            subject,
            arms,
            fallback,
        }
    }
}
