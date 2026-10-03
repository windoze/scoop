use scoop_ast as ast;
use scoop_hir as hir;

use super::integer_values::{
    IntegerBinaryResult, evaluate_integer_binary, evaluate_integer_no_gc_operation, signed_value,
};
use crate::Lowerer;

pub(in crate::globals) struct ResolvedConstIntegerIntrinsic {
    pub(in crate::globals) kind: ConstIntegerIntrinsicKind,
    pub(in crate::globals) is_infix: bool,
    pub(in crate::globals) parameters: Vec<String>,
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
        let operations = hir::NoGcIntegerOperation::ALL
            .into_iter()
            .filter(|operation| operation.supports(source))
            .map(|operation| {
                (
                    hir::IntegerIntrinsicKind::NoGcOperation {
                        kind: source,
                        operation,
                    },
                    ConstIntegerIntrinsicKind::NoGcOperation(operation),
                )
            })
            .chain(hir::IntegerDivRem::ALL.into_iter().map(|operation| {
                (
                    hir::IntegerIntrinsicKind::ManagedOperation {
                        kind: source,
                        operation,
                    },
                    ConstIntegerIntrinsicKind::ManagedDivRem(operation),
                )
            }))
            .chain(hir::IntegerKind::ALL.into_iter().map(|target_kind| {
                (
                    hir::IntegerIntrinsicKind::Conversion {
                        source,
                        target_kind,
                    },
                    ConstIntegerIntrinsicKind::Conversion(target_kind),
                )
            }));
        operations
            .filter_map(|(intrinsic, kind)| {
                self.const_integer_source_call(intrinsic, kind, source_name)
            })
            .next()
    }

    fn const_integer_source_call(
        &self,
        intrinsic: hir::IntegerIntrinsicKind,
        kind: ConstIntegerIntrinsicKind,
        source_name: &str,
    ) -> Option<ResolvedConstIntegerIntrinsic> {
        if !self.const_integer_operation_available(intrinsic) {
            return None;
        }
        let key = hir::IntrinsicFunctionKind::Integer(intrinsic);
        if let Some(&(function, _)) = self.intrinsic_functions.get(&key) {
            if self.const_intrinsic_source_name(function) != source_name {
                return None;
            }
            let signature = &self.signatures[&function];
            return Some(ResolvedConstIntegerIntrinsic {
                kind,
                is_infix: signature.modifiers.is_infix,
                parameters: signature
                    .params
                    .iter()
                    .map(|parameter| parameter.name.text.clone())
                    .collect(),
            });
        }
        let callable = self.dependencies.as_ref()?.intrinsic_callable(
            key,
            match intrinsic.gc_effect() {
                hir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
                hir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            },
        )?;
        if callable.name().as_str() != source_name {
            return None;
        }
        Some(ResolvedConstIntegerIntrinsic {
            kind,
            is_infix: callable.interface().effects().infix() == hir::CallableInfixV1::Infix,
            parameters: callable
                .source()
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| parameter.name().as_str().to_owned())
                .collect(),
        })
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
            if !self.const_integer_operation_available(hir::IntegerIntrinsicKind::NoGcOperation {
                kind,
                operation,
            }) {
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
        if !self.const_integer_operation_available(hir::IntegerIntrinsicKind::ManagedOperation {
            kind,
            operation,
        }) {
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
