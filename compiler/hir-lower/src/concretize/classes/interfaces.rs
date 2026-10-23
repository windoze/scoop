//! Merge applicable inherited slots using ordinary interface override order.

use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_interface_implementations(
        &mut self,
        source_implementations: &[export::InterfaceImplementation],
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::InterfaceImplementation> {
        let mut selected = Vec::<(
            concrete::InterfaceId,
            Vec<(
                concrete::InterfaceMethodSlot,
                export::InterfaceImplementationTarget,
            )>,
        )>::new();
        for implementation in source_implementations {
            if implementation.condition.as_ref().is_some_and(|condition| {
                !self.concrete_implementation_applies(condition, substitution)
            }) {
                continue;
            }
            let interface = self.lower_interface_type(implementation.interface, substitution);
            let index = selected
                .iter()
                .position(|(id, _)| *id == interface)
                .unwrap_or_else(|| {
                    selected.push((interface, Vec::new()));
                    selected.len() - 1
                });
            for method in &implementation.methods {
                let source_slot = self.interface_reference_slot(method.member);
                let slot = self.interface_slot_by_source[&(interface, source_slot)];
                if let Some((_, previous)) =
                    selected[index].1.iter_mut().find(|(id, _)| *id == slot)
                {
                    if *previous != method.target
                        && self.conformance_default_overrides(
                            method.target,
                            *previous,
                            substitution,
                        )
                    {
                        *previous = method.target;
                    }
                } else {
                    selected[index].1.push((slot, method.target));
                }
            }
        }
        selected
            .into_iter()
            .map(|(interface, methods)| concrete::InterfaceImplementation {
                interface,
                methods: methods
                    .into_iter()
                    .map(|(slot, target)| concrete::InterfaceMethodImplementation {
                        slot,
                        target: self.lower_conformance_target(target, substitution),
                    })
                    .collect(),
            })
            .collect()
    }

    fn conformance_default_overrides(
        &mut self,
        candidate: export::InterfaceImplementationTarget,
        previous: export::InterfaceImplementationTarget,
        substitution: &[concrete::TypeId],
    ) -> bool {
        let candidate = self.conformance_target_owner(candidate);
        let previous = self.conformance_target_owner(previous);
        let candidate = self.lower_type(candidate, substitution);
        let previous = self.lower_type(previous, substitution);
        candidate != previous
            && self
                .concrete_interface_closure(&[candidate])
                .contains(&previous)
    }

    fn conformance_target_owner(
        &self,
        target: export::InterfaceImplementationTarget,
    ) -> export::TypeId {
        match target {
            export::InterfaceImplementationTarget::Method(application)
            | export::InterfaceImplementationTarget::Abstract(application) => {
                match self.source.method_applications[application].owner {
                    export::MethodOwnerApplication::Class(id) => {
                        self.source.class_applications[id].canonical_type
                    }
                    export::MethodOwnerApplication::Struct(id) => {
                        self.source.struct_applications[id].canonical_type
                    }
                    export::MethodOwnerApplication::Enum(id) => {
                        self.source.enum_applications[id].canonical_type
                    }
                    export::MethodOwnerApplication::Interface(id) => {
                        self.source.interface_applications[id].canonical_type
                    }
                    export::MethodOwnerApplication::Object(id) => {
                        self.source.object_types[id].canonical_type
                    }
                }
            }
            export::InterfaceImplementationTarget::Imported(callable)
            | export::InterfaceImplementationTarget::ImportedAbstract(callable) => {
                self.source.imported_dependency_callables[callable]
                    .receiver()
                    .expect("an interface implementation is a member")
            }
            export::InterfaceImplementationTarget::ImportedTemplate(application)
            | export::InterfaceImplementationTarget::ImportedAbstractTemplate(application) => {
                let export::ImportedCallableArguments::Method { owner, .. } =
                    self.source.imported_generic_applications[application].arguments
                else {
                    unreachable!("an interface implementation is a member")
                };
                owner
            }
        }
    }

    fn lower_conformance_target(
        &mut self,
        target: export::InterfaceImplementationTarget,
        substitution: &[concrete::TypeId],
    ) -> concrete::InterfaceImplementationTarget {
        match target {
            export::InterfaceImplementationTarget::ImportedTemplate(application) => {
                concrete::InterfaceImplementationTarget::Method(
                    self.lower_imported_callable_application(
                        &self.source.imported_generic_applications[application],
                        substitution,
                    ),
                )
            }
            export::InterfaceImplementationTarget::ImportedAbstractTemplate(application) => {
                concrete::InterfaceImplementationTarget::Abstract {
                    declaration: self.lower_imported_callable_application(
                        &self.source.imported_generic_applications[application],
                        substitution,
                    ),
                }
            }
            export::InterfaceImplementationTarget::Method(application) => {
                let concrete::Callable::Function(function) =
                    self.lower_method_application(application, substitution);
                concrete::InterfaceImplementationTarget::Method(function)
            }
            export::InterfaceImplementationTarget::Imported(callable) => {
                concrete::InterfaceImplementationTarget::Imported(
                    self.imported_dependency_callable_map[&callable],
                )
            }
            export::InterfaceImplementationTarget::ImportedAbstract(callable) => {
                concrete::InterfaceImplementationTarget::ImportedAbstract {
                    declaration: self.imported_dependency_callable_map[&callable],
                }
            }
            export::InterfaceImplementationTarget::Abstract(application) => {
                let concrete::Callable::Function(declaration) =
                    self.lower_method_application(application, substitution);
                concrete::InterfaceImplementationTarget::Abstract { declaration }
            }
        }
    }
}
