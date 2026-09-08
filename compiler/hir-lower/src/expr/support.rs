use super::*;
use crate::{IntrinsicTypeOwner, Owner};

/// The else half of a `?.` / `?:` desugaring: the statements evaluating
/// the fallback (lazily, inside the branch), then the fallback value.
pub(super) struct ElseBranch {
    pub(super) statements: Vec<hir::Statement>,
    pub(super) value: hir::Expr,
}

impl Lowerer {
    pub(crate) fn lower_integer_literal(
        &mut self,
        literal: ast::IntegerLiteralSyntax,
        expected: Option<TypeId>,
        negative: bool,
        span: Span,
    ) -> Option<hir::Expr> {
        let expected_kind = expected.and_then(|ty| match self.types[ty] {
            Type::Integer(kind) => Some(kind),
            _ => None,
        });
        let kind = if let Some(kind) = expected_kind {
            if literal_accepts_kind(literal, kind, negative) {
                kind
            } else {
                self.error(
                    span,
                    format!(
                        "integer literal `{}` is not representable as {}",
                        display_integer_literal(literal, negative),
                        kind.canonical_name()
                    ),
                );
                return None;
            }
        } else {
            let kind = default_integer_kind(literal, negative);
            let Some(kind) = kind else {
                self.error(
                    span,
                    format!(
                        "integer literal `{}` is outside its suffix domain",
                        display_integer_literal(literal, negative)
                    ),
                );
                return None;
            };
            kind
        };
        let value = hir::HirIntegerConstant::from_magnitude(kind, literal.magnitude, negative)
            .expect("literal kind selection proves representability");
        Some(hir::Expr {
            kind: ExprKind::IntegerLiteral(value),
            ty: self.integer_type(kind),
            span,
            origin: self.expression_origin(span),
        })
    }
}

pub(crate) fn literal_accepts_kind(
    literal: ast::IntegerLiteralSyntax,
    kind: hir::IntegerKind,
    negative: bool,
) -> bool {
    use ast::IntegerSuffix;
    use hir::IntegerSignedness;

    let domain_matches = match literal.suffix {
        IntegerSuffix::None => kind.signedness() == IntegerSignedness::Signed,
        IntegerSuffix::Unsigned => !negative && kind.signedness() == IntegerSignedness::Unsigned,
        IntegerSuffix::Long => kind == hir::IntegerKind::SIGNED_64,
        IntegerSuffix::UnsignedLong => !negative && kind == hir::IntegerKind::UNSIGNED_64,
    };
    domain_matches && kind.fits_magnitude(literal.magnitude, negative)
}

pub(crate) fn default_integer_kind(
    literal: ast::IntegerLiteralSyntax,
    negative: bool,
) -> Option<hir::IntegerKind> {
    use ast::IntegerSuffix;
    use hir::IntegerKind;

    match literal.suffix {
        IntegerSuffix::None => [IntegerKind::SIGNED_32, IntegerKind::SIGNED_64]
            .into_iter()
            .find(|kind| kind.fits_magnitude(literal.magnitude, negative)),
        IntegerSuffix::Unsigned if !negative => {
            [IntegerKind::UNSIGNED_32, IntegerKind::UNSIGNED_64]
                .into_iter()
                .find(|kind| kind.fits_positive_magnitude(literal.magnitude))
        }
        IntegerSuffix::Long => IntegerKind::SIGNED_64
            .fits_magnitude(literal.magnitude, negative)
            .then_some(IntegerKind::SIGNED_64),
        IntegerSuffix::UnsignedLong if !negative => IntegerKind::UNSIGNED_64
            .fits_positive_magnitude(literal.magnitude)
            .then_some(IntegerKind::UNSIGNED_64),
        IntegerSuffix::Unsigned | IntegerSuffix::UnsignedLong => None,
    }
}

