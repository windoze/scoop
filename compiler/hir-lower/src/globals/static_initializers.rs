use scoop_ast as ast;
use scoop_hir as hir;

use super::consts::{
    ConstIntegerIntrinsicKind, IntegerBinaryResult, convert_integer_constant,
    evaluate_integer_binary, evaluate_integer_no_gc_operation,
};
use crate::Lowerer;

struct StaticValue {
    value: hir::ConstPropertyValue,
    ty: hir::TypeId,
}

impl Lowerer {
    pub(super) fn static_property_constant(
        &mut self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        let mut probe = self.clone();
        let value = probe.evaluate_static_property_constant(expression, expected);
        if value.is_some() {
            self.dependencies = probe.dependencies;
        }
        value
    }

    fn evaluate_static_property_constant(
        &mut self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        if let Some(variant) = self.static_unit_variant_constant(expression, expected) {
            return Some(variant);
        }
        let evaluated = self.evaluate_static_value(expression, Some(expected))?;
        if !self.types_equal(evaluated.ty, expected) {
            return None;
        }
        Some(match evaluated.value {
            hir::ConstPropertyValue::Integer(value) => hir::HirConstantImage::Integer(value),
            hir::ConstPropertyValue::Boolean(value) => hir::HirConstantImage::Boolean(value),
            hir::ConstPropertyValue::String(value) => hir::HirConstantImage::String(value),
        })
    }

