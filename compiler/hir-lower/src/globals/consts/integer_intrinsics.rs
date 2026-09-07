use scoop_ast as ast;
use scoop_hir as hir;

use super::integer_values::{
    IntegerBinaryResult, evaluate_integer_binary, evaluate_integer_no_gc_operation, signed_value,
};
use crate::Lowerer;

#[derive(Clone, Copy)]
pub(in crate::globals) struct ResolvedConstIntegerIntrinsic {
    pub(in crate::globals) kind: ConstIntegerIntrinsicKind,
    pub(in crate::globals) function: hir::FunctionId,
}

#[derive(Clone, Copy)]
pub(in crate::globals) enum ConstIntegerIntrinsicKind {
    NoGcOperation(hir::NoGcIntegerOperation),
    ManagedDivRem(hir::IntegerDivRem),
    Conversion(hir::IntegerKind),
}

impl Lowerer {
    pub(in crate::globals) fn resolve_const_integer_intrinsic(
        &self,
        source: hir::IntegerKind,
        source_name: &str,
    ) -> Option<ResolvedConstIntegerIntrinsic> {
        for operation in hir::NoGcIntegerOperation::ALL {
            let Some(function) = self.registered_no_gc_integer_operation(source, operation) else {
                continue;
            };
            if self.const_intrinsic_source_name(function) == source_name {
                return Some(ResolvedConstIntegerIntrinsic {
                    kind: ConstIntegerIntrinsicKind::NoGcOperation(operation),
                    function,
                });
            }
        }
        for operation in hir::IntegerDivRem::ALL {
            let key =
                hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::ManagedOperation {
                    kind: source,
                    operation,
                });
            let Some(&(function, _)) = self.intrinsic_functions.get(&key) else {
                continue;
            };
            if hir::ManagedCallableRef::try_from_function(function, &self.functions).is_some()
                && self.const_intrinsic_source_name(function) == source_name
            {
                return Some(ResolvedConstIntegerIntrinsic {
                    kind: ConstIntegerIntrinsicKind::ManagedDivRem(operation),
                    function,
                });
            }
        }
        for target_kind in hir::IntegerKind::ALL {
            let key = hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::Conversion {
                source,
                target_kind,
            });
            let Some(&(function, _)) = self.intrinsic_functions.get(&key) else {
                continue;
            };
            if hir::NoGcCallableRef::try_from_function(function, &self.functions).is_some()
                && self.const_intrinsic_source_name(function) == source_name
            {
                return Some(ResolvedConstIntegerIntrinsic {
                    kind: ConstIntegerIntrinsicKind::Conversion(target_kind),
                    function,
                });
            }
        }
        None
    }

    pub(in crate::globals) fn const_integer_receiver_expected(
        &self,
        source_name: &str,
        expected: Option<hir::TypeId>,
    ) -> Option<hir::TypeId> {
        let expected = expected?;
        let hir::Type::Integer(kind) = self.types[expected] else {
            return None;
        };
        let resolved = self.resolve_const_integer_intrinsic(kind, source_name)?;
        matches!(
            resolved.kind,
            ConstIntegerIntrinsicKind::NoGcOperation(
                hir::NoGcIntegerOperation::UnaryPlus
                    | hir::NoGcIntegerOperation::UnaryMinus
                    | hir::NoGcIntegerOperation::Inc
                    | hir::NoGcIntegerOperation::Dec
                    | hir::NoGcIntegerOperation::Add
                    | hir::NoGcIntegerOperation::Sub
                    | hir::NoGcIntegerOperation::Mul
                    | hir::NoGcIntegerOperation::And
                    | hir::NoGcIntegerOperation::Or
                    | hir::NoGcIntegerOperation::Xor
                    | hir::NoGcIntegerOperation::Inv
                    | hir::NoGcIntegerOperation::Shl
                    | hir::NoGcIntegerOperation::Shr
                    | hir::NoGcIntegerOperation::Ushr
            ) | ConstIntegerIntrinsicKind::ManagedDivRem(_)
        )
        .then_some(expected)
    }

    pub(in crate::globals) fn integer_no_gc_result_type(
        &self,
        source: hir::IntegerKind,
        operation: hir::NoGcIntegerOperation,
    ) -> hir::TypeId {
        match operation {
            hir::NoGcIntegerOperation::CompareTo => self.integer_type(hir::IntegerKind::SIGNED_64),
            hir::NoGcIntegerOperation::Equals => self.boolean,
            _ => self.integer_type(source),
        }
    }

    pub(in crate::globals) fn registered_no_gc_integer_operation(
        &self,
        source: hir::IntegerKind,
        operation: hir::NoGcIntegerOperation,
    ) -> Option<hir::FunctionId> {
        if !operation.supports(source) {
            return None;
        }
        let key = hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::NoGcOperation {
            kind: source,
            operation,
        });
        let &(function, _) = self.intrinsic_functions.get(&key)?;
        hir::NoGcCallableRef::try_from_function(function, &self.functions).map(|_| function)
    }

    pub(in crate::globals) fn evaluate_typed_integer_binary_operator(
        &self,
        operator: ast::BinOp,
        left: hir::HirIntegerConstant,
        right: hir::HirIntegerConstant,
    ) -> IntegerBinaryResult {
        if left.kind() != right.kind() {
            return IntegerBinaryResult::Unsupported;
        }
        let kind = left.kind();
        let operation = match operator {
            ast::BinOp::Add => Some(hir::NoGcIntegerOperation::Add),
            ast::BinOp::Sub => Some(hir::NoGcIntegerOperation::Sub),
            ast::BinOp::Mul => Some(hir::NoGcIntegerOperation::Mul),
            ast::BinOp::Eq | ast::BinOp::Ne => Some(hir::NoGcIntegerOperation::Equals),
            ast::BinOp::Lt | ast::BinOp::Le | ast::BinOp::Gt | ast::BinOp::Ge => {
                Some(hir::NoGcIntegerOperation::CompareTo)
            }
            ast::BinOp::Div
            | ast::BinOp::Rem
            | ast::BinOp::RangeTo
            | ast::BinOp::RangeUntil
            | ast::BinOp::Contains
            | ast::BinOp::NotContains
            | ast::BinOp::RefEq
            | ast::BinOp::RefNe
            | ast::BinOp::And
            | ast::BinOp::Or => None,
        };
        if let Some(operation) = operation {
            if self
                .registered_no_gc_integer_operation(kind, operation)
                .is_none()
            {
                return IntegerBinaryResult::Unsupported;
            }
            let Some(value) = evaluate_integer_no_gc_operation(operation, left, Some(right)) else {
                return IntegerBinaryResult::Unsupported;
            };
            return match (operator, value) {
                (ast::BinOp::Add | ast::BinOp::Sub | ast::BinOp::Mul | ast::BinOp::Eq, value) => {
                    IntegerBinaryResult::Value(value)
                }
                (ast::BinOp::Ne, hir::ConstPropertyValue::Boolean(value)) => {
                    IntegerBinaryResult::Value(hir::ConstPropertyValue::Boolean(!value))
                }
                (
                    ast::BinOp::Lt | ast::BinOp::Le | ast::BinOp::Gt | ast::BinOp::Ge,
                    hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed64(raw)),
                ) => {
                    let ordering = signed_value(hir::IntegerKind::SIGNED_64, raw);
                    let result = match operator {
                        ast::BinOp::Lt => ordering < 0,
                        ast::BinOp::Le => ordering <= 0,
                        ast::BinOp::Gt => ordering > 0,
                        ast::BinOp::Ge => ordering >= 0,
                        _ => unreachable!("the outer match selected an ordering operator"),
                    };
                    IntegerBinaryResult::Value(hir::ConstPropertyValue::Boolean(result))
                }
                _ => IntegerBinaryResult::Unsupported,
            };
        }

        let operation = match operator {
            ast::BinOp::Div => hir::IntegerDivRem::Div,
            ast::BinOp::Rem => hir::IntegerDivRem::Rem,
            _ => return IntegerBinaryResult::Unsupported,
        };
        let key =
            hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::ManagedOperation {
                kind,
                operation,
            });
        let Some(&(function, _)) = self.intrinsic_functions.get(&key) else {
            return IntegerBinaryResult::Unsupported;
        };
        if hir::ManagedCallableRef::try_from_function(function, &self.functions).is_none() {
            return IntegerBinaryResult::Unsupported;
        }
        evaluate_integer_binary(operator, left, right)
    }

    fn const_intrinsic_source_name(&self, function: hir::FunctionId) -> &str {
        self.functions[function]
            .name
            .rsplit('.')
            .next()
            .expect("function names are non-empty")
    }
}
