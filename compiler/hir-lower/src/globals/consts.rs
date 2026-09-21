use scoop_ast as ast;
use scoop_hir as hir;

use super::{PendingConst, PendingOrdinary};
use crate::Lowerer;

mod dependencies;
mod integer_authority;
mod integer_calls;
mod integer_intrinsics;
mod integer_values;

pub(super) use integer_intrinsics::{ConstIntegerIntrinsicKind, ResolvedConstIntegerIntrinsic};
pub(super) use integer_values::{
    IntegerBinaryResult, convert_integer_constant, evaluate_integer_binary,
    evaluate_integer_no_gc_operation,
};
pub(crate) use integer_values::{evaluate_hir_integer_constant, integer_wrapping_neg};

#[derive(Clone)]
enum ConstState {
    Pending,
    Visiting,
    Complete(EvaluatedConst),
    Failed,
}

#[derive(Clone)]
struct EvaluatedConst {
    value: hir::ConstPropertyValue,
    ty: hir::TypeId,
}

impl Lowerer {
    pub(super) fn resolve_const_properties(
        &mut self,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
    ) {
        let cyclic = self.diagnose_const_dependency_cycles(declarations);
        let mut states = (0..declarations.len())
            .map(|index| {
                if cyclic.contains(&index) {
                    ConstState::Failed
                } else {
                    ConstState::Pending
                }
            })
            .collect::<Vec<_>>();
        for index in 0..declarations.len() {
            let mut stack = Vec::new();
            self.evaluate_const_definition(
                index,
                declarations,
                ordinary,
                &mut states,
                &mut stack,
                None,
            );
        }

        for (index, declaration) in declarations.iter().enumerate() {
            let ConstState::Complete(value) = states[index].clone() else {
                continue;
            };
            self.current_file = declaration.file;
            self.current_owner = match declaration.owner {
                hir::PropertyOwner::Object(object) => Some(crate::Owner::Object(object)),
                hir::PropertyOwner::TopLevel => None,
                _ => unreachable!("the const worklist contains only top-level and object values"),
            };
            let expected = self.next_property_id();
            let capability = self.allocate_const_capability(
                declaration.access.clone(),
                declaration.declaration.span,
            );
            let property = self.properties.alloc(hir::Property {
                owner: declaration.owner,
                name: declaration.declaration.name.text.clone(),
                access: declaration.access.clone(),
                modifier: hir::MethodModifier::Final,
                is_override: false,
                overrides: Vec::new(),
                override_access: Vec::new(),
                ty: declaration.ty,
                capability,
                representation: hir::PropertyRepresentation::Const { value: value.value },
                span: declaration.declaration.span,
            });
            assert_eq!(property, expected);
            match declaration.owner {
                hir::PropertyOwner::TopLevel => self.top_level_namespaces.register_property(
                    declaration.file,
                    declaration.declaration.name.text.clone(),
                    property,
                    false,
                ),
                hir::PropertyOwner::Object(object) => {
                    let backing = self.objects[object].backing_class;
                    self.classes[backing].properties.push(property);
                }
                _ => unreachable!("the const worklist contains only top-level and object values"),
            }
            self.property_files.insert(property, declaration.file);
            self.imports
                .bind_property(declaration.import_source, property);
        }
        self.current_owner = None;
    }

