use super::*;

impl Lowerer {
    /// Inheritance checks (pass 2.75): cycles, property shadowing,
    /// `override` rules and interface implementation — for classes and
    /// for value types implementing interfaces (spec 4.4.3).
    pub(crate) fn check_inheritance(
        &mut self,
        pending_classes: &[(ClassId, &ast::ClassDecl, usize)],
        pending_structs: &[(hir::StructId, &ast::StructDecl, usize)],
        pending_enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.check_inheritance_cycle(id, decl);
            self.check_property_shadowing(id, decl);
        }
        for &(id, decl, file_index, owner) in pending_methods {
            self.current_file = file_index;
            self.check_override_rules(id, decl, owner);
        }
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.check_interface_implementation(id, decl);
        }
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.check_value_interface_implementation(Owner::Struct(id), decl.span);
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.check_value_interface_implementation(Owner::Enum(id), decl.span);
        }
    }

    /// A class may not directly or indirectly inherit from itself.
    fn check_inheritance_cycle(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let mut seen = vec![id];
        let mut current = id;
        while let Some((base_ty, _)) = self.classes[current].base_class {
            let Type::Class(base_application) = self.types[base_ty] else {
                unreachable!("resolved class bases are class applications")
            };
            let base = self.class_applications[base_application].template;
            if seen.contains(&base) {
                let name = self.classes[id].name.clone();
                self.error(
                    decl.span,
                    format!("class `{name}` directly or indirectly inherits from itself"),
                );
                return;
            }
            seen.push(base);
            current = base;
        }
    }

    /// M6 simplification: a constructor property may not reuse the name
    /// of a base-class property (no field shadowing).
    fn check_property_shadowing(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((base_ty, _)) = self.classes[id].base_class else {
            return;
        };
        let Type::Class(base_application) = self.types[base_ty] else {
            unreachable!("resolved class bases are class applications")
        };
        let base = self.class_applications[base_application].template;
        for prop in &decl.constructor {
            if let Some((declaring, _, _, _)) = self.find_class_field(base, &prop.name.text) {
                let base_name = self.classes[declaring].name.clone();
                self.error(
                    prop.name.span,
                    format!(
                        "property `{}` of class `{}` shadows a property of base class `{base_name}`",
                        prop.name.text, decl.name.text
                    ),
                );
            }
        }
    }

    /// The `override` rules for one member function: an overriding
    /// method must be marked, a marked method must override. For a
    /// class the candidates are the base chain's methods and the
    /// methods of every interface the class (or a base class)
    /// implements; for a value type they are the methods of its own
    /// interface list (spec 4.4.3 — implementations require the
    /// modifier there too, DESIGN.md 5.2). Interface methods have no
    /// candidates in M6 (no superinterfaces).
    fn check_override_rules(&mut self, id: FunctionId, decl: &ast::FunctionDecl, owner: Owner) {
        let candidates: Vec<(FunctionId, Vec<TypeId>)> = match owner {
            Owner::Class(class_id) => {
                let mut candidates: Vec<_> = self
                    .base_chain_methods(class_id)
                    .into_iter()
                    .map(|candidate| {
                        let arguments = match candidate.owner {
                            crate::CallableCandidateOwner::Method(owner) => {
                                self.method_owner_arguments(owner).to_vec()
                            }
                            crate::CallableCandidateOwner::Function { owner_arguments } => {
                                owner_arguments
                            }
                        };
                        (candidate.function, arguments)
                    })
                    .collect();
                for interface_ty in self.class_interfaces_all(class_id) {
                    let (iface, args) = self.interface_application(interface_ty);
                    candidates.extend(
                        self.interface_methods[&iface]
                            .iter()
                            .copied()
                            .map(|method| (method, args.clone())),
                    );
                }
                candidates
            }
            Owner::Struct(struct_id) => {
                self.interface_method_candidates(&self.structs[struct_id].interfaces.clone())
            }
            Owner::Enum(enum_id) => {
                self.interface_method_candidates(&self.enums[enum_id].interfaces.clone())
            }
            Owner::Interface(_) => return,
        };
        let sig = self.signatures[&id].clone();
        let short = decl.name.text.clone();
        let overrides = candidates
            .iter()
            .find(|(candidate, args)| {
                self.same_instantiated_signature(*candidate, &short, &sig, args)
            })
            .cloned();
        if overrides.is_none()
            && let Some((candidate, _)) = candidates.iter().find(|(candidate, args)| {
                self.same_instantiated_signature_shape(*candidate, &short, &sig, args)
            })
        {
            let target = self.functions[*candidate].name.clone();
            if self.functions[*candidate].is_suspend != sig.is_suspend {
                self.error(
                    decl.name.span,
                    format!("`{short}` must have the same `suspend` modifier as `{target}`"),
                );
            } else {
                self.error(
                    decl.name.span,
                    format!("`{short}` must have the same `operator` modifier as `{target}`"),
                );
            }
            return;
        }
        if let Some((candidate, _)) = overrides.as_ref()
            && matches!(self.function_owner.get(candidate), Some(Owner::Class(_)))
            && self.functions[*candidate]
                .method
                .is_some_and(|method| method.modifier == hir::MethodModifier::Final)
        {
            let owner = self.functions[*candidate].name.clone();
            self.error(
                decl.name.span,
                format!("`{short}` cannot override final method `{owner}`"),
            );
            return;
        }
        if matches!(owner, Owner::Class(_)) {
            let inherited_family = overrides.as_ref().and_then(|(candidate, _)| {
                matches!(self.function_owner.get(candidate), Some(Owner::Class(_))).then(|| {
                    let method = self.functions[*candidate]
                        .method
                        .expect("an override candidate is a method");
                    match method.dispatch {
                        hir::MethodDispatch::Virtual(family)
                        | hir::MethodDispatch::FinalOverride(family) => family,
                        hir::MethodDispatch::Direct | hir::MethodDispatch::Interface(_) => {
                            unreachable!("an overridable class method owns a virtual family")
                        }
                    }
                })
            });
            let modifier = self.functions[id]
                .method
                .expect("the checked declaration is a method")
                .modifier;
            let dispatch = match (inherited_family, modifier) {
                (Some(family), hir::MethodModifier::Final) => {
                    hir::MethodDispatch::FinalOverride(family)
                }
                (Some(family), hir::MethodModifier::Open | hir::MethodModifier::Abstract) => {
                    hir::MethodDispatch::Virtual(family)
                }
                (None, hir::MethodModifier::Final) => hir::MethodDispatch::Direct,
                (None, hir::MethodModifier::Open | hir::MethodModifier::Abstract) => {
                    hir::MethodDispatch::Virtual(self.fresh_virtual_method())
                }
            };
            self.functions[id]
                .method
                .as_mut()
                .expect("the checked declaration is a method")
                .dispatch = dispatch;
        }
        match (overrides, decl.is_override) {
            (Some((candidate, _)), false) => {
                let owner = self.functions[candidate].name.clone();
                self.error(
                    decl.name.span,
                    format!("`{short}` overrides `{owner}` and must be marked `override`"),
                );
            }
            (None, true) => {
                self.error(
                    decl.name.span,
                    format!("`{short}` is marked `override` but does not override any method"),
                );
            }
            _ => {}
        }
    }

    /// Every method of every implemented interface (including
    /// interfaces inherited from base classes) must have a
    /// same-signature concrete method on the class or its base chain.
    /// Abstract classes may leave methods unimplemented.
    fn check_interface_implementation(&mut self, id: ClassId, decl: &ast::ClassDecl) {
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
                    .map(|function| crate::CallableCandidate::method(function, own_owner))
                    .collect::<Vec<_>>();
                candidates.extend(self.base_chain_methods(id));
                let implemented = candidates.into_iter().find(|candidate| {
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
    fn check_value_interface_implementation(&mut self, owner: Owner, span: ast::Span) {
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

    /// Whether the method is `abstract` (bodyless class method).
    fn is_abstract_method(&self, id: FunctionId) -> bool {
        self.functions[id]
            .method
            .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
    }

    /// Signature equality for override / implementation matching:
    /// name, parameter types and return type all equal (overloads
    /// only match exactly, M7).
    fn same_signature(&self, candidate: FunctionId, name: &str, sig: &FnSig) -> bool {
        self.same_signature_shape(candidate, name, sig)
            && self.functions[candidate].is_suspend == sig.is_suspend
            && self.functions[candidate]
                .method
                .is_some_and(|method| method.operator == sig.operator)
            && self.functions[candidate].attributes == sig.attributes
    }

    fn same_signature_shape(&self, candidate: FunctionId, name: &str, sig: &FnSig) -> bool {
        let function = &self.functions[candidate];
        if function.name.rsplit('.').next() != Some(name) {
            return false;
        }
        let Some(candidate_sig) = self.signatures.get(&candidate) else {
            return false;
        };
        let candidate_own_count =
            candidate_sig.type_params.len() - candidate_sig.owner_type_param_count;
        let expected_own_count = sig.type_params.len() - sig.owner_type_param_count;
        candidate_own_count == expected_own_count
            && candidate_sig.params.len() == sig.params.len()
            && candidate_sig
                .params
                .iter()
                .zip(&sig.params)
                .all(|(a, b)| self.types_equal(a.ty, b.ty))
            && self.types_equal(candidate_sig.return_ty, sig.return_ty)
    }

    pub(super) fn instantiated_signature(
        &mut self,
        method: FunctionId,
        owner_arguments: &[TypeId],
        target_method_parameters: &[hir::TypeParamDecl],
    ) -> FnSig {
        let sig = self.signatures[&method].clone();
        let (owner_parameters, method_parameters) = match &self.functions[method].genericity {
            hir::FunctionGenericity::Plain => (Vec::new(), Vec::new()),
            hir::FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => (owner_parameters.clone(), Vec::new()),
            hir::FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => (
                owner_parameters.clone(),
                method_parameters.iter().cloned().collect(),
            ),
            hir::FunctionGenericity::Generic { .. } => {
                unreachable!("nominal methods do not use generic-function identity")
            }
        };
        assert_eq!(owner_parameters.len(), owner_arguments.len());
        assert_eq!(method_parameters.len(), target_method_parameters.len());
        let mut bindings = owner_parameters
            .iter()
            .zip(owner_arguments.iter().copied())
            .map(|(parameter, argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let target_method_types = target_method_parameters
            .iter()
            .map(|parameter| (parameter.id, self.intern_type(Type::Param(parameter.id))))
            .collect::<Vec<_>>();
        bindings.extend(
            method_parameters
                .iter()
                .zip(&target_method_types)
                .map(|(source, (_, target))| (source.id, *target)),
        );
        FnSig {
            is_suspend: sig.is_suspend,
            operator: sig.operator,
            attributes: sig.attributes,
            owner_type_param_count: 0,
            type_params: target_method_parameters.to_vec(),
            params: sig
                .params
                .into_iter()
                .map(|param| FnParam {
                    name: param.name,
                    ty: self.instantiate_method_ty(param.ty, &bindings),
                })
                .collect(),
            return_ty: self.instantiate_method_ty(sig.return_ty, &bindings),
        }
    }

    fn same_instantiated_signature(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
    ) -> bool {
        self.same_instantiated_signature_shape(candidate, name, sig, args)
            && self.functions[candidate].is_suspend == sig.is_suspend
            && self.functions[candidate]
                .method
                .is_some_and(|method| method.operator == sig.operator)
    }

    fn same_instantiated_signature_shape(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
    ) -> bool {
        let target_method_parameters = &sig.type_params[sig.owner_type_param_count..];
        if self.functions[candidate].method_type_param_count() != target_method_parameters.len() {
            return false;
        }
        let candidate_sig = self.instantiated_signature(candidate, args, target_method_parameters);
        let candidate_own_count =
            candidate_sig.type_params.len() - candidate_sig.owner_type_param_count;
        let expected_own_count = sig.type_params.len() - sig.owner_type_param_count;
        self.functions[candidate].name.rsplit('.').next() == Some(name)
            && candidate_own_count == expected_own_count
            && candidate_sig.params.len() == sig.params.len()
            && candidate_sig
                .params
                .iter()
                .zip(&sig.params)
                .all(|(a, b)| self.types_equal(a.ty, b.ty))
            && self.types_equal(candidate_sig.return_ty, sig.return_ty)
    }

    fn interface_application(&self, ty: TypeId) -> (hir::InterfaceId, Vec<TypeId>) {
        match &self.types[ty] {
            Type::Interface(application) => {
                let application = &self.interface_applications[*application];
                (application.template, application.arguments.clone())
            }
            _ => unreachable!("resolved interface lists only contain interface applications"),
        }
    }

    fn interface_method_candidates(
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
                    .map(|(_, method, arguments)| (method, arguments)),
            );
        }
        candidates
    }
}
