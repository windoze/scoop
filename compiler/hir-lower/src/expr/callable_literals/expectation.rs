use super::*;

pub(crate) enum ExpressionExpectation {
    Type(TypeId),
    Callable(CallableExpectation),
}

/// Candidate-local input to callable inference, never an incomplete HIR type.
pub(crate) struct CallableExpectation {
    pub(super) is_suspend: bool,
    pub(super) receiver: Option<TypeId>,
    pub(super) parameter_types: Vec<TypeId>,
    pub(super) return_type: Option<TypeId>,
}

impl CallableExpectation {
    pub(super) fn arity(&self) -> usize {
        usize::from(self.receiver.is_some()) + self.parameter_types.len()
    }

    pub(super) fn parameters(&self) -> impl Iterator<Item = &TypeId> {
        self.receiver.iter().chain(&self.parameter_types)
    }
}

impl Lowerer {
    pub(crate) fn callable_argument_expectation(
        &mut self,
        ty: TypeId,
        bindings: &[Option<TypeId>],
    ) -> Option<ExpressionExpectation> {
        self.try_substitute(ty, bindings)
            .map(ExpressionExpectation::Type)
            .or_else(|| {
                self.partial_callable_expectation(ty, bindings)
                    .map(ExpressionExpectation::Callable)
            })
    }

    pub(in crate::expr) fn callable_expectation(&self, ty: TypeId) -> Option<CallableExpectation> {
        let Type::Function(id) = self.types[ty] else {
            return None;
        };
        let signature = &self.function_types[id];
        Some(CallableExpectation {
            is_suspend: signature.is_suspend,
            receiver: signature.receiver,
            parameter_types: signature.parameter_types.clone(),
            return_type: Some(signature.return_type),
        })
    }

    pub(crate) fn partial_callable_expectation(
        &mut self,
        ty: TypeId,
        bindings: &[Option<TypeId>],
    ) -> Option<CallableExpectation> {
        let Type::Function(id) = self.types[ty] else {
            return None;
        };
        let signature = self.function_types[id].clone();
        let receiver = match signature.receiver {
            Some(receiver) => Some(self.try_substitute(receiver, bindings)?),
            None => None,
        };
        let parameter_types = signature
            .parameter_types
            .iter()
            .map(|&ty| self.try_substitute(ty, bindings))
            .collect::<Option<_>>()?;
        Some(CallableExpectation {
            is_suspend: signature.is_suspend,
            receiver,
            parameter_types,
            return_type: self.try_substitute(signature.return_type, bindings),
        })
    }
}
