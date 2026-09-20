use scoop_ast as ast;
use scoop_hir as hir;

use super::{
    ConstIntegerIntrinsicKind, ConstState, EvaluatedConst, IntegerBinaryResult,
    ResolvedConstIntegerIntrinsic, convert_integer_constant, evaluate_integer_binary,
    evaluate_integer_no_gc_operation,
};
use crate::Lowerer;
use crate::globals::{PendingConst, PendingOrdinary};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_const_integer_kind(
        &self,
        expression: &ast::Expr,
        expected: Option<hir::IntegerKind>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::IntegerKind> {
        let mut probe = self.clone();
        let expected = expected.map(|kind| probe.integer_type(kind));
        let mut probe_states = states.to_vec();
        let mut probe_stack = stack.to_vec();
        let evaluated = probe.evaluate_const_expression(
            expression,
            expected,
            file,
            declarations,
            ordinary,
            &mut probe_states,
            &mut probe_stack,
        )?;
        let hir::ConstPropertyValue::Integer(value) = evaluated.value else {
            return None;
        };
        matches!(probe.types[evaluated.ty], hir::Type::Integer(kind) if kind == value.kind())
            .then_some(value.kind())
    }

    pub(in crate::globals) fn const_integer_intrinsic_argument_kind(
        &self,
        source: hir::IntegerKind,
        source_name: &str,
    ) -> Option<hir::IntegerKind> {
        match self
            .resolve_const_integer_intrinsic(source, source_name)?
            .kind
        {
            ConstIntegerIntrinsicKind::NoGcOperation(operation)
                if operation.arity() == hir::IntegerOperationArity::Binary =>
            {
                Some(
                    if matches!(
                        operation,
                        hir::NoGcIntegerOperation::Shl
                            | hir::NoGcIntegerOperation::Shr
                            | hir::NoGcIntegerOperation::Ushr
                    ) {
                        hir::IntegerKind::SIGNED_64
                    } else {
                        source
                    },
                )
            }
            ConstIntegerIntrinsicKind::ManagedDivRem(_) => Some(source),
            ConstIntegerIntrinsicKind::NoGcOperation(_)
            | ConstIntegerIntrinsicKind::Conversion(_) => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn select_const_integer_literal_receiver_kind(
        &self,
        receiver: &ast::Expr,
        argument: Option<&ast::Expr>,
        source_name: &str,
        expected: Option<hir::TypeId>,
        require_infix: bool,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::IntegerKind> {
        let candidates = crate::expr::integer_literal_candidate_kinds(receiver)?;
        let mut applicable = candidates
            .into_iter()
            .filter(|&kind| {
                let Some(resolved) = self.resolve_const_integer_intrinsic(kind, source_name) else {
                    return false;
                };
                if require_infix && !self.signatures[&resolved.function].modifiers.is_infix {
                    return false;
                }
                match (
                    self.const_integer_intrinsic_argument_kind(kind, source_name),
                    argument,
                ) {
                    (Some(argument_kind), Some(argument)) => {
                        self.probe_const_integer_kind(
                            argument,
                            Some(argument_kind),
                            file,
                            declarations,
                            ordinary,
                            states,
                            stack,
                        ) == Some(argument_kind)
                    }
                    (None, None) => true,
                    (Some(_), None) | (None, Some(_)) => false,
                }
            })
            .collect::<Vec<_>>();
        let expected_kind = expected.and_then(|expected| match self.types[expected] {
            hir::Type::Integer(kind)
                if self.const_integer_receiver_expected(source_name, Some(expected))
                    == Some(expected) =>
            {
                Some(kind)
            }
            _ => None,
        });
        let preferred = expected_kind
            .or_else(|| crate::expr::integer_literal_default_kind(receiver))
            .and_then(|kind| applicable.iter().position(|candidate| *candidate == kind))
            .or_else(|| (applicable.len() == 1).then_some(0))?;
        Some(applicable.swap_remove(preferred))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn select_const_equality_integer_kind(
        &self,
        left: &ast::Expr,
        right: &ast::Expr,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &[ConstState],
        stack: &[usize],
    ) -> Option<hir::IntegerKind> {
        if let Some(kind) = crate::expr::common_integer_literal_kind(&[left, right]) {
            return Some(kind);
        }
        if crate::expr::integer_literal_candidate_kinds(left).is_some() {
            let right_kind = self.probe_const_integer_kind(
                right,
                None,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            return crate::expr::integer_literal_accepts_kind(left, right_kind)
                .then_some(right_kind);
        }
        if crate::expr::integer_literal_candidate_kinds(right).is_some() {
            let left_kind = self.probe_const_integer_kind(
                left,
                None,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            return crate::expr::integer_literal_accepts_kind(right, left_kind)
                .then_some(left_kind);
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_integer_infix(
        &mut self,
        lhs: &ast::Expr,
        target: &ast::InfixTarget,
        rhs: &ast::Expr,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        let ast::InfixTarget::Named(name) = target else {
            self.error(
                span,
                "const initializer cannot invoke an untyped infix callable".to_string(),
            );
            return None;
        };
        let receiver_expected = self
            .select_const_integer_literal_receiver_kind(
                lhs,
                Some(rhs),
                &name.text,
                expected,
                true,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )
            .map(|kind| self.integer_type(kind));
        let left = self.evaluate_const_expression(
            lhs,
            receiver_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let hir::Type::Integer(kind) = self.types[left.ty] else {
            self.error(
                name.span,
                "integer const intrinsic requires an integer receiver".to_string(),
            );
            return None;
        };
        let Some(ResolvedConstIntegerIntrinsic {
            kind: ConstIntegerIntrinsicKind::NoGcOperation(operation),
            function,
        }) = self.resolve_const_integer_intrinsic(kind, &name.text)
        else {
            self.error(
                name.span,
                "const initializer did not resolve to the exact typed core integer intrinsic"
                    .to_string(),
            );
            return None;
        };
        if !self.signatures[&function].modifiers.is_infix
            || operation.arity() != hir::IntegerOperationArity::Binary
        {
            self.error(
                name.span,
                "const infix call did not resolve to a typed core integer infix intrinsic"
                    .to_string(),
            );
            return None;
        }
        let shift = matches!(
            operation,
            hir::NoGcIntegerOperation::Shl
                | hir::NoGcIntegerOperation::Shr
                | hir::NoGcIntegerOperation::Ushr
        );
        let right_expected = if shift {
            self.integer_type(hir::IntegerKind::SIGNED_64)
        } else {
            left.ty
        };
        let right = self.evaluate_const_expression(
            rhs,
            Some(right_expected),
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let (
            hir::ConstPropertyValue::Integer(left_value),
            hir::ConstPropertyValue::Integer(right_value),
        ) = (left.value, right.value)
        else {
            self.error(
                span,
                "integer const intrinsic requires integer operands".to_string(),
            );
            return None;
        };
        if right.ty != right_expected {
            self.error(
                span,
                format!(
                    "integer const intrinsic expected {}, found {}",
                    self.type_name(right_expected),
                    self.type_name(right.ty)
                ),
            );
            return None;
        }
        let value = evaluate_integer_no_gc_operation(operation, left_value, Some(right_value))?;
        Some(EvaluatedConst {
            value,
            ty: self.integer_no_gc_result_type(kind, operation),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_integer_method(
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
                                ast::CallArgumentName::Named(argument_name) => self.signatures
                                    [&resolved.function]
                                    .params
                                    .first()
                                    .is_some_and(|parameter| {
                                        parameter.name.text == argument_name.text
                                    }),
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
                    ast::CallArgumentName::Named(argument_name) => self.signatures
                        [&resolved.function]
                        .params
                        .first()
                        .is_some_and(|parameter| parameter.name.text == argument_name.text),
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
