//! Validate floating members against the actual intrinsic nominal owners.

use super::*;
use scoop_identity::FloatConversion;

impl Lowerer {
    pub(super) fn validate_float_intrinsics(&mut self, files: &[ast::SourceFile]) {
        for intrinsic in hir::float_intrinsic_kinds() {
            let Some(function) =
                self.require_intrinsic(hir::IntrinsicFunctionKind::Float(intrinsic), files)
            else {
                continue;
            };
            let owner_kind = intrinsic.owner();
            let owner = match owner_kind {
                hir::IntrinsicTypeKind::Float(kind) => match self.core_float_type(kind) {
                    Ok(ty) => ty,
                    Err(_) => continue,
                },
                hir::IntrinsicTypeKind::Integer(kind) => self.integer_type(kind),
                _ => unreachable!("floating operations have numeric owners"),
            };
            let (result, operator, arity) = match intrinsic {
                hir::FloatIntrinsicKind::Unary { operation, .. } => {
                    let operator = match operation {
                        hir::FloatUnaryOperator::Plus => Some(hir::OperatorKind::UnaryPlus),
                        hir::FloatUnaryOperator::Negate => Some(hir::OperatorKind::UnaryMinus),
                        hir::FloatUnaryOperator::Increment => Some(hir::OperatorKind::Inc),
                        hir::FloatUnaryOperator::Decrement => Some(hir::OperatorKind::Dec),
                        _ => None,
                    };
                    (
                        if operation.is_predicate() {
                            self.boolean
                        } else {
                            owner
                        },
                        operator,
                        0,
                    )
                }
                hir::FloatIntrinsicKind::Binary { operation, .. } => {
                    let operator = match operation {
                        hir::FloatBinaryOperator::Add => Some(hir::OperatorKind::Plus),
                        hir::FloatBinaryOperator::Subtract => Some(hir::OperatorKind::Minus),
                        hir::FloatBinaryOperator::Multiply => Some(hir::OperatorKind::Times),
                        hir::FloatBinaryOperator::Divide => Some(hir::OperatorKind::Div),
                        hir::FloatBinaryOperator::Remainder => Some(hir::OperatorKind::Rem),
                        hir::FloatBinaryOperator::Equal => Some(hir::OperatorKind::Equals),
                        _ => None,
                    };
                    (
                        if operation.is_predicate() {
                            self.boolean
                        } else {
                            owner
                        },
                        operator,
                        1,
                    )
                }
                hir::FloatIntrinsicKind::Conversion(conversion) => {
                    let result = match conversion {
                        FloatConversion::FromInteger { target, .. }
                        | FloatConversion::BetweenFloats { target, .. } => {
                            let Ok(ty) = self.core_float_type(target) else {
                                continue;
                            };
                            ty
                        }
                        FloatConversion::ToInteger { target, .. } => self.integer_type(target),
                    };
                    (result, None, 0)
                }
            };
            let signature = &self.signatures[&function];
            let total_order = matches!(
                intrinsic,
                hir::FloatIntrinsicKind::Binary {
                    operation: hir::FloatBinaryOperator::TotalOrder,
                    ..
                }
            );
            let valid = self.function_has_intrinsic_owner(function, owner_kind)
                && signature.params.len() == arity
                && signature
                    .params
                    .iter()
                    .all(|parameter| parameter.ty == owner)
                && (!total_order || signature.params[0].name.text == "belowOrEqualTo")
                && signature.type_params.is_empty()
                && !signature.is_suspend
                && signature.return_ty == result
                && signature.modifiers.operator == operator
                && !signature.modifiers.is_infix;
            if !valid {
                self.malformed_operator_intrinsic(function, &intrinsic.name());
            }
        }
    }
}
