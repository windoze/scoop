use super::*;

impl Lowerer {
    pub(super) fn probe_static_integer_kind(
        &self,
        expression: &ast::Expr,
        expected: Option<hir::IntegerKind>,
    ) -> Option<hir::IntegerKind> {
        let mut probe = self.clone();
        let expected = expected.map(|kind| probe.integer_type(kind));
        let evaluated = probe.evaluate_static_value(expression, expected)?;
        let hir::ConstPropertyValue::Integer(value) = evaluated.value else {
            return None;
        };
        matches!(probe.types[evaluated.ty], hir::Type::Integer(kind) if kind == value.kind())
            .then_some(value.kind())
    }

    pub(super) fn select_static_integer_literal_receiver_kind(
        &self,
        receiver: &ast::Expr,
        argument: Option<&ast::Expr>,
        source_name: &str,
        expected: Option<hir::TypeId>,
        require_infix: bool,
    ) -> Option<hir::IntegerKind> {
        let candidates = crate::expr::integer_literal_candidate_kinds(receiver)?;
        let mut applicable = candidates
            .into_iter()
            .filter(|&kind| {
                let Some(resolved) = self.resolve_const_integer_intrinsic(kind, source_name) else {
                    return false;
                };
                if require_infix && !resolved.is_infix {
                    return false;
                }
                match (
                    self.const_integer_intrinsic_argument_kind(kind, source_name),
                    argument,
                ) {
                    (Some(argument_kind), Some(argument)) => {
                        self.probe_static_integer_kind(argument, Some(argument_kind))
                            == Some(argument_kind)
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

    pub(super) fn select_static_equality_integer_kind(
        &self,
        left: &ast::Expr,
        right: &ast::Expr,
    ) -> Option<hir::IntegerKind> {
        if let Some(kind) = crate::expr::common_integer_literal_kind(&[left, right]) {
            return Some(kind);
        }
        if crate::expr::integer_literal_candidate_kinds(left).is_some() {
            let right_kind = self.probe_static_integer_kind(right, None)?;
            return crate::expr::integer_literal_accepts_kind(left, right_kind)
                .then_some(right_kind);
        }
        if crate::expr::integer_literal_candidate_kinds(right).is_some() {
            let left_kind = self.probe_static_integer_kind(left, None)?;
            return crate::expr::integer_literal_accepts_kind(right, left_kind)
                .then_some(left_kind);
        }
        None
    }

    pub(super) fn evaluate_static_integer_infix(
        &mut self,
        lhs: &ast::Expr,
        target: &ast::InfixTarget,
        rhs: &ast::Expr,
        expected: Option<hir::TypeId>,
    ) -> Option<StaticValue> {
        let ast::InfixTarget::Named(name) = target else {
            return None;
        };
        let receiver_expected = self
            .select_static_integer_literal_receiver_kind(lhs, Some(rhs), &name.text, expected, true)
            .map(|kind| self.integer_type(kind));
        let left = self.evaluate_static_value(lhs, receiver_expected)?;
        let hir::Type::Integer(kind) = self.types[left.ty] else {
            return None;
        };
        let resolved = self.resolve_const_integer_intrinsic(kind, &name.text)?;
        let ConstIntegerIntrinsicKind::NoGcOperation(operation) = resolved.kind else {
            return None;
        };
        if !resolved.is_infix || operation.arity() != hir::IntegerOperationArity::Binary {
            return None;
        }
        let right_expected = if matches!(
            operation,
            hir::NoGcIntegerOperation::Shl
                | hir::NoGcIntegerOperation::Shr
                | hir::NoGcIntegerOperation::Ushr
        ) {
            self.integer_type(hir::IntegerKind::SIGNED_64)
        } else {
            left.ty
        };
        let right = self.evaluate_static_value(rhs, Some(right_expected))?;
        if right.ty != right_expected {
            return None;
        }
        let (
            hir::ConstPropertyValue::Integer(left_value),
            hir::ConstPropertyValue::Integer(right_value),
        ) = (left.value, right.value)
        else {
            return None;
        };
        let value = evaluate_integer_no_gc_operation(operation, left_value, Some(right_value))?;
        Some(StaticValue {
            value,
            ty: self.integer_no_gc_result_type(kind, operation),
        })
    }

    pub(super) fn evaluate_static_integer_method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        navigation: ast::Navigation,
        type_args: &[ast::CallTypeArgument],
        args: &[ast::CallArgument],
        expected: Option<hir::TypeId>,
    ) -> Option<StaticValue> {
        if navigation != ast::Navigation::Direct || !type_args.is_empty() {
            return None;
        }
        let receiver_kind = match args {
            [] => self.select_static_integer_literal_receiver_kind(
                receiver, None, &name.text, expected, false,
            ),
            [argument] if matches!(argument.spread, ast::SpreadSyntax::Plain) => self
                .select_static_integer_literal_receiver_kind(
                    receiver,
                    Some(&argument.expression),
                    &name.text,
                    expected,
                    false,
                ),
            _ => None,
        };
        let receiver_expected = receiver_kind.map(|kind| self.integer_type(kind));
        let receiver = self.evaluate_static_value(receiver, receiver_expected)?;
        if let Some(resolved) = self.resolve_const_float_intrinsic(receiver.ty, &name.text) {
            return self.evaluate_static_float_call(resolved, receiver, args);
        }
        let hir::Type::Integer(source_kind) = self.types[receiver.ty] else {
            return None;
        };
        let hir::ConstPropertyValue::Integer(receiver_value) = receiver.value else {
            return None;
        };
        let resolved = self.resolve_const_integer_intrinsic(source_kind, &name.text)?;
        match resolved.kind {
            ConstIntegerIntrinsicKind::NoGcOperation(operation) => {
                let right = match (operation.arity(), args) {
                    (hir::IntegerOperationArity::Unary, []) => None,
                    (hir::IntegerOperationArity::Binary, [argument])
                        if matches!(argument.spread, ast::SpreadSyntax::Plain)
                            && match &argument.name {
                                ast::CallArgumentName::Positional => true,
                                ast::CallArgumentName::TrailingLambda => false,
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
                        let value =
                            self.evaluate_static_value(&argument.expression, Some(expected))?;
                        if !self.types_equal(value.ty, expected) {
                            return None;
                        }
                        let hir::ConstPropertyValue::Integer(value) = value.value else {
                            return None;
                        };
                        Some(value)
                    }
                    _ => return None,
                };
                Some(StaticValue {
                    value: evaluate_integer_no_gc_operation(operation, receiver_value, right)?,
                    ty: self.integer_no_gc_result_type(source_kind, operation),
                })
            }
            ConstIntegerIntrinsicKind::ManagedDivRem(operation) => {
                let [argument] = args else {
                    return None;
                };
                let named_argument_matches = match &argument.name {
                    ast::CallArgumentName::Positional => true,
                    ast::CallArgumentName::TrailingLambda => false,
                    ast::CallArgumentName::Named(argument_name) => resolved
                        .parameters
                        .first()
                        .is_some_and(|parameter| *parameter == argument_name.text),
                };
                if !matches!(argument.spread, ast::SpreadSyntax::Plain) || !named_argument_matches {
                    return None;
                }
                let right = self.evaluate_static_value(&argument.expression, Some(receiver.ty))?;
                if !self.types_equal(right.ty, receiver.ty) {
                    return None;
                }
                let hir::ConstPropertyValue::Integer(right_value) = right.value else {
                    return None;
                };
                let operator = match operation {
                    hir::IntegerDivRem::Div => ast::BinOp::Div,
                    hir::IntegerDivRem::Rem => ast::BinOp::Rem,
                };
                let IntegerBinaryResult::Value(value) =
                    evaluate_integer_binary(operator, receiver_value, right_value)
                else {
                    return None;
                };
                Some(StaticValue {
                    value,
                    ty: receiver.ty,
                })
            }
            ConstIntegerIntrinsicKind::Conversion(target_kind) if args.is_empty() => {
                Some(StaticValue {
                    value: hir::ConstPropertyValue::Integer(convert_integer_constant(
                        receiver_value,
                        target_kind,
                    )),
                    ty: self.integer_type(target_kind),
                })
            }
            ConstIntegerIntrinsicKind::Conversion(_) => None,
        }
    }
}
