//! Statement lowering: declarations, assignments, control flow,
//! `return` and block-level scoping (milestone2 DESIGN.md 2.2,
//! milestone3 DESIGN.md 2.2).
//!
//! M4 (milestone4 DESIGN.md 3.2): statement-level `when` with pattern
//! arms, guards and exhaustiveness checking, and destructuring
//! `val` / `var` declarations (the binding target is a pattern, not
//! just an identifier).
//!
//! M5 (milestone5 DESIGN.md 2.2): subscript assignment
//! (`array[index] = value`) alongside local assignment.
//!
//! M8 (milestone8 DESIGN.md 3.2): `throw` and
//! `try { } catch (e: T) { } finally { }`.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{FunctionId, Type, TypeId};

use crate::patterns::PatternCtx;
use crate::{ForbiddenSuspendContext, Lowerer, Owner, SuspensionContext};

pub(crate) struct ValueBlock {
    pub(crate) statements: Vec<hir::Statement>,
    /// `None` means the block has no normally completing path.
    pub(crate) value: Option<hir::Expr>,
}

struct ValueArm {
    pattern: hir::Pattern,
    guard: Option<hir::Expr>,
    body: ValueBlock,
    span: Span,
}

struct ValueCatch {
    local: hir::LocalId,
    ty: TypeId,
    body: ValueBlock,
    span: Span,
}

impl Lowerer {
    pub(crate) fn lower_body(&mut self, id: FunctionId, decl: &ast::FunctionDecl) -> hir::Body {
        let sig = self.signatures[&id].clone();
        // Member functions (M6): `this` is parameter 0, an immutable
        // local of the host type; bare property / method names in the
        // body resolve against it.
        let owner = self.function_owner.get(&id).copied();
        // Method signatures already carry the combined owner-prefix plus
        // method-suffix namespace established in pass 2.5.
        self.type_params_in_scope = sig.type_params.clone();
        self.current_return_ty = sig.return_ty;
        self.current_fn_name = decl.name.text.clone();
        self.push_suspension_context(if decl.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });

        self.current_owner = owner;
        self.current_this = None;

        // Parameters are immutable locals in the function's outermost
        // scope; the body block nests inside it, so body locals may
        // shadow parameters.
        self.scopes.push();
        let mut params = Vec::with_capacity(sig.params.len() + 1);
        if let Some(owner) = owner {
            let host_ty = self.owner_ty(owner);
            let local = self.alloc_local("this".to_string(), host_ty, false);
            self.scopes.declare("this".to_string(), local);
            self.current_this = Some((local, host_ty));
            params.push(hir::Param {
                name: "this".to_string(),
                ty: host_ty,
                local,
            });
        }
        for param in &sig.params {
            if self.scopes.is_declared_here(&param.name.text) {
                self.error(
                    param.name.span,
                    format!("duplicate parameter `{}`", param.name.text),
                );
                continue;
            }
            let local = self.alloc_local(param.name.text.clone(), param.ty, false);
            self.scopes.declare(param.name.text.clone(), local);
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        self.functions[id].params = params;

        let returns_unit = self.types_equal(sig.return_ty, self.unit);
        let statements = match &decl.body {
            ast::FunctionBody::Block(block) => {
                let diagnostics_before = self.diagnostics.len();
                let statements = self.lower_block(block);
                // A non-Unit block body must not fall through on any
                // reachable path. The analysis is compositional over
                // sequential statements and structured control flow;
                // skipped when lowering already diagnosed the body,
                // because missing statements are commonly fallout of
                // an earlier error.
                if !returns_unit
                    && self.diagnostics.len() == diagnostics_before
                    && statements_can_fall_through(&statements)
                {
                    self.error(
                        block.span,
                        format!(
                            "non-Unit function `{}` may complete without returning a value",
                            decl.name.text
                        ),
                    );
                }
                statements
            }
            // `fun f(...) [: T] = expr` is a single `return expr`.
            ast::FunctionBody::Expr(expr) => {
                let mut statements = Vec::new();
                let mut sink = Vec::new();
                if let Some(value) = self.lower_expr(expr, &mut sink, Some(sig.return_ty)) {
                    if !self.is_subtype(value.ty, sig.return_ty) {
                        let expected = self.type_name(sig.return_ty);
                        let found = self.type_name(value.ty);
                        self.error(
                            expr.span(),
                            format!(
                                "body of `{}` must be of type {expected}, found {found}",
                                decl.name.text
                            ),
                        );
                    } else {
                        let value = self.adapt_to(value, sig.return_ty);
                        statements.extend(sink);
                        self.push_return(Some(value), expr.span(), &mut statements);
                    }
                }
                statements
            }
            // Bodyless declarations were diagnosed at signature
            // resolution (abstract / interface / intrinsic only);
            // there is nothing to lower.
            ast::FunctionBody::None => Vec::new(),
        };
        self.scopes.pop();
        self.type_params_in_scope.clear();
        self.current_this = None;
        self.current_owner = None;
        self.pop_suspension_context();

        hir::Body {
            locals: std::mem::take(&mut self.locals),
            statements,
        }
    }

