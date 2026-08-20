use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_reference_target(
        &mut self,
        target: &export::CallableReferenceTarget,
        function_type: concrete::FunctionTypeId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::CallableReferenceTarget {
        match target {
            export::CallableReferenceTarget::BoundIntrinsic {
                receiver,
                intrinsic,
                ..
            } => {
                if intrinsic.requires_arithmetic_exception() {
                    self.lower_arithmetic_exception_type();
                }
                concrete::CallableReferenceTarget::BoundIntrinsic {
                    receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                    intrinsic: *intrinsic,
                }
            }
            export::CallableReferenceTarget::Named(callee) => {
                concrete::CallableReferenceTarget::Named(
                    self.lower_callable_target(*callee, substitution),
                )
            }
            export::CallableReferenceTarget::Local {
                definition_path,
                callee,
            } => {
                let concrete::CallableTarget::Local(concrete::Callable::Function(function)) =
                    self.lower_callable_target(*callee, substitution)
                else {
                    unreachable!("a local reference retains its lexical implementation");
                };
                let local_function =
                    self.intern_local_function(function, definition_path.clone(), function_type);
                concrete::CallableReferenceTarget::Local {
                    local_function,
                    callee: concrete::Callable::Function(function),
                }
            }
            export::CallableReferenceTarget::BoundMember { receiver, callee } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let (callee, target) = self.lower_method_callee(*callee, receiver.ty, substitution);
                let receiver = self.adapt_method_receiver(receiver, target);
                concrete::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(receiver),
                    callee,
                }
            }
            export::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                concrete::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                    callee: self.lower_callable_target(*callee, substitution),
                }
            }
        }
    }

    pub(in crate::concretize) fn lower_callable_target(
        &mut self,
        callee: export::CallableTarget,
        substitution: &[concrete::TypeId],
    ) -> concrete::CallableTarget {
        match callee {
            export::CallableTarget::Local(callable) => {
                concrete::CallableTarget::Local(self.lower_callable(callable, substitution))
            }
            export::CallableTarget::Application(application) => {
                let application = self.source.imported_generic_applications[application].clone();
                concrete::CallableTarget::Local(concrete::Callable::Function(
                    self.lower_imported_callable_application(&application, substitution),
                ))
            }
            export::CallableTarget::Dependency(callee) => {
                concrete::CallableTarget::Imported(self.imported_dependency_callable_map[&callee])
            }
        }
    }
}
