use super::*;
use crate::{IntegerExpression, IntegerOperation, IntegerOperationArity, IntegerOperator};

impl DecodedDefaultIntegerArgumentsV1 {
    pub(super) fn resolve_operation<R, L, E>(
        self,
        descriptor: IntegerOperation,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultIntegerExpressionV1, DefaultExpressionResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let kind = descriptor.kind();
        match (descriptor.classify(), self) {
            (IntegerOperator::Unary(operation), Self::Unary(operand)) => {
                Ok(IntegerExpression::Unary {
                    kind,
                    operation,
                    operand: resolve_child(operand, resolver, locals, 49, 2)?,
                })
            }
            (IntegerOperator::Binary(operation), Self::Binary { lhs, rhs }) => {
                Ok(IntegerExpression::Binary {
                    kind,
                    operation,
                    lhs: resolve_indexed_child(lhs, resolver, locals, 49, 2, 0)?,
                    rhs: resolve_indexed_child(rhs, resolver, locals, 49, 2, 1)?,
                })
            }
            (IntegerOperator::Unary(_), Self::Binary { .. }) => {
                Err(DefaultExpressionResolutionError::IntegerOperationArity {
                    expected: IntegerOperationArity::Unary,
                    actual: IntegerOperationArity::Binary,
                })
            }
            (IntegerOperator::Binary(_), Self::Unary(_)) => {
                Err(DefaultExpressionResolutionError::IntegerOperationArity {
                    expected: IntegerOperationArity::Binary,
                    actual: IntegerOperationArity::Unary,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests;