pub(crate) fn integer_literal_default_kind(expr: &ast::Expr) -> Option<hir::IntegerKind> {
    match expr {
        ast::Expr::IntLiteral(literal) => default_integer_kind(*literal, false),
        ast::Expr::Unary { op, operand, .. } if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg) => {
            match &**operand {
                ast::Expr::IntLiteral(literal) => {
                    let negative = *op == ast::UnOp::Neg
                        && matches!(
                            literal.suffix,
                            ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                        );
                    default_integer_kind(*literal, negative)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Every exact integer representation to which a direct integer-literal
/// expression may be committed. Unary `+` remains a representation
/// intrinsic, while signed unary `-` participates in the literal boundary;
/// unsigned unary `-` is a wrapping intrinsic over a positive literal.
pub(crate) fn integer_literal_candidate_kinds(expr: &ast::Expr) -> Option<Vec<hir::IntegerKind>> {
    let (literal, negative) = match expr {
        ast::Expr::IntLiteral(literal) => (*literal, false),
        ast::Expr::Unary { op, operand, .. } if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg) => {
            let ast::Expr::IntLiteral(literal) = &**operand else {
                return None;
            };
            let negative = *op == ast::UnOp::Neg
                && matches!(
                    literal.suffix,
                    ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                );
            (*literal, negative)
        }
        _ => return None,
    };
    Some(
        hir::IntegerKind::ALL
            .into_iter()
            .filter(|&kind| literal_accepts_kind(literal, kind, negative))
            .collect(),
    )
}

pub(crate) fn integer_literal_accepts_kind(expr: &ast::Expr, expected: hir::IntegerKind) -> bool {
    integer_literal_candidate_kinds(expr).is_some_and(|kinds| kinds.contains(&expected))
}

/// Select one exact representation accepted by every direct literal in the
/// slice. The fixed-point default is the strongest per-literal default that
/// remains in the intersection, so a wide literal constrains its smaller
/// peers independently of source order.
pub(crate) fn common_integer_literal_kind(expressions: &[&ast::Expr]) -> Option<hir::IntegerKind> {
    let candidates = expressions
        .iter()
        .map(|expr| integer_literal_candidate_kinds(expr))
        .collect::<Option<Vec<_>>>()?;
    let common = hir::IntegerKind::ALL
        .into_iter()
        .filter(|kind| candidates.iter().all(|set| set.contains(kind)))
        .collect::<Vec<_>>();
    let preferred = expressions
        .iter()
        .filter_map(|expr| integer_literal_default_kind(expr))
        .max_by_key(|kind| (kind.width().bits(), kind.signedness()));
    preferred
        .filter(|kind| common.contains(kind))
        .or_else(|| (common.len() == 1).then(|| common[0]))
}

impl Lowerer {
    /// Stable strength of the integer defaults nested in a contextual seed.
    /// Callers use this only after `expr_can_provide_default_seed` has proved
    /// the whole expression can synthesize a complete type. Choosing the
    /// strongest seed removes source-order dependence without committing any
    /// probe state.
    pub(crate) fn expr_default_seed_rank(
        &self,
        expr: &ast::Expr,
    ) -> Option<(u32, hir::IntegerSignedness)> {
        fn nested_rank(expr: &ast::Expr) -> Option<(u32, hir::IntegerSignedness)> {
            if let Some(kind) = integer_literal_default_kind(expr) {
                return Some((kind.width().bits(), kind.signedness()));
            }
            let children: Vec<&ast::Expr> = match expr {
                ast::Expr::TupleLiteral { elements, .. }
                | ast::Expr::ArrayLiteral { elements, .. } => elements.iter().collect(),
                ast::Expr::Call(call) => call.args.iter().map(|arg| &arg.expression).collect(),
                ast::Expr::StructInit { args, .. } | ast::Expr::MethodCall { args, .. } => {
                    args.iter().map(|arg| &arg.expression).collect()
                }
                ast::Expr::CopyUpdate { base, .. } => vec![base],
                _ => Vec::new(),
            };
            children.into_iter().filter_map(nested_rank).max()
        }

        self.expr_can_provide_default_seed(expr)
            .then(|| nested_rank(expr))
            .flatten()
    }

    /// Probe the closed integer receiver family transactionally. A probe is
    /// eligible only when its final typed HIR proves an integer
    /// representation intrinsic or one of the four ordinary core range
    /// members. User and extension methods with the same spelling therefore
    /// cannot obtain reverse literal inference.
    pub(in crate::expr) fn probe_integer_literal_receiver(
        &self,
        receiver: &ast::Expr,
        expected: Option<TypeId>,
        mut lower: impl FnMut(&mut Lowerer, hir::Expr, &mut Vec<hir::Statement>) -> Option<hir::Expr>,
    ) -> Option<SuccessfulExprLayer> {
        let kinds = integer_literal_candidate_kinds(receiver)?;
        let default = integer_literal_default_kind(receiver);
        let expected_kind = expected.and_then(|ty| match self.types[ty] {
            Type::Integer(kind) => Some(kind),
            _ => None,
        });
        let mut successful = Vec::new();
        for kind in kinds {
            let Ok(layer) = self.probe_expr_layer(|state, layer_sink| {
                let receiver_ty = state.integer_type(kind);
                let receiver = state.lower_expr(receiver, layer_sink, Some(receiver_ty))?;
                lower(state, receiver, layer_sink)
            }) else {
                continue;
            };
            if layer
                .state
                .integer_receiver_role(&layer.expression, kind)
                .is_none()
            {
                continue;
            }
            successful.push((kind, layer));
        }
        if successful.is_empty() {
            return None;
        }
        let preferred_index = expected_kind
            .and_then(|kind| {
                successful.iter().position(|(candidate, layer)| {
                    *candidate == kind
                        && integer_operation_preserves_receiver_kind(&layer.expression)
                })
            })
            .or_else(|| {
                default.and_then(|kind| {
                    successful
                        .iter()
                        .position(|(candidate, _)| *candidate == kind)
                })
            })
            .or_else(|| (successful.len() == 1).then_some(0))?;
        Some(successful.swap_remove(preferred_index).1)
    }

    fn integer_receiver_role(
        &self,
        expression: &hir::Expr,
        receiver_kind: hir::IntegerKind,
    ) -> Option<()> {
        match &expression.kind {
            ExprKind::IntegerOperation { operation, .. } if operation.kind() == receiver_kind => {
                Some(())
            }
            ExprKind::IntegerConversion { conversion, .. }
                if conversion.source == receiver_kind =>
            {
                Some(())
            }
            ExprKind::MethodCall { callee, .. } => {
                let hir::MethodCallee::Callable(callable) = *callee else {
                    return None;
                };
                let function = self.callable_function_id(callable);
                self.is_core_integer_range_member(function, receiver_kind)
                    .then_some(())
            }
            _ => None,
        }
    }

    fn is_core_integer_range_member(
        &self,
        function: hir::FunctionId,
        receiver_kind: hir::IntegerKind,
    ) -> bool {
        if self
            .function_files
            .get(&function)
            .copied()
            .is_none_or(|file| !self.source_is_core(file))
        {
            return false;
        }
        let Some(&(owner, _)) = self
            .intrinsic_type_owners
            .get(&hir::IntrinsicTypeKind::Integer(receiver_kind))
        else {
            return false;
        };
        let owner_matches = match (self.function_owner.get(&function), owner) {
            (Some(Owner::Struct(actual)), IntrinsicTypeOwner::Struct(expected)) => {
                *actual == expected
            }
            (Some(Owner::Class(actual)), IntrinsicTypeOwner::Class(expected)) => {
                *actual == expected
            }
            _ => false,
        };
        if !owner_matches || !matches!(self.functions[function].kind, hir::FunctionKind::User(_)) {
            return false;
        }
        let signature = &self.signatures[&function];
        let owner_ty = self.integer_type(receiver_kind);
        let exact_peer =
            matches!(signature.params.as_slice(), [parameter] if parameter.ty == owner_ty);
        let source_name = self.functions[function]
            .name
            .rsplit('.')
            .next()
            .expect("function names are non-empty");
        let role_matches = match source_name {
            "rangeTo" => {
                signature.modifiers.operator == Some(hir::OperatorKind::RangeTo)
                    && !signature.modifiers.is_infix
            }
            "rangeUntil" => {
                signature.modifiers.operator == Some(hir::OperatorKind::RangeUntil)
                    && !signature.modifiers.is_infix
            }
            "until" | "downTo" => {
                signature.modifiers.operator.is_none() && signature.modifiers.is_infix
            }
            _ => false,
        };
        exact_peer
            && role_matches
            && signature.type_params.is_empty()
            && signature.owner_type_param_count == 0
    }
}

fn integer_operation_preserves_receiver_kind(expression: &hir::Expr) -> bool {
    let ExprKind::IntegerOperation { operation, .. } = &expression.kind else {
        return false;
    };
    match operation {
        hir::IntegerOperation::Managed { .. } => true,
        hir::IntegerOperation::NoGc { operation, .. } => !matches!(
            operation,
            hir::NoGcIntegerOperation::CompareTo | hir::NoGcIntegerOperation::Equals
        ),
    }
}

fn display_integer_literal(literal: ast::IntegerLiteralSyntax, negative: bool) -> String {
    if negative {
        format!("-{literal}")
    } else {
        literal.to_string()
    }
}
