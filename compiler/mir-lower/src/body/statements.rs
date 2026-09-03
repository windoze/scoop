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
                        let index_slot = self.new_hidden("idx", mir::Type::Int, false);
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
                            index: smir::Expr::local(index_slot, mir::Type::Int),
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
                condition_setup,
                cond,
                body,
            } => {
                self.lower_while(condition_setup, cond, body, span, out);
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
        let out_of_bounds = logic(
            smir::LogicOp::Or,
            smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntLt,
                    lhs: Box::new(smir::Expr::local(index, mir::Type::Int)),
                    rhs: Box::new(smir::Expr::int(0)),
                },
            ),
            smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntGe,
                    lhs: Box::new(smir::Expr::local(index, mir::Type::Int)),
                    rhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::ArrayLen {
                            array_type,
                            operand: Box::new(smir::Expr::local(
                                array,
                                mir::Type::Class(array_type),
                            )),
                        },
                    )),
                },
            ),
        );
        let throw = self.throw_builtin(
            self.module.exception_core.index_out_of_bounds_exception,
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
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(pattern, slot, &mut path, &ty, &mut bindings);
        // hir-lower only emits irrefutable patterns (tuple / struct /
        // binding / wildcard) in destructuring declarations.
        debug_assert!(cond.is_none(), "destructuring patterns are irrefutable");
        for (local, init) in bindings {
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
        }
    }

    pub(super) fn lower_while(
        &mut self,
        condition_setup: &[hir::Statement],
        cond: &hir::Expr,
        body: &[hir::Statement],
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        let mut initial_setup = self.lower_statements(condition_setup);
        let initial_cond = self.lower_expr(cond);
        initial_setup.extend(
            std::mem::take(&mut self.prelude)
                .into_iter()
                .map(|kind| smir::Statement { kind, span }),
        );
        if initial_setup.is_empty() {
            let body = self.lower_statements(body);
            out.push(smir::Statement {
                kind: smir::StatementKind::While {
                    cond: initial_cond,
                    body,
                },
                span,
            });
            return;
        }
        // The condition contains explicit HIR setup and/or an expression
        // prelude such as a trap test (`!!`), all of which must run on every
        // iteration:
        // `P; while (C) B` becomes `P; var $c = C; while ($c) { B; P;
        // $c = C }`.
        out.extend(initial_setup);
        let cond_local = self.new_hidden("cond", mir::Type::Boolean, true);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl {
                local: cond_local,
                init: initial_cond,
            },
            span,
        });
        let mut body = self.lower_statements(body);
        let mut repeated_setup = self.lower_statements(condition_setup);
        let repeated_cond = self.lower_expr(cond);
        repeated_setup.extend(
            std::mem::take(&mut self.prelude)
                .into_iter()
                .map(|kind| smir::Statement { kind, span }),
        );
        body.extend(repeated_setup);
        body.push(smir::Statement {
            kind: smir::StatementKind::Assign {
                local: cond_local,
                value: repeated_cond,
            },
            span,
        });
        out.push(smir::Statement {
            kind: smir::StatementKind::While {
                cond: smir::Expr::local(cond_local, mir::Type::Boolean),
                body,
            },
            span,
        });
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
        let mut chain =
            self.lower_arms(&when.arms, subject, &subject_ty, when.else_body.as_deref());
        out.append(&mut chain);
    }

    /// Lower `arms` into the decision sequence: each arm is
    /// `if (<pattern condition>) { <bindings>; [if (<guard>) <body>
    /// else <next>] } else <next>` — a failed guard falls through to
    /// the next arm. With no guard the arm body is the then branch
    /// directly; an unconditionally matching arm (binding / wildcard,
    /// no guard) is inlined and makes the remaining arms unreachable
    /// (hir-lower rejects those). Exhaustiveness was checked at HIR,
    /// so the innermost else can only be reached via `else_body`.
    pub(super) fn lower_arms(
        &mut self,
        arms: &[hir::WhenArm],
        subject: mir::LocalId,
        subject_ty: &mir::Type,
        else_body: Option<&[hir::Statement]>,
    ) -> Vec<smir::Statement> {
        let Some((arm, rest)) = arms.split_first() else {
            return else_body
                .map(|body| self.lower_statements(body))
                .unwrap_or_default();
        };
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(&arm.pattern, subject, &mut path, subject_ty, &mut bindings);
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
            let next = self.lower_arms(rest, subject, subject_ty, else_body);
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
        if rest.is_empty() && else_body.is_none() && arm.guard.is_none() {
            // HIR has already proved the complete arm sequence exhaustive.
            // Reaching its final unguarded arm therefore proves this pattern,
            // even when the pattern itself is refutable in isolation. Keeping
            // an impossible false edge would make values defined by every arm
            // appear live before their definitions at a suspend site.
            return then;
        }
        let Some(cond) = cond else {
            // Matches unconditionally; `rest` is unreachable.
            return then;
        };
        let next = self.lower_arms(rest, subject, subject_ty, else_body);
        vec![smir::Statement {
            kind: smir::StatementKind::If {
                cond,
                then_body: then,
                else_body: non_empty(next),
            },
            span: arm.span,
        }]
    }
}
