use super::*;

pub(in crate::expr::callable_literals) struct AnonymousSignature {
    pub(in crate::expr::callable_literals) receiver: Option<TypeId>,
    pub(in crate::expr::callable_literals) parameters: Vec<TypeId>,
    pub(in crate::expr::callable_literals) result: Option<TypeId>,
}

impl Lowerer {
    pub(in crate::expr::callable_literals) fn anonymous_signature(
        &mut self,
        source_receiver: Option<&ast::TypeRef>,
        is_suspend: bool,
        source_params: &[ast::Param],
        source_return_ty: Option<&ast::TypeRef>,
        span: Span,
        expected_signature: Option<CallableExpectation>,
    ) -> Option<AnonymousSignature> {
        let receiver_ty = match source_receiver {
            Some(ty) => Some(self.resolve_type_ref(ty)?),
            None => None,
        };
        if let Some(signature) = &expected_signature {
            if signature.is_suspend != is_suspend {
                self.error(
                    span,
                    "ordinary and suspend function types are incompatible".to_string(),
                );
                return None;
            }
            if source_params.len() + usize::from(receiver_ty.is_some()) != signature.arity() {
                self.error(
                    span,
                    format!(
                        "anonymous function has {} parameter(s), but the expected function type has {}",
                        source_params.len() + usize::from(receiver_ty.is_some()),
                        signature.arity()
                    ),
                );
                return None;
            }
        }

        if let (Some(receiver), Some(signature)) = (receiver_ty, &expected_signature) {
            let expected = *signature
                .parameters()
                .next()
                .expect("receiver arity was checked");
            if !self.is_subtype(expected, receiver) {
                self.error(
                    source_receiver.expect("explicit receiver").span,
                    format!(
                        "anonymous-function receiver type is {}, but the expected type is {}",
                        self.type_name(receiver),
                        self.type_name(expected)
                    ),
                );
                return None;
            }
        }
        let expected_parameters = expected_signature.as_ref().map(|signature| {
            signature
                .parameters()
                .copied()
                .skip(usize::from(receiver_ty.is_some()))
                .collect::<Vec<_>>()
        });
        let mut parameter_types = Vec::with_capacity(source_params.len());
        for (index, parameter) in source_params.iter().enumerate() {
            let ty = self.resolve_type_ref(&parameter.ty)?;
            if let Some(parameters) = &expected_parameters {
                let expected = parameters[index];
                if !self.is_subtype(expected, ty) {
                    let found = self.type_name(ty);
                    let expected = self.type_name(expected);
                    self.error(
                        parameter.ty.span,
                        format!(
                            "anonymous-function parameter type is {found}, but the expected type is {expected}"
                        ),
                    );
                    return None;
                }
            }
            parameter_types.push(ty);
        }
        let explicit_return = match source_return_ty {
            Some(return_ty) => Some(self.resolve_type_ref(return_ty)?),
            None => None,
        };
        let expected_return = expected_signature
            .as_ref()
            .and_then(|signature| signature.return_type);
        if let (Some(explicit), Some(expected)) = (explicit_return, expected_return)
            && !self.is_subtype(explicit, expected)
        {
            let found = self.type_name(explicit);
            let expected = self.type_name(expected);
            self.error(
                source_return_ty.expect("explicit return type").span,
                format!(
                    "anonymous-function return type is {found}, but the expected type is {expected}"
                ),
            );
            return None;
        }
        Some(AnonymousSignature {
            receiver: receiver_ty,
            parameters: parameter_types,
            result: explicit_return.or(expected_return),
        })
    }
}
