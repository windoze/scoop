use super::*;

impl<A, E> Validator<'_, A, E>
where
    A: DefaultBodyOperationAuthority<E>,
{
    pub(super) fn expect_source_receiver(
        &mut self,
        operation: DefaultExpressionOperationV1,
        receiver: &crate::SourceCallReceiver<SignatureTypeKey>,
        shape: &DefaultCallableOperationShapeV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        use crate::SourceCallReceiver;

        let role = DefaultOperationValueRoleV1::Receiver;
        match (receiver, shape.receiver()) {
            (SourceCallReceiver::NoReceiver, None) => Ok(()),
            (SourceCallReceiver::NoReceiver, Some(_)) => self.problem(
                operation,
                role,
                DefaultOperationTypingProblemV1::MissingCallableReceiver,
            ),
            (SourceCallReceiver::Receiver { .. }, None) => self.problem(
                operation,
                role,
                DefaultOperationTypingProblemV1::UnexpectedCallableReceiver,
            ),
            (SourceCallReceiver::Receiver { static_type }, Some(expected)) => {
                if static_type == expected {
                    Ok(())
                } else {
                    self.expect_relation(
                        DefaultOperationTypeRelationV1::MemberReceiver,
                        static_type,
                        expected,
                        Self::site(operation, role),
                    )
                }
            }
        }
    }

    pub(super) fn expect_direct_call_arguments(
        &mut self,
        operation: DefaultExpressionOperationV1,
        actual: &[crate::DefaultExpressionV1],
        shape: &DefaultCallableOperationShapeV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let receiver_count = usize::from(shape.receiver().is_some());
        self.expect_arity(
            actual.len(),
            receiver_count + shape.parameters().len(),
            Self::site(operation, DefaultOperationValueRoleV1::Callable),
        )?;
        if let Some(receiver) = shape.receiver() {
            self.expect_type(
                actual[0].result_type(),
                receiver,
                Self::site(operation, DefaultOperationValueRoleV1::Receiver),
            )?;
        }
        for (index, (actual, expected)) in actual[receiver_count..]
            .iter()
            .zip(shape.parameters())
            .enumerate()
        {
            self.expect_type(
                actual.result_type(),
                expected,
                Self::site(operation, DefaultOperationValueRoleV1::Argument { index }),
            )?;
        }
        Ok(())
    }
}
