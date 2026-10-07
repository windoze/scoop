use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_method_callee(
        &mut self,
        source: export::MethodCallee,
        receiver: concrete::TypeId,
        substitution: &[concrete::TypeId],
    ) -> (concrete::CallableTarget, concrete::TypeId) {
        let callee = match source {
            export::MethodCallee::Callable(callee) => {
                self.lower_callable_target(callee, substitution)
            }
            export::MethodCallee::DerivedEquality(application) => concrete::CallableTarget::Local(
                self.lower_derived_equality_application(application, substitution),
            ),
            export::MethodCallee::ImportedDerivedEquality { target, .. } => {
                let value = self.source.imported_derived_equalities[target];
                let target = self
                    .imported_derived_equalities
                    .alloc(concrete::ImportedDerivedEqualityUse(value));
                concrete::CallableTarget::DerivedEquality(target)
            }
            export::MethodCallee::Bound(bound) => {
                self.resolve_bound_callee(bound, receiver, substitution)
            }
        };
        let receiver = self.method_target_receiver(callee, receiver);
        (callee, receiver)
    }

    pub(super) fn resolve_bound_callee(
        &mut self,
        source: export::BoundCallableRefId,
        receiver: concrete::TypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::CallableTarget {
        let bound = self.source.bound_callable_refs[source].clone();
        let (interface, member, declared) = match bound.source {
            export::BoundCallableSource::Class { bound, callable } => {
                self.lower_class_application(bound, substitution);
                return self.lower_callable_target(callable, substitution);
            }
            export::BoundCallableSource::Interface {
                bound,
                member,
                declared,
            } => (
                self.lower_interface_application(bound, substitution),
                member,
                declared,
            ),
        };
        let source_slot = self.interface_reference_slot(member);
        let slot = self.interface_slot_by_source[&(interface, source_slot)];
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
                    return concrete::CallableTarget::Local(concrete::Callable::Function(function));
                }
                concrete::InterfaceImplementationTarget::Imported(callee) => {
                    let reference = self.imported_dependency_callables[callee].reference();
                    let callee = self
                        .imported_dependency_callables
                        .iter()
                        .find_map(|(id, use_)| {
                            (use_.reference() == reference
                                && matches!(
                                    use_.dispatch(),
                                    export::ImportedDependencyDispatch::Virtual { .. }
                                ))
                            .then_some(id)
                        })
                        .unwrap_or(callee);
                    return concrete::CallableTarget::Imported(callee);
                }
                concrete::InterfaceImplementationTarget::Abstract { .. }
                | concrete::InterfaceImplementationTarget::ImportedAbstract { .. } => {}
            }
        }
        self.lower_callable_target(declared, substitution)
    }

    fn method_target_receiver(
        &mut self,
        target: concrete::CallableTarget,
        receiver: concrete::TypeId,
    ) -> concrete::TypeId {
        match target {
            concrete::CallableTarget::DerivedEquality(_) => receiver,
            concrete::CallableTarget::Local(concrete::Callable::Function(function)) => {
                let owner = self.function_keys[function.into_raw().into_u32() as usize]
                    .owner
                    .expect("a method target retains its complete owner");
                match owner {
                    concrete::MethodOwner::Class(id) => self.classes[id].canonical_type,
                    concrete::MethodOwner::Struct(id) => self.structs[id].canonical_type,
                    concrete::MethodOwner::Enum(id) => self.enums[id].canonical_type,
                    concrete::MethodOwner::Interface(id) => self.interfaces[id].canonical_type,
                    concrete::MethodOwner::Object(id) => self.object_types[id].canonical_type,
                    concrete::MethodOwner::TypeOwned(ty) => ty,
                }
            }
            concrete::CallableTarget::Imported(callee) => {
                let reference = self.imported_dependency_callables[callee].reference();
                let source = self
                    .source
                    .imported_dependency_callables
                    .iter()
                    .find(|(_, use_)| use_.reference() == reference)
                    .expect("a concrete dependency use retains its export declaration")
                    .1;
                let receiver = source
                    .receiver()
                    .expect("a selected member declaration has a receiver");
                self.lower_type(receiver, &[])
            }
        }
    }

    fn concrete_bound_conformances(
        &mut self,
        receiver: concrete::TypeId,
    ) -> Vec<concrete::InterfaceImplementation> {
        match self.types[receiver].kind {
            concrete::TypeKind::Struct(id) => self.structs[id].interface_implementations.clone(),
            concrete::TypeKind::Enum(id) => self.enums[id].interface_implementations.clone(),
            concrete::TypeKind::Class(id) => self.classes[id].interface_implementations.clone(),
            concrete::TypeKind::Unit
            | concrete::TypeKind::Integer(_)
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
                    concrete::TypeKind::Unit => export::IntrinsicTypeKind::Unit,
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

    pub(super) fn lower_method_call(
        &mut self,
        receiver: concrete::Expr,
        callee: export::MethodCallee,
        args: &[export::Expr],
        direct_super: bool,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::ExprKind {
        let static_type = receiver.ty;
        let (callee, target) = self.lower_method_callee(callee, static_type, substitution);
        let receiver = self.adapt_method_receiver(receiver, target);
        let mut args = args
            .iter()
            .map(|arg| self.lower_expr(arg, substitution, locals))
            .collect::<Vec<_>>();
        match callee {
            concrete::CallableTarget::DerivedEquality(target) => {
                args.insert(0, receiver);
                concrete::ExprKind::Call {
                    callee: concrete::CallableTarget::DerivedEquality(target),
                    binding: None,
                    args,
                    receiver: export::SourceCallReceiver::Receiver { static_type },
                }
            }
            concrete::CallableTarget::Local(callee) => {
                let receiver = Box::new(receiver);
                if direct_super {
                    concrete::ExprKind::DirectSuperMethodCall {
                        receiver,
                        callee,
                        args,
                    }
                } else {
                    concrete::ExprKind::MethodCall {
                        receiver,
                        callee,
                        args,
                    }
                }
            }
            concrete::CallableTarget::Imported(callee) => {
                args.insert(0, receiver);
                concrete::ExprKind::Call {
                    callee: concrete::CallableTarget::Imported(callee),
                    binding: None,
                    args,
                    receiver: export::SourceCallReceiver::Receiver { static_type },
                }
            }
        }
    }
}