    fn evaluate_const_definition(
        &mut self,
        index: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        use_span: Option<ast::Span>,
    ) -> Option<EvaluatedConst> {
        match states[index].clone() {
            ConstState::Complete(value) => return Some(value),
            ConstState::Failed => return None,
            ConstState::Visiting => {
                let cycle_start = stack
                    .iter()
                    .position(|candidate| *candidate == index)
                    .expect("a visiting const is present on the evaluation stack");
                let mut path = stack[cycle_start..]
                    .iter()
                    .map(|candidate| self.const_definition_name(&declarations[*candidate]))
                    .collect::<Vec<_>>();
                path.push(self.const_definition_name(&declarations[index]));
                self.error(
                    use_span.unwrap_or(declarations[index].declaration.span),
                    format!("const dependency cycle: {}", path.join(" -> ")),
                );
                return None;
            }
            ConstState::Pending => {}
        }

        states[index] = ConstState::Visiting;
        stack.push(index);
        let declaration = &declarations[index];
        let previous_file = self.current_file;
        let previous_owner = self.current_owner;
        self.current_file = declaration.file;
        self.current_owner = match declaration.owner {
            hir::PropertyOwner::Object(object) => Some(crate::Owner::Object(object)),
            hir::PropertyOwner::TopLevel => None,
            _ => unreachable!("the const worklist contains only top-level and object values"),
        };
        let ast::PropertyBodySyntax::Const(expression) = &declaration.declaration.body else {
            unreachable!("the const worklist contains only const properties")
        };
        let evaluated = self.evaluate_const_expression(
            expression,
            Some(declaration.ty),
            declaration.file,
            declarations,
            ordinary,
            states,
            stack,
        );
        let result = evaluated.and_then(|evaluated| {
            if self.types_equal(evaluated.ty, declaration.ty) {
                Some(evaluated)
            } else {
                self.error(
                    expression.span(),
                    format!(
                        "const initializer of `{}` must be of type {}, found {}",
                        declaration.declaration.name.text,
                        self.type_name(declaration.ty),
                        self.type_name(evaluated.ty)
                    ),
                );
                None
            }
        });
        self.current_file = previous_file;
        self.current_owner = previous_owner;
        stack.pop();
        states[index] = result
            .clone()
            .map_or(ConstState::Failed, ConstState::Complete);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_const_expression(
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
                let alias_target = match self.resolve_direct_type_alias_qualifier(&access.receiver)
                {
                    Ok(alias) => alias.map(|(_, target)| target),
                    Err(()) => return None,
                };
                let target =
                    alias_target.or_else(|| self.nominal_qualifier_target(&access.receiver));
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
                    "const initializer must contain only literals, const references, built-in primitive operators, and exact core integer intrinsic calls"
                        .to_string(),
                );
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn find_qualified_const_definition(
        &self,
        declarations: &[PendingConst<'_>],
        qualifier: crate::NominalTarget,
        name: &str,
        file: usize,
    ) -> Option<usize> {
        let direct = match qualifier {
            crate::NominalTarget::Object(object) => self.find_const_definition(
                declarations,
                hir::PropertyOwner::Object(object),
                name,
                file,
                false,
            ),
            _ => None,
        };
        direct.or_else(|| {
            let companion = self.companion_object(qualifier.owner())?;
            self.find_const_definition(
                declarations,
                hir::PropertyOwner::Object(companion),
                name,
                file,
                false,
            )
        })
    }

    fn find_const_definition(
        &self,
        declarations: &[PendingConst<'_>],
        owner: hir::PropertyOwner,
        name: &str,
        file: usize,
        fallback_to_top_level: bool,
    ) -> Option<usize> {
        let find = |wanted_owner| {
            if wanted_owner == hir::PropertyOwner::TopLevel {
                match self.lookup_value_origin(name) {
                    crate::imports::lookup::LookupResult::Unique(
                        crate::imports::lookup::values::ValueOrigin::CurrentUnit(binding),
                    ) => {
                        let crate::imports::CurrentUnitTarget::SourceProperty(source) =
                            self.imports.binding(binding).target
                        else {
                            return None;
                        };
                        return declarations.iter().position(|candidate| matches!(candidate.import_source, crate::imports::PropertyImportSource::CurrentUnit(id) if id == source));
                    }
                    crate::imports::lookup::LookupResult::Missing
                    | crate::imports::lookup::LookupResult::Inaccessible(_)
                    | crate::imports::lookup::LookupResult::Unique(
                        crate::imports::lookup::values::ValueOrigin::Core(
                            crate::imports::lookup::values::ValueTarget::Property(_),
                        ),
                    ) => {}
                    _ => return None,
                }
                // Embedded prelude constants have no current-unit import ids and
                // are evaluated before their ordinary prelude properties exist.
                let candidates = declarations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (!self.source_is_current_cone(candidate.file)
                            && candidate.owner == wanted_owner
                            && candidate.declaration.name.text == name
                            && self.access_domain_allows(&candidate.access.lookup.0))
                        .then_some(index)
                    })
                    .collect::<Vec<_>>();
                return match candidates.as_slice() {
                    [one] => Some(*one),
                    _ => None,
                };
            }
            declarations
                .iter()
                .enumerate()
                .find_map(|(index, candidate)| {
                    (candidate.owner == wanted_owner
                        && candidate.declaration.name.text == name
                        && (candidate.access.declared != hir::DeclaredVisibility::Private
                            || candidate.file == file
                            || self.access_domain_allows(&candidate.access.lookup.0)))
                    .then_some(index)
                })
        };
        find(owner).or_else(|| {
            (fallback_to_top_level && owner != hir::PropertyOwner::TopLevel)
                .then(|| find(hir::PropertyOwner::TopLevel))
                .flatten()
        })
    }

    fn const_definition_name(&self, declaration: &PendingConst<'_>) -> String {
        match declaration.owner {
            hir::PropertyOwner::TopLevel => declaration.declaration.name.text.clone(),
            hir::PropertyOwner::Object(object) => format!(
                "{}.{}",
                crate::Owner::Object(object).describe_name(self),
                declaration.declaration.name.text
            ),
            _ => unreachable!("the const worklist contains only top-level and object values"),
        }
    }

    fn evaluate_const_unary(
        &mut self,
        operator: ast::UnOp,
        operand: EvaluatedConst,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        match (operator, operand.value) {
            (ast::UnOp::Plus | ast::UnOp::Neg, hir::ConstPropertyValue::Integer(value)) => {
                let hir::Type::Integer(kind) = self.types[operand.ty] else {
                    self.error(
                        span,
                        "integer const operator requires an integer operand".to_string(),
                    );
                    return None;
                };
                let operation = match operator {
                    ast::UnOp::Plus => hir::NoGcIntegerOperation::UnaryPlus,
                    ast::UnOp::Neg => hir::NoGcIntegerOperation::UnaryMinus,
                    ast::UnOp::Not => {
                        unreachable!("the outer match selected an integer unary operator")
                    }
                };
                if !self.const_integer_operation_available(
                    hir::IntegerIntrinsicKind::NoGcOperation { kind, operation },
                ) {
                    self.error(
                        span,
                        "const integer operator did not resolve to the exact typed core intrinsic"
                            .to_string(),
                    );
                    return None;
                }
                Some(EvaluatedConst {
                    value: evaluate_integer_no_gc_operation(operation, value, None)
                        .expect("a typed unary integer intrinsic accepts one owner operand"),
                    ty: self.integer_no_gc_result_type(kind, operation),
                })
            }
            (ast::UnOp::Not, hir::ConstPropertyValue::Boolean(value)) => Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Boolean(!value),
                ty: self.boolean,
            }),
            _ => {
                self.error(
                    span,
                    "invalid unary operator in const initializer".to_string(),
                );
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_const_binary(
        &mut self,
        operator: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        if matches!(operator, ast::BinOp::And | ast::BinOp::Or) {
            let lhs = self.evaluate_const_expression(
                lhs,
                Some(self.boolean),
                file,
                declarations,
                ordinary,
                states,
                stack,
            );
            let rhs = self.evaluate_const_expression(
                rhs,
                Some(self.boolean),
                file,
                declarations,
                ordinary,
                states,
                stack,
            );
            let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                return None;
            };
            if !self.types_equal(lhs.ty, rhs.ty) {
                self.error(
                    span,
                    format!(
                        "const operator operands must have the same type, found {} and {}",
                        self.type_name(lhs.ty),
                        self.type_name(rhs.ty)
                    ),
                );
                return None;
            }
            let (hir::ConstPropertyValue::Boolean(lhs), hir::ConstPropertyValue::Boolean(rhs)) =
                (lhs.value, rhs.value)
            else {
                self.error(
                    span,
                    "boolean const operator requires Boolean operands".to_string(),
                );
                return None;
            };
            let value = match operator {
                ast::BinOp::And => lhs && rhs,
                ast::BinOp::Or => lhs || rhs,
                _ => unreachable!("the outer match selected a boolean short-circuit operator"),
            };
            return Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Boolean(value),
                ty: self.boolean,
            });
        }

