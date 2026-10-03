use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_default_operator(
        &mut self,
        kind: &hir::DefaultExpressionKindV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        use hir::DefaultExpressionKindV1 as Kind;
        Ok(match kind {
            Kind::PrimitiveBinary { kind, lhs, rhs } => hir::ExprKind::PrimitiveBinary {
                kind: (*kind).into(),
                lhs: Box::new(self.materialize_imported_default_expression(lhs, context)?),
                rhs: Box::new(self.materialize_imported_default_expression(rhs, context)?),
            },
            Kind::PrimitiveUnary { kind, operand } => hir::ExprKind::PrimitiveUnary {
                kind: (*kind).into(),
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::IntegerOperation {
                operation,
                arguments,
            } => {
                if matches!(operation, hir::DefaultIntegerOperationV1::Managed { .. }) {
                    self.prepare_arithmetic_exception_type().map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(
                            error.diagnostic("integer division exception type"),
                        )
                    })?;
                }
                hir::ExprKind::IntegerOperation {
                    operation: (*operation).into(),
                    arguments: match arguments {
                        hir::DefaultIntegerArgumentsV1::Unary(operand) => {
                            hir::HirIntegerOperationArguments::Unary(Box::new(
                                self.materialize_imported_default_expression(operand, context)?,
                            ))
                        }
                        hir::DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                            hir::HirIntegerOperationArguments::Binary {
                                lhs: Box::new(
                                    self.materialize_imported_default_expression(lhs, context)?,
                                ),
                                rhs: Box::new(
                                    self.materialize_imported_default_expression(rhs, context)?,
                                ),
                            }
                        }
                    },
                }
            }
            Kind::IntegerConversion {
                source_kind,
                target_kind,
                operand,
            } => hir::ExprKind::IntegerConversion {
                conversion: hir::IntegerConversion {
                    source: (*source_kind).into(),
                    target_kind: (*target_kind).into(),
                },
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::Binary { operator, lhs, rhs } => hir::ExprKind::Binary {
                op: (*operator).into(),
                lhs: Box::new(self.materialize_imported_default_expression(lhs, context)?),
                rhs: Box::new(self.materialize_imported_default_expression(rhs, context)?),
            },
            Kind::Unary { operator, operand } => hir::ExprKind::Unary {
                op: (*operator).into(),
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            _ => unreachable!("the default dispatcher selects only operator expressions"),
        })
    }
}