    /// Append the statement(s) returning `value` (already checked
    /// against the current function's return type). In a `Unit`
    /// function the value is evaluated as a plain statement and the
    /// `return` is bare: `hir::StatementKind::Return::value` is absent
    /// in `Unit` functions (hir docs).
    fn push_return(&mut self, value: Option<hir::Expr>, span: Span, out: &mut Vec<hir::Statement>) {
        if let Some(value) = value {
            if self.types_equal(self.current_return_ty, self.unit) {
                out.push(hir::Statement {
                    kind: hir::StatementKind::Expr(value),
                    span,
                });
                out.push(hir::Statement {
                    kind: hir::StatementKind::Return { value: None },
                    span,
                });
            } else {
                out.push(hir::Statement {
                    kind: hir::StatementKind::Return { value: Some(value) },
                    span,
                });
            }
        } else {
            out.push(hir::Statement {
                kind: hir::StatementKind::Return { value: None },
                span,
            });
        }
    }

    /// Lower a block in a fresh scope: declarations inside are not
    /// visible after the block ends.
    fn lower_block(&mut self, block: &ast::Block) -> Vec<hir::Statement> {
        self.scopes.push();
        let mut statements = Vec::new();
        for statement in &block.statements {
            self.lower_statement(statement, &mut statements);
        }
        self.scopes.pop();
        statements
    }

    /// Whether the trailing value of a block intrinsically needs an
    /// expected type (`None`, an empty array, ...). Structured expressions
    /// run their own branch inference and are therefore not deferred here.
    fn value_block_requires_expected(&self, block: &ast::Block) -> bool {
        match block.statements.last().map(|statement| &statement.kind) {
            Some(ast::StatementKind::Expr(expr)) => self.expr_requires_expected_type(expr),
            _ => false,
        }
    }

    /// Lower a control-expression branch in a fresh scope. The final
    /// expression is removed from statement position and returned as the
    /// block's value. A final legacy control statement is interpreted as a
    /// value too, so nested `if` / `when` / `try` works even though their
    /// standalone parser forms remain statement nodes.
    pub(crate) fn lower_value_block(
        &mut self,
        block: &ast::Block,
        expected: Option<TypeId>,
    ) -> Option<ValueBlock> {
        self.scopes.push();
        let lowered = (|| {
            let mut statements = Vec::new();
            let value = if let Some((last, prefix)) = block.statements.split_last() {
                for statement in prefix {
                    self.lower_statement(statement, &mut statements);
                }
                match &last.kind {
                    ast::StatementKind::Expr(expr) => {
                        let mut tail = Vec::new();
                        let value = self.lower_expr(expr, &mut tail, expected)?;
                        statements.extend(tail);
                        Some(value)
                    }
                    ast::StatementKind::If(if_) => {
                        self.lower_if_expression(if_, &mut statements, expected)
                    }
                    ast::StatementKind::When(when) => {
                        self.lower_when_expression(when, &mut statements, expected)
                    }
                    ast::StatementKind::Try(try_) => {
                        self.lower_try_expression(try_, &mut statements, expected)
                    }
                    _ => {
                        self.lower_statement(last, &mut statements);
                        None
                    }
                }
            } else {
                None
            };
            let value = if statements_can_fall_through(&statements) {
                Some(value.unwrap_or(hir::Expr {
                    kind: hir::ExprKind::UnitLiteral,
                    ty: self.unit,
                    span: block.span,
                }))
            } else {
                None
            };
            Some(ValueBlock { statements, value })
        })();
        self.scopes.pop();
        lowered
    }

