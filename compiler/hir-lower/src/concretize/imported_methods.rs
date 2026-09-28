use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_imported_method_callee(
        &mut self,
        source: &export::ImportedMethodCallee,
        receiver: concrete::TypeId,
        substitution: &[concrete::TypeId],
    ) -> (concrete::CallableTarget, Option<concrete::InterfaceId>) {
        let bound = match source {
            export::ImportedMethodCallee::Callable(callee) => {
                return (
                    self.lower_imported_callable_target(*callee, substitution),
                    None,
                );
            }
            export::ImportedMethodCallee::DerivedEquality(application) => {
                return (
                    concrete::CallableTarget::Local(
                        self.lower_derived_equality_application(*application, substitution),
                    ),
                    None,
                );
            }
            export::ImportedMethodCallee::InterfaceBound(bound) => bound,
        };
        let interface = self.lower_interface_type(bound.interface, substitution);
        let slot = self.interface_slot_by_source[&(interface, bound.slot)];
        if !matches!(self.types[receiver].kind, concrete::TypeKind::Interface(_)) {
            let conformances = self.concrete_bound_conformances(receiver);
            let implementation = conformances
                .iter()
                .find(|conformance| conformance.interface == interface)
                .expect("a concrete bound receiver retains its checked interface conformance");
            let method = implementation
                .methods
                .iter()
                .find(|method| method.slot == slot)
                .expect("a concrete conformance retains the declared interface slot");
            match method.target {
                concrete::InterfaceImplementationTarget::Method(function) => {
                    return (
                        concrete::CallableTarget::Local(concrete::Callable::Function(function)),
                        None,
                    );
                }
                concrete::InterfaceImplementationTarget::Imported(callee) => {
                    return (concrete::CallableTarget::Imported(callee), None);
                }
                concrete::InterfaceImplementationTarget::Abstract { .. }
                | concrete::InterfaceImplementationTarget::ImportedAbstract { .. } => {}
            }
        }
        (
            self.lower_imported_callable_target(bound.declared, substitution),
            Some(interface),
        )
    }

    fn concrete_bound_conformances(
        &mut self,
        receiver: concrete::TypeId,
    ) -> Vec<concrete::InterfaceImplementation> {
        match self.types[receiver].kind {
            concrete::TypeKind::Struct(id) => self.structs[id].interface_implementations.clone(),
            concrete::TypeKind::Enum(id) => self.enums[id].interface_implementations.clone(),
            concrete::TypeKind::Class(id) => self.classes[id].interface_implementations.clone(),
            concrete::TypeKind::Integer(_)
            | concrete::TypeKind::Boolean
            | concrete::TypeKind::String => {
                if let Some((_, value)) = self
                    .structs
                    .iter()
                    .find(|(_, value)| value.canonical_type == receiver)
                {
                    if !value.interface_implementations.is_empty() {
                        return value.interface_implementations.clone();
                    }
                }
                if let Some((_, class)) = self
                    .classes
                    .iter()
                    .find(|(_, class)| class.canonical_type == receiver)
                {
                    return class.interface_implementations.clone();
                }
                let family = match self.types[receiver].kind {
                    concrete::TypeKind::Integer(kind) => export::IntrinsicTypeKind::Integer(kind),
                    concrete::TypeKind::Boolean => export::IntrinsicTypeKind::Boolean,
                    concrete::TypeKind::String => export::IntrinsicTypeKind::String,
                    _ => unreachable!("scalar conformance has a scalar representation"),
                };
                let source = self.source.imported_intrinsic_types[&family]
                    .interface_implementations
                    .clone();
                self.lower_interface_implementations(&source, &[])
            }
            _ => unreachable!("a checked nominal bound has an explicit conformance"),
        }
    }

    pub(super) fn lower_imported_method_call(
        &mut self,
        receiver: &export::Expr,
        callee: &export::ImportedMethodCallee,
        args: &[export::Expr],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::ExprKind {
        let mut receiver = self.lower_expr(receiver, substitution, locals);
        let (callee, interface) =
            self.lower_imported_method_callee(callee, receiver.ty, substitution);
        if let Some(interface) = interface {
            receiver = self.adapt_receiver_to_interface(receiver, interface);
        }
        let mut args = args
            .iter()
            .map(|arg| self.lower_expr(arg, substitution, locals))
            .collect::<Vec<_>>();
        match callee {
            concrete::CallableTarget::Local(callee) => concrete::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee,
                args,
            },
            concrete::CallableTarget::Imported(callee) => {
                let receiver_type = receiver.ty;
                args.insert(0, receiver);
                concrete::ExprKind::ImportedDependencyCall {
                    callee,
                    binding: None,
                    args,
                    receiver: export::SourceCallReceiver::Receiver {
                        static_type: receiver_type,
                    },
                }
            }
        }
    }
}
