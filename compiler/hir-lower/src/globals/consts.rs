use scoop_ast as ast;
use scoop_hir as hir;

use super::{PendingConst, PendingOrdinary};
use crate::Lowerer;

mod binary;
mod enumerations;
mod expression;
mod floating_calls;
mod floating_inference;
mod floating_values;
pub(super) use floating_calls::{ResolvedConstFloatIntrinsic, evaluate_float_call};
pub(super) use floating_values::{
    evaluate_float_binary, evaluate_float_unary, float_binary_operator,
};
mod dependencies;
mod integer_authority;
mod integer_calls;
mod integer_intrinsics;
mod integer_values;
mod lookup;

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
        enums: &[(hir::EnumId, ast::EnumSource<'_>, usize)],
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

        let mut allocated = vec![None; declarations.len()];
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
                ty: value.ty,
                capability,
                representation: hir::PropertyRepresentation::Const { value: value.value },
                span: declaration.declaration.span,
            });
            assert_eq!(property, expected);
            self.property_name_spans
                .insert(property, declaration.declaration.name.span);
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
            allocated[index] = Some(property);
        }
        if let Some(observation) = &mut self.editing_constants {
            observation.resolve_pending(&allocated);
        }
        self.resolve_integral_enum_constants(enums, declarations, ordinary, &mut states);
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
        let observation = self.begin_constant_observation();
        let evaluated = self.evaluate_const_expression(
            expression,
            declaration.ty,
            declaration.file,
            declarations,
            ordinary,
            states,
            stack,
        );
        let result = evaluated.and_then(|evaluated| {
            if !matches!(
                self.types[evaluated.ty],
                hir::Type::Integer(_) | hir::Type::Boolean | hir::Type::String
            ) && !self.is_char_type(evaluated.ty)
                && self.float_kind(evaluated.ty).is_none()
            {
                self.error(
                    declaration.declaration.type_span(),
                    format!(
                        "const property `{}` has unsupported type {}",
                        declaration.declaration.name.text,
                        self.type_name(evaluated.ty)
                    ),
                );
                return None;
            }
            if declaration
                .ty
                .is_none_or(|expected| self.types_equal(evaluated.ty, expected))
            {
                Some(evaluated)
            } else {
                self.error(
                    expression.span(),
                    format!(
                        "const initializer of `{}` must be of type {}, found {}",
                        declaration.declaration.name.text,
                        self.type_name(
                            declaration
                                .ty
                                .expect("an incompatible const has an explicit type")
                        ),
                        self.type_name(evaluated.ty)
                    ),
                );
                None
            }
        });
        self.finish_constant_observation(observation, result.is_some());
        self.current_file = previous_file;
        self.current_owner = previous_owner;
        stack.pop();
        states[index] = result
            .clone()
            .map_or(ConstState::Failed, ConstState::Complete);
        result
    }

    fn evaluate_const_unary(
        &mut self,
        operator: ast::UnOp,
        operand: EvaluatedConst,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        match (operator, operand.value) {
            (ast::UnOp::Plus | ast::UnOp::Neg, hir::ConstPropertyValue::Float(value)) => {
                Some(EvaluatedConst {
                    value: evaluate_float_unary(
                        if operator == ast::UnOp::Plus {
                            hir::FloatUnaryOperator::Plus
                        } else {
                            hir::FloatUnaryOperator::Negate
                        },
                        value,
                    ),
                    ty: operand.ty,
                })
            }
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

pub(super) fn evaluate_character_binary(
    operator: ast::BinOp,
    left: char,
    right: char,
) -> Option<hir::ConstPropertyValue> {
    let value = match operator {
        ast::BinOp::Eq => left == right,
        ast::BinOp::Ne => left != right,
        ast::BinOp::Lt => left < right,
        ast::BinOp::Le => left <= right,
        ast::BinOp::Gt => left > right,
        ast::BinOp::Ge => left >= right,
        _ => return None,
    };
    Some(hir::ConstPropertyValue::Boolean(value))
}
