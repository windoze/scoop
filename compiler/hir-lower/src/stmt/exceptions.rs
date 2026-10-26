use super::*;

mod catches;

struct ValueCatch {
    local: hir::LocalId,
    ty: TypeId,
    body: ValueBlock,
    span: Span,
}

impl Lowerer {
    /// Throw operands retain their actual type and use the ordinary exception
    /// transfer. An operand that cannot complete never reaches the transfer.
    pub(crate) fn lower_throw(
        &mut self,
        expr: &ast::Expr,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let value = self.lower_expr(expr, &mut sink, None)?;
        let throwable = self.throwable_ty(expr.span())?;
        if !self.is_subtype(value.ty, throwable) {
            let found = self.type_name(value.ty);
            self.error(
                expr.span(),
                format!("cannot throw value of type {found}: not a subtype of Throwable"),
            );
            return None;
        }
        // The operand is evaluated right before the `throw`, so
        // desugaring statements belong before it.
        out.extend(sink);
        Some(hir::StatementKind::Throw(value))
    }

    /// `try { } catch (e: T) { } finally { }` (spec 11.7, milestone8
    /// DESIGN.md 3.2). Every catch parameter type must be a subtype of
    /// `Throwable`; catches are checked in declaration order and a
    /// catch whose type is a subtype of (or equal to) an earlier
    /// catch's type is unreachable — an error in M8 (DESIGN.md 5.1).
    /// The catch local is immutable and scopes over its clause body
    /// only. The parser guarantees at least one `catch` or a `finally`
    /// and a type annotation on every catch parameter.
    pub(super) fn lower_try(&mut self, try_: &ast::Try) -> hir::StatementKind {
        let incoming = self.smart_casts.clone();
        let body = self.lower_block(&try_.body);
        let catch_facts = self.catch_smart_cast_facts(incoming.clone(), &body, None);
        let mut catches = Vec::with_capacity(try_.catches.len());
        for catch in &try_.catches {
            let Ok(Some(ty)) = self.resolve_catch_type(
                &catch.ty,
                catch.span,
                catches.iter().map(|earlier: &hir::CatchClause| earlier.ty),
            ) else {
                continue;
            };
            let local =
                self.alloc_declared_local(catch.name.text.clone(), ty, false, catch.name.span);
            let body = self.with_catch_scope(&catch.name, local, |this| {
                this.with_smart_casts(catch_facts.clone(), |this| this.lower_block(&catch.body))
            });
            catches.push(hir::CatchClause {
                local,
                ty,
                body,
                span: catch.span,
            });
        }
        let finally_facts = self.finally_smart_cast_facts(
            incoming.clone(),
            &body,
            catches.iter().map(|catch| catch.body.as_slice()),
        );
        let finally_body = try_
            .finally_body
            .as_ref()
            .map(|b| self.with_smart_casts(finally_facts, |this| this.lower_block(b)));
        self.smart_casts = incoming;
        hir::StatementKind::Try(hir::Try {
            body,
            catches,
            finally_body,
        })
    }

