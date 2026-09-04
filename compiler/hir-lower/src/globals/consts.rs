use scoop_ast as ast;
use scoop_hir as hir;

use super::{PendingConst, PendingOrdinary};
use crate::Lowerer;

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
            let expected = self.next_property_id();
            let capability = self.allocate_const_capability(
                declaration.access.clone(),
                declaration.declaration.span,
            );
            let property = self.properties.alloc(hir::Property {
                owner: hir::PropertyOwner::TopLevel,
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
            self.properties_by_name
                .entry(declaration.declaration.name.text.clone())
                .or_default()
                .push(property);
            self.property_files.insert(property, declaration.file);
        }
    }

    fn diagnose_const_dependency_cycles(
        &mut self,
        declarations: &[PendingConst<'_>],
    ) -> std::collections::HashSet<usize> {
        let graph = declarations
            .iter()
            .map(|declaration| {
                let ast::PropertyBodySyntax::Const(expression) = &declaration.declaration.body
                else {
                    unreachable!("the const worklist contains only const properties")
                };
                let mut dependencies = Vec::new();
                collect_const_dependencies(
                    expression,
                    declaration.file,
                    declarations,
                    &mut dependencies,
                );
                dependencies
            })
            .collect::<Vec<_>>();
        let mut states = vec![DependencyState::Pending; declarations.len()];
        let mut stack = Vec::new();
        let mut cycles = Vec::new();
        for index in 0..declarations.len() {
            find_const_cycles(
                index,
                &graph,
                declarations,
                &mut states,
                &mut stack,
                &mut cycles,
            );
        }
        let mut cyclic = std::collections::HashSet::new();
        for (span, path, members) in &cycles {
            self.error(
                *span,
                format!("const dependency cycle: {}", path.join(" -> ")),
            );
            cyclic.extend(members);
        }
        cyclic
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
                    .map(|candidate| declarations[*candidate].declaration.name.text.clone())
                    .collect::<Vec<_>>();
                path.push(declarations[index].declaration.name.text.clone());
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
        self.current_file = declaration.file;
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
            ast::Expr::IntLiteral { value, .. } => Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Integer(*value),
                ty: expected
                    .filter(|ty| matches!(self.types[*ty], hir::Type::Int | hir::Type::UInt))
                    .unwrap_or(self.int),
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
                let Some(target) =
                    declarations
                        .iter()
                        .enumerate()
                        .find_map(|(index, candidate)| {
                            (candidate.declaration.name.text == name.text
                                && (candidate.access.declared != hir::DeclaredVisibility::Private
                                    || candidate.file == file))
                                .then_some(index)
                        })
                else {
                    let message = if declarations
                        .iter()
                        .any(|candidate| candidate.declaration.name.text == name.text)
                    {
                        format!("const property `{}` is not accessible here", name.text)
                    } else if self.properties_by_name.contains_key(&name.text)
                        || ordinary.iter().any(|candidate| {
                            candidate.declaration.name.text == name.text
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
            ast::Expr::Unary { op, operand, span } => {
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
            _ => {
                self.error(
                    expression.span(),
                    "const initializer must contain only literals, const references, and built-in primitive operators"
                        .to_string(),
                );
                None
            }
        }
    }

    fn evaluate_const_unary(
        &mut self,
        operator: ast::UnOp,
        operand: EvaluatedConst,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        match (operator, operand.value) {
            (ast::UnOp::Plus, hir::ConstPropertyValue::Integer(value)) => Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Integer(value),
                ty: operand.ty,
            }),
            (ast::UnOp::Neg, hir::ConstPropertyValue::Integer(value))
                if matches!(self.types[operand.ty], hir::Type::Int) =>
            {
                Some(EvaluatedConst {
                    value: hir::ConstPropertyValue::Integer(value.wrapping_neg()),
                    ty: operand.ty,
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
            )?;
            let hir::ConstPropertyValue::Boolean(lhs) = lhs.value else {
                self.error(
                    span,
                    "boolean const operator requires Boolean operands".to_string(),
                );
                return None;
            };
            if (operator == ast::BinOp::And && !lhs) || (operator == ast::BinOp::Or && lhs) {
                return Some(EvaluatedConst {
                    value: hir::ConstPropertyValue::Boolean(lhs),
                    ty: self.boolean,
                });
            }
            let rhs = self.evaluate_const_expression(
                rhs,
                Some(self.boolean),
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            let hir::ConstPropertyValue::Boolean(rhs) = rhs.value else {
                self.error(
                    span,
                    "boolean const operator requires Boolean operands".to_string(),
                );
                return None;
            };
            return Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Boolean(rhs),
                ty: self.boolean,
            });
        }

        let operand_expected = match operator {
            ast::BinOp::Add
            | ast::BinOp::Sub
            | ast::BinOp::Mul
            | ast::BinOp::Div
            | ast::BinOp::Rem => expected,
            _ => None,
        };
        let lhs = self.evaluate_const_expression(
            lhs,
            operand_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let rhs = self.evaluate_const_expression(
            rhs,
            Some(lhs.ty),
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
                self.const_integer_binary(operator, left, right, lhs.ty, span)
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

    fn const_integer_binary(
        &mut self,
        operator: ast::BinOp,
        left: i64,
        right: i64,
        operand_ty: hir::TypeId,
        span: ast::Span,
    ) -> Option<hir::ConstPropertyValue> {
        let unsigned = matches!(self.types[operand_ty], hir::Type::UInt);
        let arithmetic = match operator {
            ast::BinOp::Add => Some(left.wrapping_add(right)),
            ast::BinOp::Sub => Some(left.wrapping_sub(right)),
            ast::BinOp::Mul => Some(left.wrapping_mul(right)),
            ast::BinOp::Div if right != 0 && unsigned => {
                Some(((left as u64) / (right as u64)) as i64)
            }
            ast::BinOp::Rem if right != 0 && unsigned => {
                Some(((left as u64) % (right as u64)) as i64)
            }
            ast::BinOp::Div if right != 0 => Some(left.checked_div(right).unwrap_or(i64::MIN)),
            ast::BinOp::Rem if right != 0 => Some(left.checked_rem(right).unwrap_or(0)),
            ast::BinOp::Div | ast::BinOp::Rem => {
                self.error(span, "division by zero in const initializer".to_string());
                return None;
            }
            _ => None,
        };
        if matches!(
            operator,
            ast::BinOp::Add | ast::BinOp::Sub | ast::BinOp::Mul | ast::BinOp::Div | ast::BinOp::Rem
        ) {
            return arithmetic.map(hir::ConstPropertyValue::Integer);
        }
        let comparison = if unsigned {
            let left = left as u64;
            let right = right as u64;
            match operator {
                ast::BinOp::Lt => left < right,
                ast::BinOp::Le => left <= right,
                ast::BinOp::Gt => left > right,
                ast::BinOp::Ge => left >= right,
                ast::BinOp::Eq => left == right,
                ast::BinOp::Ne => left != right,
                _ => return None,
            }
        } else {
            match operator {
                ast::BinOp::Lt => left < right,
                ast::BinOp::Le => left <= right,
                ast::BinOp::Gt => left > right,
                ast::BinOp::Ge => left >= right,
                ast::BinOp::Eq => left == right,
                ast::BinOp::Ne => left != right,
                _ => return None,
            }
        };
        Some(hir::ConstPropertyValue::Boolean(comparison))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DependencyState {
    Pending,
    Visiting,
    Complete,
}

fn collect_const_dependencies(
    expression: &ast::Expr,
    file: usize,
    declarations: &[PendingConst<'_>],
    dependencies: &mut Vec<(usize, ast::Span)>,
) {
    match expression {
        ast::Expr::Var(name) => {
            if let Some(index) = declarations
                .iter()
                .enumerate()
                .find_map(|(index, candidate)| {
                    (candidate.declaration.name.text == name.text
                        && (candidate.access.declared != hir::DeclaredVisibility::Private
                            || candidate.file == file))
                        .then_some(index)
                })
            {
                dependencies.push((index, name.span));
            }
        }
        ast::Expr::Unary { operand, .. } => {
            collect_const_dependencies(operand, file, declarations, dependencies);
        }
        ast::Expr::Binary { lhs, rhs, .. } => {
            collect_const_dependencies(lhs, file, declarations, dependencies);
            collect_const_dependencies(rhs, file, declarations, dependencies);
        }
        _ => {}
    }
}

fn find_const_cycles(
    index: usize,
    graph: &[Vec<(usize, ast::Span)>],
    declarations: &[PendingConst<'_>],
    states: &mut [DependencyState],
    stack: &mut Vec<usize>,
    cycles: &mut Vec<(ast::Span, Vec<String>, Vec<usize>)>,
) {
    match states[index] {
        DependencyState::Complete | DependencyState::Visiting => return,
        DependencyState::Pending => {}
    }
    states[index] = DependencyState::Visiting;
    stack.push(index);
    for &(dependency, span) in &graph[index] {
        if states[dependency] == DependencyState::Visiting {
            let cycle_start = stack
                .iter()
                .position(|candidate| *candidate == dependency)
                .expect("a visiting const is present on the dependency stack");
            let mut path = stack[cycle_start..]
                .iter()
                .map(|candidate| declarations[*candidate].declaration.name.text.clone())
                .collect::<Vec<_>>();
            path.push(declarations[dependency].declaration.name.text.clone());
            cycles.push((span, path, stack[cycle_start..].to_vec()));
        } else {
            find_const_cycles(dependency, graph, declarations, states, stack, cycles);
        }
    }
    stack.pop();
    states[index] = DependencyState::Complete;
}