    fn evaluate_static_value(
        &mut self,
        expression: &ast::Expr,
        expected: Option<hir::TypeId>,
    ) -> Option<StaticValue> {
        match expression {
            ast::Expr::IntLiteral(literal) => {
                let expression =
                    self.lower_integer_literal(*literal, expected, false, literal.span)?;
                let hir::ExprKind::IntegerLiteral(value) = expression.kind else {
                    unreachable!("literal lowering produces an integer constant")
                };
                Some(StaticValue {
                    value: hir::ConstPropertyValue::Integer(value),
                    ty: expression.ty,
                })
            }
            ast::Expr::BoolLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::Boolean(*value),
                ty: self.boolean,
            }),
            ast::Expr::StringLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::String(value.clone()),
                ty: self.string,
            }),
            ast::Expr::Var(name) => {
                let crate::imports::lookup::LookupResult::Unique(origin) =
                    self.lookup_value_origin(&name.text)
                else {
                    return None;
                };
                if let crate::imports::lookup::values::ValueOrigin::Dependency(binding) = origin {
                    let imported = self.select_imported_dependency_constant(&binding, name.span)?;
                    return Some(StaticValue {
                        value: imported.value,
                        ty: imported.ty,
                    });
                }
                let crate::imports::lookup::values::ValueTarget::Property(property) =
                    self.materialized_value_target(&origin)?
                else {
                    return None;
                };
                let declaration = self.properties[property].clone();
                let hir::PropertyRepresentation::Const { value } = declaration.representation
                else {
                    return None;
                };
                Some(StaticValue {
                    value,
                    ty: declaration.ty,
                })
            }
            ast::Expr::Unary { op, operand, .. } => {
                if *op == ast::UnOp::Neg
                    && let ast::Expr::IntLiteral(literal) = &**operand
                    && matches!(
                        literal.suffix,
                        ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                    )
                {
                    let expression =
                        self.lower_integer_literal(*literal, expected, true, expression.span())?;
                    let hir::ExprKind::IntegerLiteral(value) = expression.kind else {
                        unreachable!("literal lowering produces an integer constant")
                    };
                    return Some(StaticValue {
                        value: hir::ConstPropertyValue::Integer(value),
                        ty: expression.ty,
                    });
                }
                let operand = self.evaluate_static_value(operand, expected)?;
                match (*op, operand.value) {
                    (ast::UnOp::Plus | ast::UnOp::Neg, hir::ConstPropertyValue::Integer(value)) => {
                        let hir::Type::Integer(kind) = self.types[operand.ty] else {
                            return None;
                        };
                        let operation = match op {
                            ast::UnOp::Plus => hir::NoGcIntegerOperation::UnaryPlus,
                            ast::UnOp::Neg => hir::NoGcIntegerOperation::UnaryMinus,
                            ast::UnOp::Not => {
                                unreachable!("the outer match selected an integer unary operator")
                            }
                        };
                        self.registered_no_gc_integer_operation(kind, operation)?;
                        Some(StaticValue {
                            value: evaluate_integer_no_gc_operation(operation, value, None)?,
                            ty: self.integer_no_gc_result_type(kind, operation),
                        })
                    }
                    (ast::UnOp::Not, hir::ConstPropertyValue::Boolean(value)) => {
                        Some(StaticValue {
                            value: hir::ConstPropertyValue::Boolean(!value),
                            ty: self.boolean,
                        })
                    }
                    _ => None,
                }
            }
            ast::Expr::Binary { op, lhs, rhs, .. } => {
                let equality = matches!(op, ast::BinOp::Eq | ast::BinOp::Ne);
                let source_name = match op {
                    ast::BinOp::Add => Some("plus"),
                    ast::BinOp::Sub => Some("minus"),
                    ast::BinOp::Mul => Some("times"),
                    ast::BinOp::Div => Some("div"),
                    ast::BinOp::Rem => Some("rem"),
                    ast::BinOp::Lt | ast::BinOp::Le | ast::BinOp::Gt | ast::BinOp::Ge => {
                        Some("compareTo")
                    }
                    _ => None,
                };
                let operand_kind = if equality {
                    self.select_static_equality_integer_kind(lhs, rhs)
                } else {
                    source_name.and_then(|source_name| {
                        self.select_static_integer_literal_receiver_kind(
                            lhs,
                            Some(rhs),
                            source_name,
                            expected,
                            false,
                        )
                    })
                };
                let operand_expected = operand_kind.map(|kind| self.integer_type(kind));
                let lhs = self.evaluate_static_value(lhs, operand_expected)?;
                let rhs_expected = if equality {
                    operand_expected
                } else {
                    Some(lhs.ty)
                };
                let rhs = self.evaluate_static_value(rhs, rhs_expected)?;
                if !self.types_equal(lhs.ty, rhs.ty) {
                    return None;
                }
                let value = match (lhs.value, rhs.value) {
                    (
                        hir::ConstPropertyValue::Integer(left),
                        hir::ConstPropertyValue::Integer(right),
                    ) => match self.evaluate_typed_integer_binary_operator(*op, left, right) {
                        IntegerBinaryResult::Value(value) => Some(value),
                        IntegerBinaryResult::DivisionByZero | IntegerBinaryResult::Unsupported => {
                            None
                        }
                    },
                    (
                        hir::ConstPropertyValue::Boolean(left),
                        hir::ConstPropertyValue::Boolean(right),
                    ) => match op {
                        ast::BinOp::And => Some(hir::ConstPropertyValue::Boolean(left && right)),
                        ast::BinOp::Or => Some(hir::ConstPropertyValue::Boolean(left || right)),
                        ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                        ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                        _ => None,
                    },
                    (
                        hir::ConstPropertyValue::String(left),
                        hir::ConstPropertyValue::String(right),
                    ) => match op {
                        ast::BinOp::Add => Some(hir::ConstPropertyValue::String(left + &right)),
                        ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                        ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                        ast::BinOp::Lt => Some(hir::ConstPropertyValue::Boolean(left < right)),
                        ast::BinOp::Le => Some(hir::ConstPropertyValue::Boolean(left <= right)),
                        ast::BinOp::Gt => Some(hir::ConstPropertyValue::Boolean(left > right)),
                        ast::BinOp::Ge => Some(hir::ConstPropertyValue::Boolean(left >= right)),
                        _ => None,
                    },
                    _ => None,
                }?;
                let ty = match value {
                    hir::ConstPropertyValue::Boolean(_) => self.boolean,
                    hir::ConstPropertyValue::String(_) => self.string,
                    hir::ConstPropertyValue::Integer(_) => lhs.ty,
                };
                Some(StaticValue { value, ty })
            }
            ast::Expr::InfixCall {
                lhs, target, rhs, ..
            } => self.evaluate_static_integer_infix(lhs, target, rhs, expected),
            ast::Expr::MethodCall {
                receiver,
                name,
                navigation,
                type_args,
                args,
                ..
            } => self.evaluate_static_integer_method(
                receiver,
                name,
                *navigation,
                type_args,
                args,
                expected,
            ),
            _ => None,
        }
    }

    fn probe_static_integer_kind(
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

    fn select_static_integer_literal_receiver_kind(
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
                if require_infix && !self.signatures[&resolved.function].modifiers.is_infix {
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

    fn select_static_equality_integer_kind(
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

    fn evaluate_static_integer_infix(
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
        if !self.signatures[&resolved.function].modifiers.is_infix
            || operation.arity() != hir::IntegerOperationArity::Binary
        {
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

    fn evaluate_static_integer_method(
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
                    ast::CallArgumentName::Named(argument_name) => self.signatures
                        [&resolved.function]
                        .params
                        .first()
                        .is_some_and(|parameter| parameter.name.text == argument_name.text),
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
