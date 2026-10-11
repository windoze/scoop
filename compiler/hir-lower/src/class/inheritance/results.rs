use super::interfaces::InterfaceMemberInstance;
use super::*;
use crate::declaration_inference::ResultType;

impl Lowerer {
    pub(super) fn defer_override_result(
        &mut self,
        implementation: FunctionId,
        declaration: FunctionId,
        arguments: Vec<TypeId>,
        span: ast::Span,
    ) {
        if self.signatures[&implementation].result_pending
            || self.signatures[&declaration].result_pending
        {
            let actual = self.declared_result_type(implementation);
            let name = self.source_function_name(implementation).to_owned();
            self.defer_result_contract(
                actual,
                ResultType::Function {
                    function: declaration,
                    arguments,
                },
                &name,
                span,
            );
        }
    }

    pub(super) fn defer_imported_override_result(
        &mut self,
        function: FunctionId,
        result: TypeId,
        span: ast::Span,
    ) {
        if self.signatures[&function].result_pending {
            let actual = self.declared_result_type(function);
            let name = self.source_function_name(function).to_owned();
            self.defer_result_contract(actual, ResultType::Known(result), &name, span);
        }
    }

    pub(super) fn interface_result_type(&self, member: &InterfaceMemberInstance) -> ResultType {
        match member.member {
            hir::InterfaceMethodReference::Local(id) => {
                let Type::Interface(owner) = self.types[member.owner] else {
                    unreachable!("interface slot owner is an interface")
                };
                ResultType::Function {
                    function: self.interface_method_entities[id].function,
                    arguments: self.interface_applications[owner].arguments.clone(),
                }
            }
            hir::InterfaceMethodReference::Imported { .. } => {
                ResultType::Known(member.signature.result)
            }
        }
    }

    pub(super) fn defer_conformance_result(
        &mut self,
        candidate: &crate::CallableCandidate,
        member: &InterfaceMemberInstance,
        span: ast::Span,
    ) {
        if self.signatures[&candidate.function].result_pending || member.signature.result_pending {
            let arguments = self.callable_candidate_owner_arguments(candidate);
            let actual = ResultType::Function {
                function: candidate.function,
                arguments,
            };
            let expected = self.interface_result_type(member);
            self.defer_result_contract(actual, expected, &member.signature.name, span);
        }
    }
}
