use super::*;

struct ValueCatch {
    local: hir::LocalId,
    ty: TypeId,
    body: ValueBlock,
    span: Span,
}

impl Lowerer {
    /// `throw expr` (spec 11.7, milestone8 DESIGN.md 3.2): the operand
    /// must be a subtype of the core `Throwable` class. `throw`
    /// produces no value — M8 has no `Nothing` type, so it lowers to
    /// the dedicated `Throw` statement, which downstream stages treat
    /// as control flow that never falls through.
    pub(super) fn lower_throw(
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
        let body = self.lower_block(&try_.body);
        let mut catches = Vec::with_capacity(try_.catches.len());
        for catch in &try_.catches {
            let Some(ty) = self.resolve_type_ref(&catch.ty) else {
                continue; // diagnostic already recorded
            };
            let Some(throwable) = self.throwable_ty(catch.ty.span) else {
                continue;
            };
            if !self.is_subtype(ty, throwable) {
                let found = self.type_name(ty);
                self.error(
                    catch.ty.span,
                    format!("catch parameter type {found} is not a subtype of Throwable"),
                );
                continue;
            }
            // Shadowing: an earlier catch whose type covers this one
            // (supertype or equal) makes it unreachable.
            if catches
                .iter()
                .any(|earlier: &hir::CatchClause| self.is_subtype(ty, earlier.ty))
            {
                let found = self.type_name(ty);
                self.error(
                    catch.span,
                    format!(
                        "unreachable catch block: {found} is already covered by an earlier catch"
                    ),
                );
                continue;
            }
            self.push_scope();
            let local =
                self.alloc_declared_local(catch.name.text.clone(), ty, false, catch.name.span);
            self.scopes.declare(catch.name.text.clone(), local);
            let body = self.lower_block(&catch.body);
            self.pop_scope();
            catches.push(hir::CatchClause {
                local,
                ty,
                body,
                span: catch.span,
            });
        }
        let finally_body = try_.finally_body.as_ref().map(|b| self.lower_block(b));
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
        let mut resolved = Vec::new();
        let mut covered = Vec::new();
        for catch in &try_.catches {
            let Some(ty) = self.resolve_type_ref(&catch.ty) else {
                continue;
            };
            let throwable = self.throwable_ty(catch.ty.span)?;
            if !self.is_subtype(ty, throwable) {
                let found = self.type_name(ty);
                self.error(
                    catch.ty.span,
                    format!("catch parameter type {found} is not a subtype of Throwable"),
                );
                continue;
            }
            if covered.iter().any(|&earlier| self.is_subtype(ty, earlier)) {
                let found = self.type_name(ty);
                self.error(
                    catch.span,
                    format!(
                        "unreachable catch block: {found} is already covered by an earlier catch"
                    ),
                );
                continue;
            }
            covered.push(ty);
            let local =
                self.alloc_declared_local(catch.name.text.clone(), ty, false, catch.name.span);
            resolved.push((catch, ty, local));
        }

        let defer_body = expected.is_none() && self.value_block_requires_expected(&try_.body);
        let catch_deferred: Vec<_> = resolved
            .iter()
            .map(|(catch, _, _)| {
                expected.is_none() && self.value_block_requires_expected(&catch.body)
            })
            .collect();
        let mut body = if defer_body {
            None
        } else {
            Some(self.lower_value_block(&try_.body, expected)?)
        };
        let mut catches: Vec<Option<ValueCatch>> = (0..resolved.len()).map(|_| None).collect();
        for (index, &(catch, ty, local)) in resolved.iter().enumerate() {
            if !catch_deferred[index] {
                catches[index] = Some(self.lower_value_catch(catch, ty, local, expected)?);
            }
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
            let body_rank = body
                .is_none()
                .then(|| self.value_block_default_seed_rank(&try_.body))
                .flatten();
            let catch_seed = resolved
                .iter()
                .enumerate()
                .filter(|(index, _)| catches[*index].is_none() && catch_deferred[*index])
                .filter_map(|(index, &(catch, ty, local))| {
                    self.value_block_default_seed_rank(&catch.body)
                        .map(|rank| (rank, index, catch, ty, local))
                })
                .max_by_key(|(rank, _, _, _, _)| *rank);
            if body_rank.is_some()
                && body_rank >= catch_seed.as_ref().map(|(rank, _, _, _, _)| *rank)
            {
                body = Some(self.lower_value_block(&try_.body, None)?);
            } else if let Some((_, index, catch, ty, local)) = catch_seed {
                catches[index] = Some(self.lower_value_catch(catch, ty, local, None)?);
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
            body = Some(self.lower_value_block(&try_.body, hint)?);
        }
        for (index, &(catch, ty, local)) in resolved.iter().enumerate() {
            if catches[index].is_none() {
                catches[index] = Some(self.lower_value_catch(catch, ty, local, hint)?);
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

        let finally_body = match &try_.finally_body {
            Some(finally) => {
                let mut block = self.lower_value_block(finally, None)?;
                if let Some(value) = block.value.take()
                    && !matches!(value.kind, hir::ExprKind::UnitLiteral)
                {
                    block.statements.push(hir::Statement {
                        span: value.span,
                        kind: hir::StatementKind::Expr(value),
                    });
                }
                Some(block.statements)
            }
            None => None,
        };
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
        self.push_scope();
        self.scopes.declare(catch.name.text.clone(), local);
        let body = self.lower_value_block(&catch.body, expected);
        self.pop_scope();
        Some(ValueCatch {
            local,
            ty,
            body: body?,
            span: catch.span,
        })
    }
}