    /// `try` in value position. The pending result assignment lives inside
    /// the try/catch path, before `finally`, so the existing structured
    /// exception lowering preserves Kotlin's result and override semantics.
    pub(crate) fn lower_try_expression(
        &mut self,
        try_: &ast::Try,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let incoming = self.smart_casts.clone();
        let mut resolved = Vec::new();
        let mut covered = Vec::new();
        for catch in &try_.catches {
            let Some(ty) = self
                .resolve_catch_type(&catch.ty, catch.span, covered.iter().copied())
                .ok()?
            else {
                continue;
            };
            covered.push(ty);
            let local =
                self.alloc_declared_local(catch.name.text.clone(), ty, false, catch.name.span);
            resolved.push((catch, ty, local));
        }

        let mut body_failure = None;
        let mut catch_failures = (0..resolved.len()).map(|_| None).collect::<Vec<_>>();
        let mut body = self.probe_contextual_value(
            self.value_block_context_requirement(&try_.body),
            expected,
            &mut body_failure,
            |this, expected| {
                this.with_smart_casts(incoming.clone(), |this| {
                    this.lower_value_block(&try_.body, expected)
                })
            },
        )?;
        let catch_facts = if let Some(body) = &body {
            self.catch_smart_cast_facts(incoming.clone(), &body.statements, body.value.as_ref())
        } else {
            self.facts_without_mutable_bindings(incoming.clone())
        };
        let mut catches: Vec<Option<ValueCatch>> = (0..resolved.len()).map(|_| None).collect();
        for (index, &(catch, ty, local)) in resolved.iter().enumerate() {
            catches[index] = self.probe_contextual_value(
                self.value_block_context_requirement(&catch.body),
                expected,
                &mut catch_failures[index],
                |this, expected| {
                    this.with_smart_casts(catch_facts.clone(), |this| {
                        this.lower_value_catch(catch, ty, local, expected)
                    })
                },
            )?;
        }
        let has_hint = body
            .as_ref()
            .is_some_and(|body| body.value.as_ref().is_some())
            || catches.iter().any(|catch| {
                catch
                    .as_ref()
                    .is_some_and(|catch| catch.body.value.as_ref().is_some())
            });
        if !has_hint {
            let body_rank = (body.is_none() && body_failure.is_none())
                .then(|| self.value_block_default_seed_rank(&try_.body))
                .flatten();
            let mut seeds = resolved
                .iter()
                .enumerate()
                .filter(|(index, _)| catches[*index].is_none() && catch_failures[*index].is_none())
                .filter_map(|(index, &(catch, _, _))| {
                    self.value_block_default_seed_rank(&catch.body)
                        .map(|rank| (rank, Some(index)))
                })
                .collect::<Vec<_>>();
            seeds.extend(body_rank.map(|rank| (rank, None)));
            seeds.sort_by_key(|(rank, index)| std::cmp::Reverse((*rank, index.is_none(), *index)));
            for (_, index) in seeds {
                let ready = if let Some(index) = index {
                    let (catch, ty, local) = resolved[index];
                    catches[index] = self.probe_contextual_value(
                        ExpressionContextRequirement::Unresolved,
                        None,
                        &mut catch_failures[index],
                        |this, expected| {
                            this.with_smart_casts(catch_facts.clone(), |this| {
                                this.lower_value_catch(catch, ty, local, expected)
                            })
                        },
                    )?;
                    catches[index].is_some()
                } else {
                    body = self.probe_contextual_value(
                        ExpressionContextRequirement::Unresolved,
                        None,
                        &mut body_failure,
                        |this, expected| {
                            this.with_smart_casts(incoming.clone(), |this| {
                                this.lower_value_block(&try_.body, expected)
                            })
                        },
                    )?;
                    body.is_some()
                };
                if ready {
                    break;
                }
            }
        }
        let mut hint_types = Vec::new();
        if let Some(ty) = body
            .as_ref()
            .and_then(|body| body.value.as_ref().map(|value| value.ty))
        {
            hint_types.push(ty);
        }
        hint_types.extend(catches.iter().filter_map(|catch| {
            catch
                .as_ref()
                .and_then(|catch| catch.body.value.as_ref().map(|value| value.ty))
        }));
        let hint = (!hint_types.is_empty()).then(|| self.least_upper_bound(&hint_types));
        if body.is_none() {
            body =
                Some(
                    self.finish_contextual_value(hint, &mut body_failure, |this, expected| {
                        this.with_smart_casts(incoming.clone(), |this| {
                            this.lower_value_block(&try_.body, expected)
                        })
                    })?,
                );
        }
        for (index, &(catch, ty, local)) in resolved.iter().enumerate() {
            if catches[index].is_none() {
                catches[index] = Some(self.finish_contextual_value(
                    hint,
                    &mut catch_failures[index],
                    |this, expected| {
                        this.with_smart_casts(catch_facts.clone(), |this| {
                            this.lower_value_catch(catch, ty, local, expected)
                        })
                    },
                )?);
            }
        }
        let mut body = body.expect("the try body was lowered");
        let mut catches: Vec<ValueCatch> = catches
            .into_iter()
            .map(|catch| catch.expect("every catch body was lowered"))
            .collect();
        let mut block_refs = vec![&mut body];
        block_refs.extend(catches.iter_mut().map(|catch| &mut catch.body));
        let result =
            self.finish_control_value("try", try_.span, expected, block_refs.as_mut_slice())?;
        drop(block_refs);

        let finally_facts = self.finally_smart_cast_facts(
            incoming.clone(),
            &body.statements,
            catches.iter().map(|catch| catch.body.statements.as_slice()),
        );
        let finally_body = match &try_.finally_body {
            Some(finally) => {
                let mut block = self.with_smart_casts(finally_facts, |this| {
                    this.lower_value_block(finally, None)
                })?;
                if let Some(value) = block.value.take()
                    && !matches!(value.kind, hir::ExprKind::UnitLiteral)
                {
                    block.statements.push(hir::Statement {
                        span: value.origin.concrete().evaluation.span,
                        kind: hir::StatementKind::Expr(value),
                    });
                }
                Some(block.statements)
            }
            None => None,
        };
        self.smart_casts = incoming;
        sink.push(hir::Statement {
            span: try_.span,
            kind: hir::StatementKind::Try(hir::Try {
                body: body.statements,
                catches: catches
                    .into_iter()
                    .map(|catch| hir::CatchClause {
                        local: catch.local,
                        ty: catch.ty,
                        body: catch.body.statements,
                        span: catch.span,
                    })
                    .collect(),
                finally_body,
            }),
        });
        Some(result)
    }

    fn lower_value_catch(
        &mut self,
        catch: &ast::CatchClause,
        ty: TypeId,
        local: hir::LocalId,
        expected: Option<TypeId>,
    ) -> Option<ValueCatch> {
        let body = self.with_catch_scope(&catch.name, local, |this| {
            this.lower_value_block(&catch.body, expected)
        });
        Some(ValueCatch {
            local,
            ty,
            body: body?,
            span: catch.span,
        })
    }
}
