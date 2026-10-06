use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::globals::{PendingConst, PendingOrdinary};

use super::super::{
    ConstIntegerIntrinsicKind, ConstState, EvaluatedConst, IntegerBinaryResult,
    convert_integer_constant, evaluate_integer_binary, evaluate_integer_no_gc_operation,
};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::globals::consts) fn evaluate_const_integer_method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        navigation: ast::Navigation,
        type_args: &[ast::CallTypeArgument],
        args: &[ast::CallArgument],
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        if navigation != ast::Navigation::Direct || !type_args.is_empty() {
            self.error(
                span,
                "const integer intrinsics require a direct non-generic call".to_string(),
            );
            return None;
        }
        let receiver_kind = match args {
            [] => self.select_const_integer_literal_receiver_kind(
                receiver,
                None,
                &name.text,
                expected,
                false,
                file,
                declarations,
                ordinary,
                states,
                stack,
            ),
            [argument] if matches!(argument.spread, ast::SpreadSyntax::Plain) => self
                .select_const_integer_literal_receiver_kind(
                    receiver,
                    Some(&argument.expression),
                    &name.text,
                    expected,
                    false,
                    file,
                    declarations,
                    ordinary,
                    states,
                    stack,
                ),
            _ => None,
        };
        let receiver_expected = receiver_kind.map(|kind| self.integer_type(kind));
        let receiver = self.evaluate_const_expression(
            receiver,
            receiver_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        if let Some(resolved) = self.resolve_const_float_intrinsic(receiver.ty, &name.text) {
            return self.evaluate_const_float_call(
                resolved,
                receiver,
                args,
                file,
                declarations,
                ordinary,
                states,
                stack,
                span,
            );
        }
        if self.float_kind(receiver.ty).is_some() {
            self.error(
                name.span,
                "const initializer did not resolve to an exact typed core floating intrinsic"
                    .into(),
            );
            return None;
        }
        let hir::Type::Integer(source_kind) = self.types[receiver.ty] else {
            self.error(
                name.span,
                "integer const intrinsic requires an integer receiver".to_string(),
            );
            return None;
        };
        let hir::ConstPropertyValue::Integer(receiver_value) = receiver.value else {
            unreachable!("an integer-typed const value has an integer payload")
        };
        let Some(resolved) = self.resolve_const_integer_intrinsic(source_kind, &name.text) else {
            self.error(
                name.span,
                "const initializer did not resolve to the exact typed core integer intrinsic"
                    .to_string(),
            );
            return None;
        };
        match resolved.kind {
            ConstIntegerIntrinsicKind::NoGcOperation(operation) => {
                let right = match (operation.arity(), args) {
                    (hir::IntegerOperationArity::Unary, []) => None,
                    (hir::IntegerOperationArity::Binary, [argument])
                        if matches!(argument.spread, ast::SpreadSyntax::Plain)
                            && match &argument.name {
                                ast::CallArgumentName::Positional => true,
                                ast::CallArgumentName::Named(argument_name) => resolved
                                    .parameters
                                    .first()
                                    .is_some_and(|parameter| *parameter == argument_name.text),
                            } =>
                    {
                        let expected = if matches!(
                            operation,
                            hir::NoGcIntegerOperation::Shl
                                | hir::NoGcIntegerOperation::Shr
                                | hir::NoGcIntegerOperation::Ushr
                        ) {
                            self.integer_type(hir::IntegerKind::SIGNED_64)
                        } else {
                            receiver.ty
                        };
                        let value = self.evaluate_const_expression(
                            &argument.expression,
                            Some(expected),
                            file,
                            declarations,
                            ordinary,
                            states,
                            stack,
                        )?;
                        if !self.types_equal(value.ty, expected) {
                            self.error(
                                argument.span,
                                format!(
                                    "integer const intrinsic expected {}, found {}",
                                    self.type_name(expected),
                                    self.type_name(value.ty)
                                ),
                            );
                            return None;
                        }
                        let hir::ConstPropertyValue::Integer(value) = value.value else {
                            self.error(
                                argument.span,
                                "integer const intrinsic requires an integer argument".to_string(),
                            );
                            return None;
                        };
                        Some(value)
                    }
                    _ => {
                        self.error(
                            span,
                            "const integer intrinsic arguments do not match the exact typed core declaration"
                                .to_string(),
                        );
                        return None;
                    }
                };
                let value = evaluate_integer_no_gc_operation(operation, receiver_value, right)
                    .expect("validated integer intrinsic operands match its typed operation");
                Some(EvaluatedConst {
                    value,
                    ty: self.integer_no_gc_result_type(source_kind, operation),
                })
            }
            ConstIntegerIntrinsicKind::ManagedDivRem(operation) => {
                let [argument] = args else {
                    self.error(
                        span,
                        "const integer div/rem arguments do not match the exact typed core declaration"
                            .to_string(),
                    );
                    return None;
                };
                let named_argument_matches = match &argument.name {
                    ast::CallArgumentName::Positional => true,
                    ast::CallArgumentName::Named(argument_name) => resolved
                        .parameters
                        .first()
                        .is_some_and(|parameter| *parameter == argument_name.text),
                };
                if !matches!(argument.spread, ast::SpreadSyntax::Plain) || !named_argument_matches {
                    self.error(
                        span,
                        "const integer div/rem arguments do not match the exact typed core declaration"
                            .to_string(),
                    );
                    return None;
                }
                let right = self.evaluate_const_expression(
                    &argument.expression,
                    Some(receiver.ty),
                    file,
                    declarations,
                    ordinary,
                    states,
                    stack,
                )?;
                if !self.types_equal(right.ty, receiver.ty) {
                    self.error(
                        argument.span,
                        format!(
                            "integer const intrinsic expected {}, found {}",
                            self.type_name(receiver.ty),
                            self.type_name(right.ty)
                        ),
                    );
                    return None;
                }
                let hir::ConstPropertyValue::Integer(right_value) = right.value else {
                    self.error(
                        argument.span,
                        "integer const intrinsic requires an integer argument".to_string(),
                    );
                    return None;
                };
                let operator = match operation {
                    hir::IntegerDivRem::Div => ast::BinOp::Div,
                    hir::IntegerDivRem::Rem => ast::BinOp::Rem,
                };
                match evaluate_integer_binary(operator, receiver_value, right_value) {
                    IntegerBinaryResult::Value(value) => Some(EvaluatedConst {
                        value,
                        ty: receiver.ty,
                    }),
                    IntegerBinaryResult::DivisionByZero => {
                        self.error(span, "division by zero in const initializer".to_string());
                        None
                    }
                    IntegerBinaryResult::Unsupported => {
                        unreachable!("validated integer div/rem operands have matching kinds")
                    }
                }
            }
            ConstIntegerIntrinsicKind::Conversion(target_kind) => {
                if !args.is_empty() {
                    self.error(
                        span,
                        "const integer conversion arguments do not match the exact typed core declaration"
                            .to_string(),
                    );
                    return None;
                }
                Some(EvaluatedConst {
                    value: hir::ConstPropertyValue::Integer(convert_integer_constant(
                        receiver_value,
                        target_kind,
                    )),
                    ty: self.integer_type(target_kind),
                })
            }
        }
    }
}
