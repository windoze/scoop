use super::*;

impl Lowerer {
    /// Every method of every implemented interface (including
    /// interfaces inherited from base classes) must have a
    /// same-signature concrete method on the class or its base chain.
    /// Abstract classes may leave methods unimplemented.
    pub(super) fn check_interface_implementation(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        self.classes[id].interface_implementations.clear();
        let abstract_class = self.classes[id].modifier == hir::ClassModifier::Abstract;
        let class_name = self.classes[id].name.clone();
        for interface_ty in self.class_interfaces_all(id) {
            let (iface, _args) = self.interface_application(interface_ty);
            let application = match self.types[interface_ty] {
                Type::Interface(application) => application,
                _ => unreachable!("class interface closure contains interface applications"),
            };
            let methods = self.interface_member_instances(application);
            let mut implementations = Vec::with_capacity(methods.len());
            for (member, method, member_arguments) in methods {
                if self.functions[method].method_type_param_count() != 0 {
                    // Interface methods with their own type parameters are
                    // rejected while their declarations are resolved.  Do
                    // not manufacture a dispatch application for that
                    // invalid declaration during error recovery.
                    continue;
                }
                let qualified = self.functions[method].name.clone();
                let sig = self.instantiated_signature(method, &member_arguments, &[]);
                let short = qualified.rsplit('.').next().expect("methods are qualified");
                let own_owner =
                    hir::MethodOwnerApplication::Class(self.classes[id].self_application);
                let mut candidates = self.classes[id]
                    .methods
                    .iter()
                    .copied()
                    .map(|function| {
                        crate::CallableCandidate::inheritance_method(function, own_owner)
                    })
                    .collect::<Vec<_>>();
                candidates.extend(self.base_chain_methods(id));
                let implemented = candidates.into_iter().find(|candidate| {
                    if !self.function_is_accessible(candidate.function, None) {
                        return false;
                    }
                    let owner_arguments = match &candidate.owner {
                        crate::CallableCandidateOwner::Method(owner) => {
                            self.method_owner_arguments(*owner).to_vec()
                        }
                        crate::CallableCandidateOwner::Function { owner_arguments } => {
                            owner_arguments.clone()
                        }
                    };
                    self.same_instantiated_signature(
                        candidate.function,
                        short,
                        &sig,
                        &owner_arguments,
                    )
                });
                match implemented {
                    Some(candidate)
                        if abstract_class || !self.is_abstract_method(candidate.function) =>
                    {
                        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                            unreachable!("interface implementations are methods")
                        };
                        let application = self.record_method_application(candidate.function, owner);
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Method(application),
                        });
                    }
                    _ if abstract_class => {
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Subclass,
                        });
                    }
                    _ => {
                        let iface_name = self.interfaces[iface].name.clone();
                        self.error(
                        decl.span,
                        format!(
                            "class `{class_name}` does not implement interface method `{iface_name}.{short}`"
                        ),
                    );
                    }
                }
            }
            self.classes[id]
                .interface_implementations
                .push(hir::InterfaceImplementation {
                    interface: application,
                    methods: implementations,
                });
        }
    }

    /// Every method of every interface a value type implements must
    /// have a same-signature method among the value type's own methods
    /// (spec 4.4.3; value types have no base chain to inherit from,
    /// and their methods are always concrete).
    pub(super) fn check_value_interface_implementation(&mut self, owner: Owner, span: ast::Span) {
        let (declared_interfaces, own_methods): (Vec<TypeId>, Vec<FunctionId>) = match owner {
            Owner::Struct(id) => (
                self.structs[id].interfaces.clone(),
                self.structs[id].methods.clone(),
            ),
            Owner::Enum(id) => (
                self.enums[id].interfaces.clone(),
                self.enums[id].methods.clone(),
            ),
            // Only called for value types.
            Owner::Class(_) | Owner::Interface(_) => return,
        };
        let mut interfaces = Vec::new();
        for interface in declared_interfaces {
            self.append_interface_closure(interface, &mut interfaces);
        }
        match owner {
            Owner::Struct(id) => self.structs[id].interface_implementations.clear(),
            Owner::Enum(id) => self.enums[id].interface_implementations.clear(),
            Owner::Class(_) | Owner::Interface(_) => {}
        }
        for interface_ty in interfaces {
            let (iface, _args) = self.interface_application(interface_ty);
            let application = match self.types[interface_ty] {
                Type::Interface(application) => application,
                _ => unreachable!("value interface list contains interface applications"),
            };
            let methods = self.interface_member_instances(application);
            let mut implementations = Vec::with_capacity(methods.len());
            for (member, method, member_arguments) in methods {
                if self.functions[method].method_type_param_count() != 0 {
                    // The declaration-site diagnostic is authoritative;
                    // an illegal generic interface member has no itable
                    // identity for conformance recovery to complete.
                    continue;
                }
                let qualified = self.functions[method].name.clone();
                let sig = self.instantiated_signature(method, &member_arguments, &[]);
                let short = qualified.rsplit('.').next().expect("methods are qualified");
                let implemented = own_methods
                    .iter()
                    .copied()
                    .filter(|candidate| self.function_is_accessible(*candidate, None))
                    .find(|&candidate| self.same_signature(candidate, short, &sig));
                if let Some(function) = implemented {
                    let owner_application =
                        self.method_owner_application(owner, self.owner_type_args(owner));
                    let application = self.record_method_application(function, owner_application);
                    implementations.push(hir::InterfaceMethodImplementation {
                        member,
                        target: hir::InterfaceImplementationTarget::Method(application),
                    });
                } else {
                    let iface_name = self.interfaces[iface].name.clone();
                    let host = owner.describe(self);
                    self.error(
                        span,
                        format!(
                            "{host} does not implement interface method `{iface_name}.{short}`"
                        ),
                    );
                }
            }
            let implementation = hir::InterfaceImplementation {
                interface: application,
                methods: implementations,
            };
            match owner {
                Owner::Struct(id) => self.structs[id]
                    .interface_implementations
                    .push(implementation),
                Owner::Enum(id) => self.enums[id]
                    .interface_implementations
                    .push(implementation),
                Owner::Class(_) | Owner::Interface(_) => unreachable!(),
            }
        }
    }
    pub(super) fn interface_application(&self, ty: TypeId) -> (hir::InterfaceId, Vec<TypeId>) {
        match &self.types[ty] {
            Type::Interface(application) => {
                let application = &self.interface_applications[*application];
                (application.template, application.arguments.clone())
            }
            _ => unreachable!("resolved interface lists only contain interface applications"),
        }
    }

    pub(super) fn interface_method_candidates(
        &mut self,
        interfaces: &[TypeId],
    ) -> Vec<(FunctionId, Vec<TypeId>)> {
        let mut candidates = Vec::new();
        for &interface_ty in interfaces {
            let Type::Interface(application) = self.types[interface_ty] else {
                unreachable!("resolved interface lists contain interface applications")
            };
            candidates.extend(
                self.interface_member_instances(application)
                    .into_iter()
                    .filter(|(_, method, _)| self.function_is_accessible(*method, None))
                    .map(|(_, method, arguments)| (method, arguments)),
            );
        }
        candidates
    }
}
