use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_statements(
        &mut self,
        statements: &[hir::Statement],
    ) -> Vec<smir::Statement> {
        let mut out = Vec::new();
        for statement in statements {
            self.lower_statement(statement, &mut out);
        }
        out
    }

    pub(super) fn lower_statement(
        &mut self,
        statement: &hir::Statement,
        out: &mut Vec<smir::Statement>,
    ) {
        let span = statement.span;
        let kind = match &statement.kind {
            hir::StatementKind::LocalFunction(_) => return,
            hir::StatementKind::InitializationEnsure {
                unit,
                cycle_exception: _,
            } => {
                let function = self.module.initialization_units[*unit].ensure;
                smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::User(self.function_map[&function]),
                        },
                        args: Vec::new(),
                        return_ty: mir::Type::Unit,
                    }),
                ))
            }
            hir::StatementKind::Expr(expr) => {
                let expr = self.lower_expr(expr);
                self.drain_prelude(span, out);
                smir::StatementKind::Expr(expr)
            }
            hir::StatementKind::Return { value } => {
                let value = value.as_ref().map(|value| self.lower_expr(value));
                self.drain_prelude(span, out);
                smir::StatementKind::Return { value }
            }
            hir::StatementKind::Break { target } => smir::StatementKind::Break {
                target: self.active_loop(*target),
            },
            hir::StatementKind::Continue { target } => smir::StatementKind::Continue {
                target: self.active_loop(*target),
            },
            hir::StatementKind::ValDecl { pattern, init } => {
                self.lower_val_decl(pattern, init, span, out);
                return;
            }
            hir::StatementKind::Assign { target, value } => {
                let kind = match target {
                    hir::AssignTarget::Local(local) => {
                        let local = self.local_map[local];
                        let value = self.lower_expr(value);
                        smir::StatementKind::Assign { local, value }
                    }
                    hir::AssignTarget::Global(global) => smir::StatementKind::GlobalAssign {
                        global: self.global_map[global],
                        value: self.lower_expr(value),
                    },
                    hir::AssignTarget::SingletonPublishedRoot(root) => {
                        let root = self.singleton_root_map[root];
                        smir::StatementKind::GlobalAssign {
                            global: self.singleton_published_roots[root].global,
                            value: self.lower_expr(value),
                        }
                    }
                    // `m[i] = v` (only `MutableArray`, checked at HIR).
                    // M8: the bounds check moved here from codegen —
                    // the array and the index are evaluated once into
                    // hidden locals and checked before the store; the
                    // value expression stays inside the `ArraySet`
                    // node and is evaluated after the check.
                    hir::AssignTarget::Index { array, index } => {
                        let array_ty = self.lower_type(array.ty);
                        let mir::Type::Class(array_type) = array_ty else {
                            unreachable!("an array assignment has an intrinsic class type")
                        };
                        let array_slot =
                            self.new_hidden("arr", mir::Type::Class(array_type), false);
                        let index_ty = mir::Type::Integer(mir::IntegerKind::SIGNED_64);
                        let index_slot = self.new_hidden("idx", index_ty.clone(), false);
                        let array_value = self.lower_expr(array);
                        self.prelude.push(smir::StatementKind::ValDecl {
                            local: array_slot,
                            init: array_value,
                        });
                        let index_value = self.lower_expr(index);
                        self.prelude.push(smir::StatementKind::ValDecl {
                            local: index_slot,
                            init: index_value,
                        });
                        self.bounds_check(array_type, array_slot, index_slot, span);
                        let value = self.lower_expr(value);
                        smir::StatementKind::ArraySet {
                            array_type,
                            array: smir::Expr::local(array_slot, mir::Type::Class(array_type)),
                            index: smir::Expr::local(index_slot, index_ty),
                            value,
                        }
                    }
                    // `obj.field = v` (only `var` properties of
                    // classes, checked at HIR); the index is the
                    // flattened field index.
                    hir::AssignTarget::Field { receiver, field } => {
                        let hir::FieldRef::ClassField { index, .. } = field else {
                            unreachable!("hir-lower only allows assignment to class properties")
                        };
                        let object = self.lower_expr(receiver);
                        let value = self.lower_expr(value);
                        smir::StatementKind::FieldSet {
                            object,
                            index: *index,
                            value,
                        }
                    }
                };
                self.drain_prelude(span, out);
                kind
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let cond = self.lower_expr(cond);
                self.drain_prelude(span, out);
                let then_body = self.lower_statements(then_body);
                let else_body = else_body.as_ref().map(|body| self.lower_statements(body));
                smir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                }
            }
            hir::StatementKind::While {
                target,
                condition_setup,
                cond,
                body,
            } => {
                self.lower_while(*target, condition_setup, cond, body, span, out);
                return;
            }
            hir::StatementKind::When(when) => {
                self.lower_when(when, span, out);
                return;
            }
            // `try` / `catch` / `finally` stays structured in MIR
            // (M8, DESIGN 3.3); the control-flow expansion (invoke /
            // landingpad) is LIR's job.
            hir::StatementKind::Try(try_) => smir::StatementKind::Try(self.lower_try(try_)),
            hir::StatementKind::Throw(expr) => {
                let value = self.lower_expr(expr);
                self.drain_prelude(span, out);
                smir::StatementKind::Throw(value)
            }
        };
        out.push(smir::Statement { kind, span });
    }

    /// `try` translates one-to-one: body, ordered catches (the catch
    /// type is resolved to the concrete MIR type), and the optional
    /// finally body.
    pub(super) fn lower_try(&mut self, try_: &hir::Try) -> smir::Try {
        let body = self.lower_statements(&try_.body);
        let catches = try_
            .catches
            .iter()
            .map(|catch| smir::CatchClause {
                local: self.local_map[&catch.local],
                ty: Box::new(self.lower_type(catch.ty)),
                body: self.lower_statements(&catch.body),
                span: catch.span,
            })
            .collect();
        let finally_body = try_
            .finally_body
            .as_ref()
            .map(|body| self.lower_statements(body));
        smir::Try {
            body,
            catches,
            finally_body,
        }
    }

    /// Construct and throw one compiler-known exception. The zero-argument
    /// constructor target is complete in LocalConcrete HIR, so this operation
    /// only transposes typed identities.
    pub(super) fn throw_builtin(
        &mut self,
        exception: hir::CompilerException,
        span: Span,
    ) -> smir::Statement {
        let constructor = &self.module.class_constructors[exception.callable()];
        debug_assert!(constructor.parameters.is_empty());
        let ctor = self.ctors[&exception.callable()];
        let class_id = self.class_map[&constructor.class];
        let exception_ty = mir::Type::Class(class_id);
        smir::Statement {
            kind: smir::StatementKind::Throw(smir::Expr::new(
                exception_ty.clone(),
                smir::ExprKind::ClassNew {
                    class_id,
                    initializer: ctor,
                    args: Vec::new(),
                },
            )),
            span,
        }
    }

    /// The M8 array bounds check (DESIGN section 1), shared by
    /// `ArrayGet` and `ArraySet`:
    /// `if (index < 0 || index >= array.size) throw IndexOutOfBoundsException()`.
    /// CFG normalization expands the `||` into branch edges.
    pub(super) fn bounds_check(
        &mut self,
        array_type: mir::ClassId,
        array: mir::LocalId,
        index: mir::LocalId,
        span: Span,
    ) {
        let index_kind = mir::IntegerKind::SIGNED_64;
        let index_ty = mir::Type::Integer(index_kind);
        let out_of_bounds = logic(
            smir::LogicOp::Or,
            smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(
                    index_kind,
                    mir::IntegerComparisonOperator::LessThan,
                ),
                smir::Expr::local(index, index_ty.clone()),
                smir::Expr::integer(mir::MirIntegerConstant::Signed64(0)),
            ),
            smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(
                    index_kind,
                    mir::IntegerComparisonOperator::GreaterThanOrEqual,
                ),
                smir::Expr::local(index, index_ty.clone()),
                smir::Expr::new(
                    index_ty,
                    smir::ExprKind::ArrayLen {
                        array_type,
                        operand: Box::new(smir::Expr::local(array, mir::Type::Class(array_type))),
                    },
                ),
            ),
        );
        let throw = self.throw_builtin(
            self.module
                .core_protocols
                .exceptions
                .index_out_of_bounds_exception,
            span,
        );
        self.prelude.push(smir::StatementKind::If {
            cond: out_of_bounds,
            then_body: vec![throw],
            else_body: None,
        });
    }

    /// A `val` declaration: either the plain M1–M3 binding form, or a
    /// destructuring declaration (spec 4.6) whose init value is
    /// evaluated once into a hidden local that the pattern's bindings
    /// extract from.
    pub(super) fn lower_val_decl(
        &mut self,
        pattern: &hir::Pattern,
        init: &hir::Expr,
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        if let hir::Pattern::Binding { local } = pattern {
            let local = self.local_map[local];
            let init = self.lower_expr(init);
            self.drain_prelude(span, out);
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
            return;
        }
        let ty = self.lower_type(init.ty);
        let init = self.lower_expr(init);
        self.drain_prelude(span, out);
        let slot = self.new_hidden("bind", ty.clone(), false);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl { local: slot, init },
            span,
        });
        let mut path = Vec::new();
        let mut steps = Vec::new();
        let mut bindings = Vec::new();
        self.lower_pattern(pattern, slot, &mut path, &ty, &mut steps, &mut bindings);
        // hir-lower only emits irrefutable patterns (tuple / struct /
        // binding / wildcard) in destructuring declarations.
        debug_assert!(steps.is_empty(), "destructuring patterns are irrefutable");
        for (local, init) in bindings {
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
        }
    }

    pub(super) fn lower_while(
        &mut self,
        target: hir::LoopId,
        condition_setup: &[hir::Statement],
        cond: &hir::Expr,
        body: &[hir::Statement],
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        let loop_id = smir::LoopId::from_raw(self.next_loop_id);
        self.next_loop_id = self
            .next_loop_id
            .checked_add(1)
            .expect("one callable cannot contain u32::MAX structured loops");
        assert!(
            self.active_loops
                .iter()
                .all(|(active, _)| *active != target),
            "a concrete HIR loop identity is established exactly once on its active path"
        );
        self.active_loops.push((target, loop_id));
        let mut condition_setup = self.lower_statements(condition_setup);
        let cond = self.lower_expr(cond);
        self.drain_prelude(span, &mut condition_setup);
        let body = self.lower_statements(body);
        let popped = self.active_loops.pop();
        assert_eq!(
            popped,
            Some((target, loop_id)),
            "structured loop remapping is lexically nested"
        );
        out.push(smir::Statement {
            kind: smir::StatementKind::While {
                target: loop_id,
                condition_setup,
                cond,
                body,
            },
            span,
        });
    }

    fn active_loop(&self, target: hir::LoopId) -> smir::LoopId {
        let (source, lowered) = self
            .active_loops
            .last()
            .expect("concrete HIR binds every break/continue inside an active loop");
        assert_eq!(
            *source, target,
            "unlabelled break/continue targets the lexical innermost loop"
        );
        *lowered
    }

    /// `when` becomes a decision sequence (DESIGN 3.3): the subject is
    /// evaluated once into a hidden local, then the arms chain if/else
    /// tests; the `else` arm is the fallback.
    pub(super) fn lower_when(
        &mut self,
        when: &hir::When,
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        if let hir::WhenFallback::Impossible(proof) = &when.fallback {
            let proof_subject = match proof {
                hir::ExhaustivenessProof::IrrefutableArm { subject_ty } => *subject_ty,
                hir::ExhaustivenessProof::PatternMatrix { subject_ty } => *subject_ty,
                hir::ExhaustivenessProof::EnumPatternMatrix {
                    subject_ty,
                    enum_id,
                } => {
                    assert_eq!(
                        self.module.types[*subject_ty].kind,
                        hir::TypeKind::Enum(*enum_id),
                        "an enum exhaustiveness proof must name its subject enum",
                    );
                    *subject_ty
                }
            };
            assert_eq!(
                proof_subject, when.subject.ty,
                "an exhaustiveness proof must match its when subject",
            );
        }
        let subject_ty = self.lower_type(when.subject.ty);
        let subject_init = self.lower_expr(&when.subject);
        self.drain_prelude(span, out);
        let subject = self.new_hidden("when", subject_ty.clone(), false);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl {
                local: subject,
                init: subject_init,
            },
            span,
        });
        let mut chain = self.lower_arms(&when.arms, subject, &subject_ty, &when.fallback, span);
        out.append(&mut chain);
    }

    /// Lower `arms` into ordered pattern decisions. A failed test or guard
    /// falls through to the next arm. Bindings are declared only after the
    /// complete pattern succeeds. HIR exhaustiveness controls only the final
    /// false edge; it never suppresses a refutable pattern's runtime tests.
    pub(super) fn lower_arms(
        &mut self,
        arms: &[hir::WhenArm],
        subject: mir::LocalId,
        subject_ty: &mir::Type,
        fallback: &hir::WhenFallback,
        fallback_span: Span,
    ) -> Vec<smir::Statement> {
        let Some((arm, rest)) = arms.split_first() else {
            return match fallback {
                hir::WhenFallback::Else(body) => self.lower_statements(body),
                hir::WhenFallback::Impossible(_) => vec![smir::Statement {
                    kind: smir::StatementKind::Unreachable,
                    span: fallback_span,
                }],
            };
        };
        let mut path = Vec::new();
        let mut steps = Vec::new();
        let mut bindings = Vec::new();
        let decision_subject = if pattern_requires_stable_variant_subject(&arm.pattern) {
            self.new_hidden("pattern.subject", subject_ty.clone(), false)
        } else {
            subject
        };
        self.lower_pattern(
            &arm.pattern,
            decision_subject,
            &mut path,
            subject_ty,
            &mut steps,
            &mut bindings,
        );
        if decision_subject != subject {
            assert!(
                !steps.is_empty(),
                "a refutable pattern emits a runtime test"
            );
            steps.insert(
                0,
                smir::PatternDecisionStep::Materialize {
                    local: decision_subject,
                    init: smir::Expr::local(subject, subject_ty.clone()),
                },
            );
        }
        let mut then: Vec<smir::Statement> = bindings
            .into_iter()
            .map(|(local, init)| smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span: arm.span,
            })
            .collect();
        if let Some(guard) = &arm.guard {
            then.extend(self.lower_statements(&guard.setup));
            let guard_cond = self.lower_expr(&guard.condition);
            let guard_prelude = std::mem::take(&mut self.prelude);
            then.extend(guard_prelude.into_iter().map(|kind| smir::Statement {
                kind,
                span: arm.span,
            }));
            let body = self.lower_statements(&arm.body);
            let next = self.lower_arms(rest, subject, subject_ty, fallback, fallback_span);
            then.push(smir::Statement {
                kind: smir::StatementKind::If {
                    cond: guard_cond,
                    then_body: body,
                    else_body: non_empty(next),
                },
                span: arm.span,
            });
        } else {
            then.extend(self.lower_statements(&arm.body));
        }
        if steps.is_empty() {
            // Matches unconditionally; `rest` is unreachable.
            return then;
        }
        let next = self.lower_arms(rest, subject, subject_ty, fallback, fallback_span);
        vec![smir::Statement {
            kind: smir::StatementKind::PatternDecision(smir::PatternDecision {
                steps,
                then_body: then,
                else_body: next,
            }),
            span: arm.span,
        }]
    }
}
