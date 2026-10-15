use super::*;

pub(crate) enum ExpressionExpectation {
    Type(TypeId),
    Lambda(LambdaExpectation),
}

/// Candidate-local input to lambda inference, never an incomplete HIR type.
pub(crate) struct LambdaExpectation {
    pub(super) is_suspend: bool,
    pub(super) receiver: Option<TypeId>,
    pub(super) parameter_types: Vec<TypeId>,
    pub(super) return_type: Option<TypeId>,
}

impl Lowerer {
    pub(in crate::expr) fn lambda_expectation(&self, ty: TypeId) -> Option<LambdaExpectation> {
        let Type::Function(id) = self.types[ty] else {
            return None;
        };
        let signature = &self.function_types[id];
        Some(LambdaExpectation {
            is_suspend: signature.is_suspend,
            receiver: signature.receiver,
            parameter_types: signature.parameter_types.clone(),
            return_type: Some(signature.return_type),
        })
    }

    pub(crate) fn partial_lambda_expectation(
        &mut self,
        ty: TypeId,
        bindings: &[Option<TypeId>],
    ) -> Option<LambdaExpectation> {
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
        Some(LambdaExpectation {
            is_suspend: signature.is_suspend,
            receiver,
            parameter_types,
            return_type: self.try_substitute(signature.return_type, bindings),
        })
    }
}
