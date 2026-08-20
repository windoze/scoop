//! Property accessors use the same selected bound calls as ordinary methods.

use super::*;

impl Lowerer {
    pub(super) fn property_method_callee(
        &mut self,
        function: hir::FunctionId,
        owner: hir::MethodOwnerApplication,
        receiver: TypeId,
    ) -> hir::MethodCallee {
        let callable = hir::Callable::Method(self.record_method_application(function, owner));
        let source = match (self.types[receiver].clone(), owner) {
            (hir::Type::Param(receiver_parameter), hir::MethodOwnerApplication::Class(bound)) => {
                crate::CallableCandidateSource::ClassBound {
                    receiver_parameter,
                    bound,
                    member: function,
                }
            }
            _ => crate::CallableCandidateSource::Direct,
        };
        let arguments = self.method_owner_arguments(owner).to_vec();
        self.materialize_method_callee(source, callable, &arguments)
    }
}
