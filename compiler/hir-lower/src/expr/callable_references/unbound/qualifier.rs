use super::*;
use crate::imports::lookup::calls::{ExpressionQualifierLookup, ExpressionQualifierTarget};

impl Lowerer {
    pub(in crate::expr::callable_references) fn reference_type_qualifier(
        &mut self,
        expression: &ast::Expr,
        expected: Option<TypeId>,
    ) -> Result<Option<TypeId>, ()> {
        if let Some(reference) = expression.applied_qualifier_type() {
            self.check_applied_qualifier_shadowing(&reference)?;
            return self.resolve_type_ref(&reference).map(Some).ok_or(());
        }
        let expected_receiver = self
            .expected_function_signature(expected)
            .and_then(|(_, signature)| signature.parameters().next().copied());
        let reference = match expression {
            ast::Expr::Var(name) => {
                if self.lexical_or_member_value_blocks_type_qualifier(&name.text) {
                    return Ok(None);
                }
                let lookup = self.lookup_expression_qualifier(name);
                if matches!(
                    lookup,
                    ExpressionQualifierLookup::Value
                        | ExpressionQualifierLookup::Unique(
                            ExpressionQualifierTarget::Object(_)
                                | ExpressionQualifierTarget::DependencyObject(_)
                        )
                ) {
                    return Ok(None);
                }
                if lookup == ExpressionQualifierLookup::Ambiguous {
                    self.error(
                        name.span,
                        format!("ambiguous callable reference qualifier `{}`", name.text),
                    );
                    return Err(());
                }
                let nominal = self.nominal_qualifier_target(expression);
                if let Some(receiver) = expected_receiver {
                    let direct_nominal =
                        self.lexical_nested_nominal_target(&name.text)
                            .or(match lookup {
                                ExpressionQualifierLookup::Unique(
                                    ExpressionQualifierTarget::Type(
                                        crate::namespace::TopLevelTypeTarget::Nominal(target),
                                    ),
                                ) => Some(target),
                                _ => None,
                            });
                    if direct_nominal.is_some()
                        && self.nominal_target_for_type(receiver) == direct_nominal
                    {
                        return Ok(Some(receiver));
                    }
                    if let ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::DependencyType(
                            hir::ImportedTarget::GenericType(owner),
                        ),
                    ) = lookup
                        && self.imported_nominal_owner(receiver)
                            == Some(hir::SourceNominalId::GenericTemplate(owner.persistent()))
                    {
                        return Ok(Some(receiver));
                    }
                }
                if nominal.is_none()
                    && !self.is_declared_type_name(name)
                    && self.source_type_alias_named(&name.text).is_none()
                    && !self
                        .type_params_in_scope
                        .iter()
                        .any(|parameter| parameter.name == name.text)
                    && !matches!(
                        lookup,
                        ExpressionQualifierLookup::Unique(
                            ExpressionQualifierTarget::Type(_)
                                | ExpressionQualifierTarget::DependencyType(_)
                        ) | ExpressionQualifierLookup::Inaccessible(_)
                    )
                {
                    return Ok(None);
                }
                ast::TypeRef {
                    kind: ast::TypeRefKind::Named(name.clone()),
                    span: name.span,
                }
            }
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
                if self.nominal_qualifier_target(expression).is_none()
                    && self
                        .resolve_imported_nominal_qualifier(expression)?
                        .is_none()
                {
                    return Ok(None);
                }
                let Some(reference) = qualifier_syntax(expression) else {
                    return Ok(None);
                };
                reference
            }
            _ => return Ok(None),
        };
        self.resolve_type_ref(&reference).map(Some).ok_or(())
    }
}

fn qualifier_syntax(expression: &ast::Expr) -> Option<ast::TypeRef> {
    match expression {
        ast::Expr::Var(name) => Some(ast::TypeRef {
            kind: ast::TypeRefKind::Named(name.clone()),
            span: name.span,
        }),
        ast::Expr::TypeQualifier(reference) => Some(reference.clone()),
        ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Direct => {
            let ast::FieldSelector::Name(name) = &access.selector else {
                return None;
            };
            Some(qualifier_syntax(&access.receiver)?.with_member(
                name.clone(),
                Vec::new(),
                access.span.end,
            ))
        }
        _ => None,
    }
}