        let equality = matches!(operator, ast::BinOp::Eq | ast::BinOp::Ne);
        let operand_kind = if equality {
            self.select_const_equality_integer_kind(
                lhs,
                rhs,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )
        } else {
            self.select_const_binary_literal_kind(operator, lhs, expected, |kind| {
                self.probe_const_integer_kind(
                    rhs,
                    Some(kind),
                    file,
                    declarations,
                    ordinary,
                    states,
                    stack,
                ) == Some(kind)
            })
        };
        let operand_expected = operand_kind.map(|kind| self.integer_type(kind));
        let lhs = self.evaluate_const_expression(
            lhs,
            operand_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let rhs_expected = if equality {
            operand_expected
        } else {
            Some(lhs.ty)
        };
        let rhs = self.evaluate_const_expression(
            rhs,
            rhs_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        if !self.types_equal(lhs.ty, rhs.ty) {
            self.error(
                span,
                format!(
                    "const operator operands must have the same type, found {} and {}",
                    self.type_name(lhs.ty),
                    self.type_name(rhs.ty)
                ),
            );
            return None;
        }

        let result = match (lhs.value, rhs.value) {
            (hir::ConstPropertyValue::Integer(left), hir::ConstPropertyValue::Integer(right)) => {
                let hir::Type::Integer(kind) = self.types[lhs.ty] else {
                    unreachable!("integer constant values have integer types")
                };
                debug_assert_eq!(left.kind(), kind);
                debug_assert_eq!(right.kind(), kind);
                match self.evaluate_typed_integer_binary_operator(operator, left, right) {
                    IntegerBinaryResult::Value(value) => Some(value),
                    IntegerBinaryResult::DivisionByZero => {
                        self.error(span, "division by zero in const initializer".to_string());
                        return None;
                    }
                    IntegerBinaryResult::Unsupported => None,
                }
            }
            (hir::ConstPropertyValue::Boolean(left), hir::ConstPropertyValue::Boolean(right)) => {
                match operator {
                    ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                    ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                    _ => None,
                }
            }
            (hir::ConstPropertyValue::String(left), hir::ConstPropertyValue::String(right)) => {
                match operator {
                    ast::BinOp::Add => Some(hir::ConstPropertyValue::String(left + &right)),
                    ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                    ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                    ast::BinOp::Lt => Some(hir::ConstPropertyValue::Boolean(left < right)),
                    ast::BinOp::Le => Some(hir::ConstPropertyValue::Boolean(left <= right)),
                    ast::BinOp::Gt => Some(hir::ConstPropertyValue::Boolean(left > right)),
                    ast::BinOp::Ge => Some(hir::ConstPropertyValue::Boolean(left >= right)),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(value) = result else {
            self.error(
                span,
                "invalid binary operator in const initializer".to_string(),
            );
            return None;
        };
        let ty = match value {
            hir::ConstPropertyValue::Boolean(_) => self.boolean,
            hir::ConstPropertyValue::String(_) => self.string,
            hir::ConstPropertyValue::Integer(_) => lhs.ty,
        };
        Some(EvaluatedConst { value, ty })
    }

    fn evaluate_integer_literal(
        &mut self,
        literal: ast::IntegerLiteralSyntax,
        expected: Option<hir::TypeId>,
        negative: bool,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        let expression = self.lower_integer_literal(literal, expected, negative, span)?;
        let hir::ExprKind::IntegerLiteral(value) = expression.kind else {
            unreachable!("integer literal lowering produces a typed integer constant")
        };
        Some(EvaluatedConst {
            value: hir::ConstPropertyValue::Integer(value),
            ty: expression.ty,
        })
    }
}
