use scoop_ast as ast;
use scoop_hir as hir;

use super::{PendingConst, PendingOrdinary};
use crate::Lowerer;

mod binary;
mod expression;
mod floating_calls;
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
