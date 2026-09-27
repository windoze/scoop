use super::*;
use std::collections::HashSet;

enum InterfaceDefaultSelection {
    Default {
        function: FunctionId,
        owner: hir::InterfaceApplicationId,
    },
    Obligation,
    Conflict(Vec<InterfaceDefaultCandidate>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct InterfaceDefaultCandidate {
    function: FunctionId,
    owner: hir::InterfaceApplicationId,
    implementation: hir::InterfaceMemberImplementation,
}

#[derive(PartialEq, Eq, Hash)]
struct InterfaceObligationKey {
    name: String,
    parameter_types: Vec<TypeId>,
    return_ty: TypeId,
    is_suspend: bool,
}

impl Lowerer {
    /// Every method of every implemented interface (including
    /// interfaces inherited from base classes) must have a
    /// same-signature concrete method on the class or its base chain.
    /// Abstract classes may leave methods unimplemented.
    pub(super) fn check_interface_implementation(
        &mut self,
        id: ClassId,
        span: ast::Span,
        host: &str,
    ) {
        self.classes[id].interface_implementations.clear();
        let abstract_class = self.classes[id].modifier == hir::ClassModifier::Abstract;
        let all_interfaces = self.class_interfaces_all(id);
        let mut reported_conflicts = HashSet::new();
        let mut reported_obligations = HashSet::new();
        for &interface_ty in &all_interfaces {
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
                let own_owner = self.object_by_backing_class.get(&id).map_or(
                    hir::MethodOwnerApplication::Class(self.classes[id].self_application),
                    |object| hir::MethodOwnerApplication::Object(self.objects[*object].object_type),
                );
                let mut candidates = self.classes[id]
                    .methods
                    .iter()
                    .copied()
                    .map(|function| crate::CallableCandidate::method(function, own_owner))
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
                    Some(candidate) if !self.is_abstract_method(candidate.function) => {
                        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                            unreachable!("interface implementations are methods")
                        };
                        let application = self.record_method_application(candidate.function, owner);
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Method(application),
                        });
                    }
                    Some(_) if abstract_class => {
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Subclass,
                        });
                    }
                    Some(_) => {
                        let iface_name = self.interfaces[iface].name.clone();
                        let key = Self::interface_obligation_key(short, &sig);
                        if reported_obligations.insert(key) {
                            self.error(
                                span,
                                format!(
                                    "{host} leaves interface method `{iface_name}.{short}` abstract"
                                ),
                            );
                        }
                    }
                    None => match self.select_interface_default(&all_interfaces, short, &sig) {
                        InterfaceDefaultSelection::Default { function, owner } => {
                            let application = self.record_method_application(
                                function,
                                hir::MethodOwnerApplication::Interface(owner),
                            );
                            implementations.push(hir::InterfaceMethodImplementation {
                                member,
                                target: hir::InterfaceImplementationTarget::Method(application),
                            });
                        }
                        InterfaceDefaultSelection::Obligation if abstract_class => {
                            implementations.push(hir::InterfaceMethodImplementation {
                                member,
                                target: hir::InterfaceImplementationTarget::Subclass,
                            });
                        }
                        InterfaceDefaultSelection::Obligation => {
                            let iface_name = self.interfaces[iface].name.clone();
                            let key = Self::interface_obligation_key(short, &sig);
                            if reported_obligations.insert(key) {
                                self.error(
                                    span,
                                    format!("{host} does not implement interface method `{iface_name}.{short}`"),
                                );
                            }
                        }
                        InterfaceDefaultSelection::Conflict(functions) => {
                            self.report_default_conflict(
                                &mut reported_conflicts,
                                &functions,
                                span,
                                host,
                                short,
                            );
                        }
                    },
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
            Owner::Class(_) | Owner::Interface(_) | Owner::Object(_) => return,
        };
        let mut interfaces = Vec::new();
        for interface in declared_interfaces {
            self.append_interface_closure(interface, &mut interfaces);
        }
        let all_interfaces = interfaces.clone();
        let mut reported_conflicts = HashSet::new();
        let mut reported_mutable_properties = HashSet::new();
        let mut reported_obligations = HashSet::new();
        match owner {
            Owner::Struct(id) => self.structs[id].interface_implementations.clear(),
            Owner::Enum(id) => self.enums[id].interface_implementations.clear(),
            Owner::Class(_) | Owner::Interface(_) | Owner::Object(_) => {}
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
                if let hir::InterfaceMemberRole::PropertySetter(property) =
                    self.interface_method_entities[member].role
                {
                    if reported_mutable_properties.insert(property) {
                        let property_name = self.properties[property].name.clone();
                        let host = owner.describe(self);
                        self.error(
                            span,
                            format!(
                                "{host} cannot implement mutable interface property `{property_name}`"
                            ),
                        );
                    }
                    continue;
                }
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
                match implemented {
                    Some(function) => {
                        let owner_application =
                            self.method_owner_application(owner, self.owner_type_args(owner));
                        let application =
                            self.record_method_application(function, owner_application);
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Method(application),
                        });
                    }
                    None => match self.select_interface_default(&all_interfaces, short, &sig) {
                        InterfaceDefaultSelection::Default { function, owner } => {
                            let application = self.record_method_application(
                                function,
                                hir::MethodOwnerApplication::Interface(owner),
                            );
                            implementations.push(hir::InterfaceMethodImplementation {
                                member,
                                target: hir::InterfaceImplementationTarget::Method(application),
                            });
                        }
                        InterfaceDefaultSelection::Obligation => {
                            let iface_name = self.interfaces[iface].name.clone();
                            let host = owner.describe(self);
                            let key = Self::interface_obligation_key(short, &sig);
                            if reported_obligations.insert(key) {
                                self.error(
                                    span,
                                    format!(
                                        "{host} does not implement interface method `{iface_name}.{short}`"
                                    ),
                                );
                            }
                        }
                        InterfaceDefaultSelection::Conflict(functions) => {
                            let host = owner.describe(self);
                            self.report_default_conflict(
                                &mut reported_conflicts,
                                &functions,
                                span,
                                &host,
                                short,
                            );
                        }
                    },
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
                Owner::Class(_) | Owner::Interface(_) | Owner::Object(_) => unreachable!(),
            }
        }
    }
    fn select_interface_default(
        &mut self,
        interfaces: &[TypeId],
        name: &str,
        signature: &crate::FnSig,
    ) -> InterfaceDefaultSelection {
        let mut candidates = Vec::new();
        for &interface_ty in interfaces {
            let Type::Interface(application) = self.types[interface_ty] else {
                unreachable!("default candidates come from interface applications")
            };
            let value = self.interface_applications[application].clone();
            for &member in &self.interfaces[value.template].methods.clone() {
                let declaration = self.interface_method_entities[member].clone();
                if !self.function_is_accessible(declaration.function, None)
                    || !self.same_instantiated_signature(
                        declaration.function,
                        name,
                        signature,
                        &value.arguments,
                    )
                {
                    continue;
                }
                let candidate = InterfaceDefaultCandidate {
                    function: declaration.function,
                    owner: application,
                    implementation: declaration.implementation,
                };
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
        let mut frontier = Vec::new();
        for candidate in &candidates {
            let overridden = candidates.iter().any(|other| {
                other.owner != candidate.owner
                    && self.interface_application_reaches(
                        other.owner,
                        candidate.owner,
                        &mut Vec::new(),
                    )
            });
            if !overridden {
                frontier.push(*candidate);
            }
        }
        let defaults = frontier
            .into_iter()
            .filter(|candidate| {
                candidate.implementation == hir::InterfaceMemberImplementation::Body
            })
            .collect::<Vec<_>>();
        match defaults.as_slice() {
            [] => InterfaceDefaultSelection::Obligation,
            [candidate] => InterfaceDefaultSelection::Default {
                function: candidate.function,
                owner: candidate.owner,
            },
            _ => InterfaceDefaultSelection::Conflict(defaults),
        }
    }

    fn interface_obligation_key(name: &str, signature: &crate::FnSig) -> InterfaceObligationKey {
        InterfaceObligationKey {
            name: name.to_string(),
            parameter_types: signature
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .collect(),
            return_ty: signature.return_ty,
            is_suspend: signature.is_suspend,
        }
    }

    fn interface_application_reaches(
        &mut self,
        current: hir::InterfaceApplicationId,
        target: hir::InterfaceApplicationId,
        visiting: &mut Vec<hir::InterfaceApplicationId>,
    ) -> bool {
        if current == target {
            return true;
        }
        if visiting.contains(&current) {
            return false;
        }
        visiting.push(current);
        let value = self.interface_applications[current].clone();
        let parents = self.interfaces[value.template].parents.clone();
        let reaches = parents.into_iter().any(|parent| {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &value.arguments);
            let Type::Interface(parent) = self.types[parent] else {
                unreachable!("interface parent substitution stays an interface")
            };
            self.interface_application_reaches(parent, target, visiting)
        });
        visiting.pop();
        reaches
    }

    fn report_default_conflict(
        &mut self,
        reported: &mut HashSet<Vec<(FunctionId, hir::InterfaceApplicationId)>>,
        candidates: &[InterfaceDefaultCandidate],
        span: ast::Span,
        host: &str,
        name: &str,
    ) {
        let mut key = candidates
            .iter()
            .map(|candidate| (candidate.function, candidate.owner))
            .collect::<Vec<_>>();
        key.sort_by_key(|(function, owner)| (function.into_raw(), owner.into_raw()));
        key.dedup();
        if !reported.insert(key.clone()) {
            return;
        }
        let providers = key
            .iter()
            .map(|(function, owner)| {
                let owner_ty = self.interface_applications[*owner].canonical_type;
                let member = self.functions[*function]
                    .name
                    .rsplit('.')
                    .next()
                    .expect("interface methods are qualified");
                format!("{}.{member}", self.type_name(owner_ty))
            })
            .collect::<Vec<_>>()
            .join(" and ");
        self.error(
            span,
            format!(
                "{host} inherits conflicting defaults for `{name}` from {providers}; declare an explicit override"
            ),
        );
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
