use super::*;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_expression(
        &mut self,
        expression: &ast::Expr,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
    ) -> Option<EvaluatedConst> {
        match expression {
            ast::Expr::IntLiteral(literal) => {
                self.evaluate_integer_literal(*literal, expected, false, literal.span)
            }
            ast::Expr::FloatLiteral(literal) => {
                let expression =
                    self.lower_float_literal(literal, expected, false, literal.span)?;
                let hir::ExprKind::FloatLiteral(value) = expression.kind else {
                    unreachable!("float literal lowering produces a floating constant")
                };
                Some(EvaluatedConst {
                    value: hir::ConstPropertyValue::Float(value),
                    ty: expression.ty,
                })
            }
            ast::Expr::CharLiteral { value, .. } => Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Char(*value),
                ty: self.core_character_type().ok()?,
            }),
            ast::Expr::BoolLiteral { value, .. } => Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Boolean(*value),
                ty: self.boolean,
            }),
            ast::Expr::StringLiteral { value, .. } => Some(EvaluatedConst {
                value: hir::ConstPropertyValue::String(value.clone()),
                ty: self.string,
            }),
            ast::Expr::Var(name) => {
                let current_owner = stack
                    .last()
                    .map(|index| declarations[*index].owner)
                    .unwrap_or(hir::PropertyOwner::TopLevel);
                let Some(target) =
                    self.find_const_definition(declarations, current_owner, &name.text, file, true)
                else {
                    let lookup = self.lookup_value_origin(&name.text);
                    if let crate::imports::lookup::LookupResult::Unique(
                        crate::imports::lookup::values::ValueOrigin::Dependency(binding),
                    ) = &lookup
                    {
                        let imported =
                            self.select_imported_dependency_constant(binding, name.span)?;
                        return Some(EvaluatedConst {
                            value: imported.value,
                            ty: imported.ty,
                        });
                    }
                    if matches!(
                        &lookup,
                        crate::imports::lookup::LookupResult::Ambiguous { .. }
                    ) {
                        self.resolve_value_origin(name).ok()?;
                    }
                    let message = if matches!(
                        &lookup,
                        crate::imports::lookup::LookupResult::Unique(_)
                    ) {
                        format!(
                            "const initializer may only reference const properties; `{}` is not const",
                            name.text
                        )
                    } else if declarations.iter().any(|candidate| {
                        candidate.declaration.name.text == name.text
                            && (candidate.owner != hir::PropertyOwner::TopLevel
                                || self
                                    .top_level_namespaces
                                    .source_lookup_rank(file, candidate.file)
                                    .is_some())
                    }) {
                        format!("const property `{}` is not accessible here", name.text)
                    } else if self.has_top_level_property_candidate(&name.text)
                        || ordinary.iter().any(|candidate| {
                            self.top_level_namespaces
                                .source_lookup_rank(file, candidate.file)
                                .is_some()
                                && candidate.declaration.name.text == name.text
                                && (candidate.access.declared != hir::DeclaredVisibility::Private
                                    || candidate.file == file)
                        })
                    {
                        format!(
                            "const initializer may only reference const properties; `{}` is not const",
                            name.text
                        )
                    } else {
                        format!("unknown const property `{}`", name.text)
                    };
                    self.error(name.span, message);
                    return None;
                };
                self.evaluate_const_definition(
                    target,
                    declarations,
                    ordinary,
                    states,
                    stack,
                    Some(name.span),
                )
            }
            ast::Expr::FieldAccess(access)
                if access.navigation == ast::Navigation::Direct
                    && matches!(access.selector, ast::FieldSelector::Name(_)) =>
            {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    unreachable!("the match guard selected a named field")
                };
                if let Some((value, ty)) =
                    self.lower_imported_const_reference(access, expected).ok()?
                {
                    return Some(EvaluatedConst { value, ty });
                }
                let target = self
                    .complete_companion_qualifier(&access.receiver)
                    .ok()?
                    .map(|(target, _)| target);
                let Some(target) = target else {
                    self.error(
                        access.span,
                        "const initializer qualifiers must name an object or a companion host"
                            .to_string(),
                    );
                    return None;
                };
                let Some(definition) =
                    self.find_qualified_const_definition(declarations, target, &name.text, file)
                else {
                    self.error(
                        name.span,
                        format!(
                            "qualified singleton has no accessible const property `{}`",
                            name.text
                        ),
                    );
                    return None;
                };
                self.evaluate_const_definition(
                    definition,
                    declarations,
                    ordinary,
                    states,
                    stack,
                    Some(name.span),
                )
            }
            ast::Expr::Unary { op, operand, span } => {
                if *op == ast::UnOp::Neg
                    && let ast::Expr::IntLiteral(literal) = &**operand
                    && matches!(
                        literal.suffix,
                        ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                    )
                {
                    return self.evaluate_integer_literal(*literal, expected, true, *span);
                }
                let operand = self.evaluate_const_expression(
                    operand,
                    expected,
                    file,
                    declarations,
                    ordinary,
                    states,
                    stack,
                )?;
                self.evaluate_const_unary(*op, operand, *span)
            }
            ast::Expr::Binary { op, lhs, rhs, span } => self.evaluate_const_binary(
                *op,
                lhs,
                rhs,
                expected,
                file,
                declarations,
                ordinary,
                states,
                stack,
                *span,
            ),
            ast::Expr::InfixCall {
                lhs,
                target,
                rhs,
                span,
            } => self.evaluate_const_integer_infix(
                lhs,
                target,
                rhs,
                expected,
                file,
                declarations,
                ordinary,
                states,
                stack,
                *span,
            ),
            ast::Expr::MethodCall {
                receiver,
                name,
                navigation,
                type_args,
                args,
                span,
            } => self.evaluate_const_integer_method(
                receiver,
                name,
                *navigation,
                type_args,
                args,
                expected,
                file,
                declarations,
                ordinary,
                states,
                stack,
                *span,
            ),
            _ => {
                self.error(
                    expression.span(),
                    "const initializer must contain only literals, const references, built-in primitive operators, and exact core numeric intrinsic calls"
                        .to_string(),
                );
                None
            }
        }
    }
}
