use super::*;

#[derive(Clone)]
pub(super) struct ResultContract {
    actual: ResultType,
    expected: ResultType,
    file: usize,
    span: ast::Span,
    name: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum ResultType {
    Known(TypeId),
    Function {
        function: FunctionId,
        arguments: Vec<TypeId>,
    },
}

impl Lowerer {
    pub(crate) fn declared_result_type(&mut self, function: FunctionId) -> ResultType {
        let parameters = self.signatures[&function].type_params.clone();
        let arguments = parameters
            .iter()
            .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
            .collect();
        ResultType::Function {
            function,
            arguments,
        }
    }

    pub(crate) fn defer_result_contract(
        &mut self,
        actual: ResultType,
        expected: ResultType,
        name: &str,
        span: ast::Span,
    ) {
        if actual != expected
            && !self
                .declaration_inference
                .contracts
                .iter()
                .any(|check| check.actual == actual && check.expected == expected)
        {
            self.declaration_inference.contracts.push(ResultContract {
                actual,
                expected,
                name: name.into(),
                span,
                file: self.current_file,
            });
        }
    }

    pub(crate) fn validate_inferred_result_contracts(&mut self) {
        let file = self.current_file;
        for check in std::mem::take(&mut self.declaration_inference.contracts) {
            let Some(actual) = self.contract_result_type(check.actual) else {
                continue;
            };
            let Some(expected) = self.contract_result_type(check.expected) else {
                continue;
            };
            if !self.types_equal(actual, expected) {
                self.current_file = check.file;
                self.error(
                    check.span,
                    format!(
                        "return type of `{}` must match inherited result {}, found {}",
                        check.name,
                        self.type_name(expected),
                        self.type_name(actual)
                    ),
                );
            }
        }
        self.current_file = file;
    }

    fn contract_result_type(&mut self, result: ResultType) -> Option<TypeId> {
        match result {
            ResultType::Known(ty) => Some(ty),
            ResultType::Function {
                function,
                arguments,
            } => {
                let signature = self.signatures[&function].clone();
                if signature.result_pending {
                    return None;
                }
                let bindings = signature
                    .type_params
                    .iter()
                    .map(|parameter| parameter.id)
                    .zip(arguments)
                    .collect::<Vec<_>>();
                Some(self.instantiate_method_ty(signature.return_ty, &bindings))
            }
        }
    }
}