    /// Infer a provisional branch hint from already-lowered normal paths.
    fn value_block_hint<'a>(
        &mut self,
        blocks: impl Iterator<Item = &'a ValueBlock>,
    ) -> Option<TypeId> {
        let types: Vec<_> = blocks
            .filter_map(|block| block.value.as_ref().map(|value| value.ty))
            .collect();
        (!types.is_empty()).then(|| self.least_upper_bound(&types))
    }

    /// Type-check and materialize a structured expression's branch result.
    /// Each normal non-Unit branch assigns the same hidden local; Unit tails
    /// are merely evaluated for side effects.
    fn finish_control_value(
        &mut self,
        kind: &str,
        span: Span,
        expected: Option<TypeId>,
        blocks: &mut [&mut ValueBlock],
    ) -> Option<hir::Expr> {
        let value_types: Vec<_> = blocks
            .iter()
            .filter_map(|block| block.value.as_ref().map(|value| value.ty))
            .collect();
        let result_ty = expected.unwrap_or_else(|| {
            if value_types.is_empty() {
                self.unit
            } else {
                self.least_upper_bound(&value_types)
            }
        });
        for block in blocks.iter() {
            if let Some(value) = &block.value
                && !self.is_subtype(value.ty, result_ty)
            {
                let expected = self.type_name(result_ty);
                let found = self.type_name(value.ty);
                self.error(
                    value.span,
                    format!("{kind} branch result must be of type {expected}, found {found}"),
                );
                return None;
            }
        }

        if self.types_equal(result_ty, self.unit) {
            for block in blocks.iter_mut() {
                if let Some(value) = block.value.take()
                    && !matches!(value.kind, hir::ExprKind::UnitLiteral)
                {
                    block.statements.push(hir::Statement {
                        span: value.span,
                        kind: hir::StatementKind::Expr(value),
                    });
                }
            }
            return Some(hir::Expr {
                kind: hir::ExprKind::UnitLiteral,
                ty: self.unit,
                span,
            });
        }

        let result = self.alloc_hidden_result(result_ty);
        for block in blocks.iter_mut() {
            if let Some(value) = block.value.take() {
                let value = self.adapt_to(value, result_ty);
                block.statements.push(hir::Statement {
                    span: value.span,
                    kind: hir::StatementKind::Assign {
                        target: hir::AssignTarget::Local(result),
                        value,
                    },
                });
            }
        }
        Some(hir::Expr {
            kind: hir::ExprKind::Local(result),
            ty: result_ty,
            span,
        })
    }

    /// `if` in value position. The condition prelude and the structured HIR
    /// statement are appended to the caller's desugaring sink; the returned
    /// expression reads the branch-result local.
    pub(crate) fn lower_if_expression(
        &mut self,
        if_: &ast::If,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let Some(else_block) = &if_.else_block else {
            self.error(
                if_.span,
                "if expression requires an `else` branch".to_string(),
            );
            return None;
        };
        let mut cond_sink = Vec::new();
        let before = self.diagnostics.len();
        let cond = self.lower_condition(&if_.cond, "if", &mut cond_sink)?;
        let (then_narrowings, else_narrowings) = if self.diagnostics.len() == before {
            (
                self.resolve_smart_casts(&if_.cond, true),
                self.resolve_smart_casts(&if_.cond, false),
            )
        } else {
            (Vec::new(), Vec::new())
        };

        let defer_then = expected.is_none() && self.value_block_requires_expected(&if_.then_block);
        let defer_else = expected.is_none() && self.value_block_requires_expected(else_block);
        let mut then_value = if defer_then {
            None
        } else {
            Some(self.with_smart_casts(then_narrowings.clone(), |this| {
                this.lower_value_block(&if_.then_block, expected)
            })?)
        };
        let mut else_value = if defer_else {
            None
        } else {
            Some(self.with_smart_casts(else_narrowings.clone(), |this| {
                this.lower_value_block(else_block, expected)
            })?)
        };
        let hint = self.value_block_hint(then_value.iter().chain(else_value.iter()));
        if then_value.is_none() {
            then_value = Some(self.with_smart_casts(then_narrowings, |this| {
                this.lower_value_block(&if_.then_block, hint)
            })?);
        }
        if else_value.is_none() {
            else_value = Some(self.with_smart_casts(else_narrowings, |this| {
                this.lower_value_block(else_block, hint)
            })?);
        }
        let mut then_value = then_value.expect("both branches were lowered");
        let mut else_value = else_value.expect("both branches were lowered");
        let result = self.finish_control_value(
            "if",
            if_.span,
            expected,
            &mut [&mut then_value, &mut else_value],
        )?;
        sink.extend(cond_sink);
        sink.push(hir::Statement {
            span: if_.span,
            kind: hir::StatementKind::If {
                cond,
                then_body: then_value.statements,
                else_body: Some(else_value.statements),
            },
        });
        Some(result)
    }

    /// Lower one statement, appending to `out`. Nested blocks are
    /// flattened into `out`: scoping is fully resolved here (every
    /// reference already carries its `LocalId`), so HIR needs no block
    /// statement.
    ///
    /// Expression positions create a fresh desugaring sink (see
    /// `lower_expr` in expr.rs); the sink is drained into `out` right
    /// before the owning statement.
    fn lower_statement(&mut self, statement: &ast::Statement, out: &mut Vec<hir::Statement>) {
        let kind = match &statement.kind {
            ast::StatementKind::Expr(expr) => {
                let mut sink = Vec::new();
                let Some(lowered) = self.lower_expr(expr, &mut sink, None) else {
                    return; // diagnostic already recorded
                };
                // M1 rule, unchanged: expression statements are calls
                // (declarations, assignments and control flow are their
                // own statement kinds since M2; M6 adds method calls).
                // Constructions lower to `StructInit` /
                // `VariantConstruct`, not `Call`, so they are rejected
                // here.
                if matches!(
                    lowered.kind,
                    hir::ExprKind::Call { .. }
                        | hir::ExprKind::MethodCall { .. }
                        | hir::ExprKind::CallableCall { .. }
                ) {
                    out.extend(sink);
                    hir::StatementKind::Expr(lowered)
                } else {
                    self.error(
                        statement.span,
                        "statement must be a function call".to_string(),
                    );
                    return;
                }
            }
            ast::StatementKind::Return { value } => {
                let return_ty = self.current_return_ty;
                match value {
                    None => {
                        if !self.types_equal(return_ty, self.unit) {
                            let name = self.current_fn_name.clone();
                            let expected = self.type_name(return_ty);
                            self.error(
                                statement.span,
                                format!(
                                    "`return` without a value in function `{name}` returning {expected}"
                                ),
                            );
                            return;
                        }
                        hir::StatementKind::Return { value: None }
                    }
                    Some(expr) => {
                        let mut sink = Vec::new();
                        let Some(value) = self.lower_expr(expr, &mut sink, Some(return_ty)) else {
                            return; // diagnostic already recorded
                        };
                        if !self.is_subtype(value.ty, return_ty) {
                            let name = self.current_fn_name.clone();
                            let expected = self.type_name(return_ty);
                            let found = self.type_name(value.ty);
                            self.error(
                                expr.span(),
                                format!(
                                    "`return` value of `{name}` must be of type {expected}, found {found}"
                                ),
                            );
                            return;
                        }
                        let value = self.adapt_to(value, return_ty);
                        out.extend(sink);
                        self.push_return(Some(value), statement.span, out);
                        return; // push_return already appended
                    }
                }
            }
            ast::StatementKind::ValDecl(decl) => {
                let Some(kind) = self.lower_val_decl(decl, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::When(when) => {
                let Some(kind) = self.lower_when(when, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::Throw(expr) => {
                let Some(kind) = self.lower_throw(expr, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::Try(try_) => self.lower_try(try_),
            ast::StatementKind::Assign(assign) => {
                let Some(kind) = self.lower_assign(assign, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::If(if_) => {
                let mut sink = Vec::new();
                let before = self.diagnostics.len();
                let Some(cond) = self.lower_condition(&if_.cond, "if", &mut sink) else {
                    return;
                };
                // The condition is evaluated exactly once, right before
                // the `if`, so desugaring statements belong before it.
                out.extend(sink);
                // Smart casts (milestone6 DESIGN.md 5.4): `x is T`
                // narrows `x` in the then branch, `x !is T` in the
                // else branch. Skipped when the condition produced
                // diagnostics (its narrowings would be unreliable; the
                // module is rejected anyway).
                let (then_narrowings, else_narrowings) = if self.diagnostics.len() == before {
                    (
                        self.resolve_smart_casts(&if_.cond, true),
                        self.resolve_smart_casts(&if_.cond, false),
                    )
                } else {
                    (Vec::new(), Vec::new())
                };
                let then_body = self
                    .with_smart_casts(then_narrowings, |this| this.lower_block(&if_.then_block));
                let else_body = if_
                    .else_block
                    .as_ref()
                    .map(|b| self.with_smart_casts(else_narrowings, |this| this.lower_block(b)));
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                }
            }
            ast::StatementKind::While(while_) => {
                let mut sink = Vec::new();
                let Some(cond) = self.lower_condition(&while_.cond, "while", &mut sink) else {
                    return;
                };
                // A while condition is re-evaluated once per iteration,
                // but sink statements would execute once before the
                // loop; reject desugaring operators here instead of
                // silently changing evaluation semantics.
                if !sink.is_empty() {
                    self.error(
                        while_.span,
                        "`?.` and `?:` are not allowed in a while condition".to_string(),
                    );
                    return;
                }
                let body = self.lower_block(&while_.body);
                hir::StatementKind::While { cond, body }
            }
            ast::StatementKind::Block(block) => {
                self.scopes.push();
                for statement in &block.statements {
                    self.lower_statement(statement, out);
                }
                self.scopes.pop();
                return;
            }
        };
        out.push(hir::Statement {
            kind,
            span: statement.span,
        });
    }

    /// `val` / `var` declarations always have an initializer (the AST
    /// guarantees it): the declared type is the annotation when present
    /// (the initializer must match it exactly), the initializer's type
    /// otherwise. The annotation is the initializer's expected-type
    /// hint (this is what types a `None` construction).
    ///
    /// The binding target is a pattern (spec 4.6): a plain `val x` is
    /// `Pattern::Binding`, destructuring uses tuple/struct patterns.
    /// Only irrefutable patterns are allowed here — `lower_pattern`
    /// with `in_when: false` rejects enum variants and literals.
    fn lower_val_decl(
        &mut self,
        decl: &ast::ValDecl,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let annotation = match &decl.ty {
            Some(ty_ref) => Some(self.resolve_type_ref(ty_ref)?),
            None => None,
        };
        let mut sink = Vec::new();
        let init = self.lower_expr(&decl.init, &mut sink, annotation)?;
        let (init, ty) = match annotation {
            Some(expected) => {
                if !self.is_subtype(init.ty, expected) {
                    let expected_name = self.type_name(expected);
                    let found = self.type_name(init.ty);
                    self.error(
                        decl.init.span(),
                        format!(
                            "initializer of `{}` must be of type {expected_name}, found {found}",
                            ast::dump_pattern(&decl.target)
                        ),
                    );
                    return None;
                }
                (self.adapt_to(init, expected), expected)
            }
            None => {
                let ty = init.ty;
                (init, ty)
            }
        };
        // The pattern is lowered after the initializer, so bindings are
        // not visible in their own initializer.
        let pattern = self.lower_pattern(
            &decl.target,
            ty,
            PatternCtx {
                mutable: decl.mutable,
                in_when: false,
            },
        )?;
        out.extend(sink);
        Some(hir::StatementKind::ValDecl { pattern, init })
    }

    /// Statement-position pattern `when` (spec 5). The subject must be an
    /// enum, tuple or struct — the current pattern-matching subset has no
    /// Kotlin-style condition `when`. Pattern bindings
    /// scope over the arm's guard and body; exhaustiveness is checked
    /// over the whole statement.
    fn lower_when(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let subject = self.lower_expr(&when.subject, &mut sink, None)?;
        if !matches!(
            self.types[subject.ty],
            Type::Enum(..) | Type::Tuple(..) | Type::Struct(..)
        ) {
            let found = self.type_name(subject.ty);
            self.error(
                when.subject.span(),
                format!("`when` subject must be an enum, tuple or struct, found {found}"),
            );
            return None;
        }
        // The subject is evaluated exactly once, right before the
        // `when`, so desugaring statements belong before it.
        out.extend(sink);

        let mut arms = Vec::with_capacity(when.arms.len());
        let mut arms_ok = true;
        for arm in &when.arms {
            self.scopes.push();
            let lowered = self.lower_arm(arm, subject.ty);
            self.scopes.pop();
            match lowered {
                Some(arm) => arms.push(arm),
                None => arms_ok = false,
            }
        }
        let else_body = when.else_body.as_ref().map(|b| self.lower_block(b));
        // With a failed arm the coverage information is unreliable;
        // the module is rejected anyway, so skip the exhaustiveness
        // check to avoid noise.
        if arms_ok {
            self.check_exhaustiveness(when.span, subject.ty, &arms, else_body.is_some());
        }
        Some(hir::StatementKind::When(hir::When {
            subject,
            arms,
            else_body,
        }))
    }

    /// Pattern `when` in value position. Arms that need context are lowered
    /// after context-independent arms establish a provisional result type;
    /// source order is restored in the resulting HIR.
    pub(crate) fn lower_when_expression(
        &mut self,
        when: &ast::When,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let mut subject_sink = Vec::new();
        let subject = self.lower_expr(&when.subject, &mut subject_sink, None)?;
        if !matches!(
            self.types[subject.ty],
            Type::Enum(..) | Type::Tuple(..) | Type::Struct(..)
        ) {
            let found = self.type_name(subject.ty);
            self.error(
                when.subject.span(),
                format!("`when` subject must be an enum, tuple or struct, found {found}"),
            );
            return None;
        }

        let deferred: Vec<_> = when
            .arms
            .iter()
            .map(|arm| expected.is_none() && self.value_block_requires_expected(&arm.body))
            .collect();
        let defer_else = when
            .else_body
            .as_ref()
            .is_some_and(|body| expected.is_none() && self.value_block_requires_expected(body));
        let mut arms: Vec<Option<ValueArm>> = (0..when.arms.len()).map(|_| None).collect();
        for (index, arm) in when.arms.iter().enumerate() {
            if !deferred[index] {
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, expected)?);
            }
        }
        let mut else_value = match &when.else_body {
            Some(body) if !defer_else => Some(self.lower_value_block(body, expected)?),
            _ => None,
        };
        let mut hint_types: Vec<_> = arms
            .iter()
            .filter_map(|arm| {
                arm.as_ref()
                    .and_then(|arm| arm.body.value.as_ref().map(|value| value.ty))
            })
            .collect();
        if let Some(ty) = else_value
            .as_ref()
            .and_then(|body| body.value.as_ref().map(|value| value.ty))
        {
            hint_types.push(ty);
        }
        let hint = (!hint_types.is_empty()).then(|| self.least_upper_bound(&hint_types));
        for (index, arm) in when.arms.iter().enumerate() {
            if arms[index].is_none() {
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, hint)?);
            }
        }
        if defer_else {
            else_value =
                Some(self.lower_value_block(
                    when.else_body.as_ref().expect("deferred else exists"),
                    hint,
                )?);
        }
        let mut arms: Vec<ValueArm> = arms
            .into_iter()
            .map(|arm| arm.expect("every when arm was lowered"))
            .collect();
        let mut block_refs: Vec<&mut ValueBlock> =
            arms.iter_mut().map(|arm| &mut arm.body).collect();
        if let Some(else_value) = else_value.as_mut() {
            block_refs.push(else_value);
        }
        let result =
            self.finish_control_value("when", when.span, expected, block_refs.as_mut_slice())?;
        drop(block_refs);
        let arms: Vec<_> = arms
            .into_iter()
            .map(|arm| hir::WhenArm {
                pattern: arm.pattern,
                guard: arm.guard,
                body: arm.body.statements,
                span: arm.span,
            })
            .collect();
        self.check_exhaustiveness(when.span, subject.ty, &arms, else_value.is_some());
        sink.extend(subject_sink);
        sink.push(hir::Statement {
            span: when.span,
            kind: hir::StatementKind::When(hir::When {
                subject,
                arms,
                else_body: else_value.map(|body| body.statements),
            }),
        });
        Some(result)
    }

    /// One `when` arm: pattern (its bindings are in scope), optional
    /// guard (must be `Boolean`), body block.
    fn lower_arm(&mut self, arm: &ast::WhenArm, subject_ty: TypeId) -> Option<hir::WhenArm> {
        let (pattern, guard) = self.lower_arm_head(arm, subject_ty)?;
        let body = self.lower_block(&arm.body);
        Some(hir::WhenArm {
            pattern,
            guard,
            body,
            span: arm.span,
        })
    }

    fn lower_value_arm(
        &mut self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
        expected: Option<TypeId>,
    ) -> Option<ValueArm> {
        self.scopes.push();
        let lowered = (|| {
            let (pattern, guard) = self.lower_arm_head(arm, subject_ty)?;
            let body = self.lower_value_block(&arm.body, expected)?;
            Some(ValueArm {
                pattern,
                guard,
                body,
                span: arm.span,
            })
        })();
        self.scopes.pop();
        lowered
    }

    fn lower_arm_head(
        &mut self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
    ) -> Option<(hir::Pattern, Option<hir::Expr>)> {
        let pattern = self.lower_pattern(
            &arm.pattern,
            subject_ty,
            PatternCtx {
                mutable: false,
                in_when: true,
            },
        )?;
        let guard = match &arm.guard {
            Some(guard) => {
                let mut sink = Vec::new();
                let guard_expr = self.lower_expr(guard, &mut sink, None)?;
                // A guard is evaluated once per arm attempt, but sink
                // statements would execute unconditionally before the
                // arm body; reject desugaring operators (same rule as
                // while conditions).
                if !sink.is_empty() {
                    self.error(
                        guard.span(),
                        "`?.` and `?:` are not allowed in a when guard".to_string(),
                    );
                    return None;
                }
                if guard_expr.ty != self.boolean {
                    let found = self.type_name(guard_expr.ty);
                    self.error(
                        guard.span(),
                        format!("when guard must be Boolean, found {found}"),
                    );
                    return None;
                }
                Some(guard_expr)
            }
            None => None,
        };
        Some((pattern, guard))
    }

    /// `throw expr` (spec 11.7, milestone8 DESIGN.md 3.2): the operand
    /// must be a subtype of the core `Throwable` class. `throw`
    /// produces no value — M8 has no `Nothing` type, so it lowers to
    /// the dedicated `Throw` statement, which downstream stages treat
    /// as control flow that never falls through.
    fn lower_throw(
        &mut self,
        expr: &ast::Expr,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let value = self.lower_expr(expr, &mut sink, None)?;
        if let Some(throwable) = self.throwable_ty() {
            if !self.is_subtype(value.ty, throwable) {
                let found = self.type_name(value.ty);
                self.error(
                    expr.span(),
                    format!("cannot throw value of type {found}: not a subtype of Throwable"),
                );
                return None;
            }
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
    fn lower_try(&mut self, try_: &ast::Try) -> hir::StatementKind {
        let body = self.lower_block(&try_.body);
        let mut catches = Vec::with_capacity(try_.catches.len());
        for catch in &try_.catches {
            let Some(ty) = self.resolve_type_ref(&catch.ty) else {
                continue; // diagnostic already recorded
            };
            if let Some(throwable) = self.throwable_ty() {
                if !self.is_subtype(ty, throwable) {
                    let found = self.type_name(ty);
                    self.error(
                        catch.ty.span,
                        format!("catch parameter type {found} is not a subtype of Throwable"),
                    );
                    continue;
                }
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
            self.scopes.push();
            let local = self.alloc_local(catch.name.text.clone(), ty, false);
            self.scopes.declare(catch.name.text.clone(), local);
            let body = self.lower_block(&catch.body);
            self.scopes.pop();
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
            if let Some(throwable) = self.throwable_ty()
                && !self.is_subtype(ty, throwable)
            {
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
            let local = self.alloc_local(catch.name.text.clone(), ty, false);
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
        self.scopes.push();
        self.scopes.declare(catch.name.text.clone(), local);
        let body = self.lower_value_block(&catch.body, expected);
        self.scopes.pop();
        Some(ValueCatch {
            local,
            ty,
            body: body?,
            span: catch.span,
        })
    }

    /// Assignment. A `Local` target names a declared, mutable local; an
    /// `Index` target (`array[index] = value`, spec 10.5) requires a
    /// `MutableArray<T>` receiver, an `Int` index and a value of
    /// exactly the element type `T`; a `Field` target
    /// (`obj.field = value`, M6) requires a class receiver and a `var`
    /// constructor property. In all cases the target type is the
    /// value's expected-type hint.
    fn lower_assign(
        &mut self,
        assign: &ast::Assign,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        match &assign.target {
            ast::AssignTarget::Local(name) => self.lower_local_assign(assign, name, out),
            ast::AssignTarget::Index {
                receiver, index, ..
            } => self.lower_index_assign(assign, receiver, index, out),
            ast::AssignTarget::Field { receiver, name, .. } => {
                let mut sink = Vec::new();
                let Some(receiver) = self.lower_expr(receiver, &mut sink, None) else {
                    return None; // diagnostic already recorded
                };
                let kind = self.assign_class_field(assign, receiver, name, &mut sink)?;
                out.extend(sink);
                Some(kind)
            }
        }
    }

    /// `receiver.name = value` (M6): the receiver must be a class and
    /// `name` a `var` constructor property on it or its base chain
    /// (resolved exactly like a field read — absolute layout index,
    /// declaring class in the `FieldRef`). Value types are immutable
    /// and reject the assignment outright. The value's desugaring
    /// statements append to the receiver's sink (evaluation order:
    /// receiver, then value, then the store).
    fn assign_class_field(
        &mut self,
        assign: &ast::Assign,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let class_id = match self.types[receiver.ty] {
            Type::Class(id) => id,
            _ if self.is_value_ty(receiver.ty) => {
                self.error(
                    name.span,
                    "field assignment is not supported (value types are immutable)".to_string(),
                );
                return None;
            }
            _ => {
                let found = self.type_name(receiver.ty);
                self.error(name.span, format!("type `{found}` has no fields"));
                return None;
            }
        };
        let Some((declaring, index, field_ty, mutable)) =
            self.find_class_field(class_id, &name.text)
        else {
            let class_name = self.classes[class_id].name.clone();
            self.error(
                name.span,
                format!("class `{class_name}` has no field `{}`", name.text),
            );
            return None;
        };
        if !mutable {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        }
        let value = self.lower_expr(&assign.value, sink, Some(field_ty))?;
        if !self.is_subtype(value.ty, field_ty) {
            let expected = self.type_name(field_ty);
            let found = self.type_name(value.ty);
            self.error(
                assign.value.span(),
                format!(
                    "cannot assign value of type {found} to property `{}` of type {expected}",
                    name.text
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, field_ty);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Field {
                receiver: Box::new(receiver),
                field: hir::FieldRef::ClassField {
                    class_id: declaring,
                    index,
                },
            },
            value,
        })
    }

    /// `name = value`: the target must be a declared, mutable local and
    /// the value type must match the local's type. Inside a class
    /// method a bare name may also denote a constructor property
    /// (`y = v` meaning `this.y = v`): `var` properties store through
    /// `this`, `val` properties are immutable (diagnostic).
    /// Value-type fields stay unwritable as before.
    fn lower_local_assign(
        &mut self,
        assign: &ast::Assign,
        name: &ast::Ident,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let Some(local) = self.scopes.lookup(&name.text) else {
            if let Some(capture) = self.available_capture(&name.text) {
                if capture.mutable {
                    self.error(
                        name.span,
                        format!(
                            "cannot capture mutable local `{}`; bind its current value to a `val` snapshot or capture explicit reference state",
                            name.text
                        ),
                    );
                } else {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable variable `{}`", name.text),
                    );
                }
                return None;
            }
            match self.current_owner {
                Some(Owner::Class(class_id)) => {
                    if let Some((_, _, _, mutable)) = self.find_class_field(class_id, &name.text) {
                        if !mutable {
                            self.error(
                                name.span,
                                format!("cannot assign to immutable property `{}`", name.text),
                            );
                            return None;
                        }
                        let receiver = self
                            .lower_current_this(name.span)
                            .expect("a member callable body always has a lexical `this`");
                        let mut sink = Vec::new();
                        let kind = self.assign_class_field(assign, receiver, name, &mut sink)?;
                        out.extend(sink);
                        return Some(kind);
                    }
                }
                Some(Owner::Struct(struct_id))
                    if self.structs[struct_id]
                        .fields
                        .iter()
                        .any(|field| field.name == name.text) =>
                {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable property `{}`", name.text),
                    );
                    return None;
                }
                _ => {}
            }
            self.error(name.span, format!("unknown variable `{}`", name.text));
            return None;
        };
        if !self.locals[local].mutable {
            self.error(
                name.span,
                format!("cannot assign to immutable variable `{}`", name.text),
            );
            return None;
        }
        let expected = self.locals[local].ty;
        let mut sink = Vec::new();
        let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
        if !self.is_subtype(value.ty, expected) {
            let expected_name = self.type_name(expected);
            let found = self.type_name(value.ty);
            self.error(
                assign.value.span(),
                format!(
                    "cannot assign value of type {found} to `{}` of type {expected_name}",
                    name.text
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, expected);
        out.extend(sink);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Local(local),
            value,
        })
    }

    /// `array[index] = value` (spec 10.5, milestone5 DESIGN.md 2.2):
    /// only `MutableArray<T>` is assignable (an `Array<T>` receiver
    /// gets its own diagnostic), the index must be `Int` and the value
    /// exactly `T`.
    fn lower_index_assign(
        &mut self,
        assign: &ast::Assign,
        receiver: &ast::Expr,
        index: &ast::Expr,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let array = self.lower_expr(receiver, &mut sink, None)?;
        let element_ty = match self.types[array.ty].clone() {
            Type::MutableArray(element) => element,
            Type::Array(_) => {
                let found = self.type_name(array.ty);
                self.error(
                    receiver.span(),
                    format!("cannot assign to an element of immutable {found}"),
                );
                return None;
            }
            _ => {
                let found = self.type_name(array.ty);
                self.error(
                    receiver.span(),
                    format!("subscript is only supported on arrays, found {found}"),
                );
                return None;
            }
        };
        let index = self.lower_expr(index, &mut sink, Some(self.int))?;
        if index.ty != self.int {
            let found = self.type_name(index.ty);
            self.error(
                index.span,
                format!("array index must be Int, found {found}"),
            );
            return None;
        }
        let value = self.lower_expr(&assign.value, &mut sink, Some(element_ty))?;
        if !self.is_subtype(value.ty, element_ty) {
            let expected = self.type_name(element_ty);
            let found = self.type_name(value.ty);
            self.error(
                assign.value.span(),
                format!(
                    "cannot assign value of type {found} to an array element of type {expected}"
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, element_ty);
        out.extend(sink);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Index { array, index },
            value,
        })
    }

    /// `if` / `while` conditions must be `Boolean`.
    fn lower_condition(
        &mut self,
        cond: &ast::Expr,
        keyword: &str,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let cond = self.lower_expr(cond, sink, None)?;
        if cond.ty != self.boolean {
            let found = self.type_name(cond.ty);
            self.error(
                cond.span,
                format!("{keyword} condition must be Boolean, found {found}"),
            );
            return None;
        }
        Some(cond)
    }
}

/// Whether control can reach the end of a statement list. Once one
/// statement cannot fall through, later statements are unreachable
/// and cannot make the list fall through again.
fn statements_can_fall_through(statements: &[hir::Statement]) -> bool {
    statements.iter().all(statement_can_fall_through)
}

/// Structured fallthrough analysis for the non-Unit return rule.
/// `when` is exhaustive by the time HIR exists; an empty arm list is
/// kept conservative for malformed HIR produced after diagnostics.
fn statement_can_fall_through(statement: &hir::Statement) -> bool {
    match &statement.kind {
        hir::StatementKind::Return { .. } | hir::StatementKind::Throw(_) => false,
        hir::StatementKind::If {
            then_body,
            else_body,
            ..
        } => {
            statements_can_fall_through(then_body)
                || else_body.as_deref().is_none_or(statements_can_fall_through)
        }
        hir::StatementKind::When(when) => {
            if when.arms.is_empty() && when.else_body.is_none() {
                return true;
            }
            when.arms
                .iter()
                .any(|arm| statements_can_fall_through(&arm.body))
                || when
                    .else_body
                    .as_deref()
                    .is_some_and(statements_can_fall_through)
        }
        hir::StatementKind::Try(try_) => {
            if try_
                .finally_body
                .as_deref()
                .is_some_and(|body| !statements_can_fall_through(body))
            {
                return false;
            }
            statements_can_fall_through(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| statements_can_fall_through(&catch.body))
        }
        hir::StatementKind::Expr(_)
        | hir::StatementKind::ValDecl { .. }
        | hir::StatementKind::Assign { .. }
        | hir::StatementKind::While { .. } => true,
    }
}
