use super::*;
use hir::ImportedCallableSource;
use std::collections::HashSet;

mod members;
pub(super) use members::{InterfaceMemberInstance, InterfaceSignature};

enum InterfaceDefaultSelection {
    Default(InterfaceMemberInstance),
    Obligation(InterfaceMemberInstance),
    Conflict(Vec<InterfaceMemberInstance>),
}

impl Lowerer {
    pub(super) fn check_interface_implementation(
        &mut self,
        id: ClassId,
        span: ast::Span,
        host: &str,
    ) {
        let owner = self
            .object_by_backing_class
            .get(&id)
            .map_or(Owner::Class(id), |id| Owner::Object(*id));
        self.check_abstract_class_methods(id, span, host);
        self.check_nominal_interface_implementation(owner, span, host);
    }

    pub(super) fn check_value_interface_implementation(&mut self, owner: Owner, span: ast::Span) {
        self.check_nominal_interface_implementation(owner, span, &owner.describe(self));
    }

    pub(in crate::class) fn owner_interfaces(&mut self, owner: Owner) -> Vec<TypeId> {
        let roots = match owner {
            Owner::Class(id) => return self.class_interfaces_all(id),
            Owner::Object(id) => return self.class_interfaces_all(self.objects[id].backing_class),
            Owner::Struct(id) => self.structs[id].interfaces.clone(),
            Owner::Enum(id) => self.enums[id].interfaces.clone(),
            Owner::Interface(id) => self.interfaces[id].parents.clone(),
        };
        let mut interfaces = Vec::new();
        for root in roots {
            self.append_interface_closure(root, &mut interfaces);
        }
        interfaces
    }

    fn check_nominal_interface_implementation(
        &mut self,
        owner: Owner,
        span: ast::Span,
        host: &str,
    ) {
        let (own_methods, class) = match owner {
            Owner::Class(id) => (self.classes[id].methods.clone(), Some(id)),
            Owner::Object(id) => {
                let class = self.objects[id].backing_class;
                (self.classes[class].methods.clone(), Some(class))
            }
            Owner::Struct(id) => (self.structs[id].methods.clone(), None),
            Owner::Enum(id) => (self.enums[id].methods.clone(), None),
            Owner::Interface(_) => {
                unreachable!("interfaces declare slots rather than nominal conformances")
            }
        };
        let abstract_class =
            class.is_some_and(|id| self.classes[id].modifier == hir::ClassModifier::Abstract);
        let all_interfaces = self.owner_interfaces(owner);
        let own_owner = self.method_owner_application(owner, self.owner_type_args(owner));
        let mut candidates = own_methods
            .into_iter()
            .map(|function| crate::CallableCandidate::method(function, own_owner))
            .collect::<Vec<_>>();
        if let Some(class) = class {
            candidates.extend(self.base_chain_methods(class));
        }
        let mut conformances = Vec::new();
        let mut reported_conflicts = HashSet::new();
        let mut reported_obligations = HashSet::new();
        let mut reported_mutable = HashSet::new();
        for &interface in &all_interfaces {
            let members = self.conformance_members(interface);
            let mut methods = Vec::with_capacity(members.len());
            for member in members {
                if class.is_none()
                    && let Some(property) = &member.mutable_property
                {
                    if reported_mutable.insert((member.owner, property.clone())) {
                        self.error(
                            span,
                            format!(
                                "{host} cannot implement mutable interface property `{property}`"
                            ),
                        );
                    }
                    continue;
                }
                let implemented = candidates
                    .iter()
                    .find(|candidate| {
                        if !self.function_is_accessible(candidate.function, None)
                            || self.functions[candidate.function].method_type_param_count() != 0
                        {
                            return false;
                        }
                        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                            unreachable!("nominal candidates are methods")
                        };
                        let arguments = self.method_owner_arguments(owner).to_vec();
                        let signature =
                            self.instantiated_signature(candidate.function, &arguments, &[]);
                        let signature = InterfaceSignature::local(
                            &self.functions[candidate.function].name,
                            &signature,
                        );
                        self.same_interface_signature(&signature, &member.signature)
                    })
                    .cloned();
                let imported_implementation = if implemented.is_none() {
                    class.and_then(|class| {
                        self.inherited_imported_class_methods(class, &member.signature.name, span)
                            .into_iter()
                            .find(|method| {
                                self.same_interface_signature(&method.signature, &member.signature)
                            })
                    })
                } else {
                    None
                };
                let target = match implemented {
                    Some(candidate) if !self.is_abstract_method(candidate.function) => {
                        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                            unreachable!("nominal candidates are methods")
                        };
                        Some(hir::InterfaceImplementationTarget::Method(
                            self.record_method_application(candidate.function, owner),
                        ))
                    }
                    Some(candidate) if abstract_class => {
                        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                            unreachable!("nominal candidates are methods")
                        };
                        Some(hir::InterfaceImplementationTarget::Abstract(
                            self.record_method_application(candidate.function, owner),
                        ))
                    }
                    Some(_) => {
                        self.report_interface_obligation(
                            &mut reported_obligations,
                            &member,
                            span,
                            host,
                            true,
                        );
                        None
                    }
                    None if imported_implementation.is_some() => {
                        let method =
                            imported_implementation.expect("selected dependency implementation");
                        let abstract_method = method.declaration.interface().modality()
                            == hir::CallableModalityV1::Abstract;
                        if abstract_method && !abstract_class {
                            self.report_interface_obligation(
                                &mut reported_obligations,
                                &member,
                                span,
                                host,
                                true,
                            );
                            None
                        } else {
                            self.select_imported_callable_declaration_use(method.declaration)
                                .map(|callable| {
                                    if abstract_method {
                                        hir::InterfaceImplementationTarget::ImportedAbstract(
                                            callable,
                                        )
                                    } else {
                                        hir::InterfaceImplementationTarget::Imported(callable)
                                    }
                                })
                                .map_err(|error| {
                                    self.error(
                                        span,
                                        format!("invalid inherited interface target: {error}"),
                                    )
                                })
                                .ok()
                        }
                    }
                    None => match self.select_interface_default(&all_interfaces, &member.signature)
                    {
                        InterfaceDefaultSelection::Default(default) => {
                            self.conformance_target(&default, span)
                        }
                        InterfaceDefaultSelection::Obligation(declaration) if abstract_class => {
                            self.conformance_target(&declaration, span)
                        }
                        InterfaceDefaultSelection::Obligation(_) => {
                            self.report_interface_obligation(
                                &mut reported_obligations,
                                &member,
                                span,
                                host,
                                false,
                            );
                            None
                        }
                        InterfaceDefaultSelection::Conflict(defaults) => {
                            let mut key = defaults
                                .iter()
                                .map(|default| (default.member, default.owner))
                                .collect::<Vec<_>>();
                            key.sort_unstable();
                            key.dedup();
                            if reported_conflicts.insert(key) {
                                let providers = defaults
                                    .iter()
                                    .map(|default| {
                                        format!(
                                            "{}.{}",
                                            self.type_name(default.owner),
                                            default.signature.name
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" and ");
                                self.error(span, format!("{host} inherits conflicting defaults for `{}` from {providers}; declare an explicit override", member.signature.name));
                            }
                            None
                        }
                    },
                };
                if let Some(target) = target {
                    methods.push(hir::InterfaceMethodImplementation {
                        member: member.member,
                        target,
                    });
                }
            }
            conformances.push(hir::InterfaceImplementation { interface, methods });
        }
        match owner {
            Owner::Class(id) => self.classes[id].interface_implementations = conformances,
            Owner::Object(id) => {
                self.classes[self.objects[id].backing_class].interface_implementations =
                    conformances
            }
            Owner::Struct(id) => self.structs[id].interface_implementations = conformances,
            Owner::Enum(id) => self.enums[id].interface_implementations = conformances,
            Owner::Interface(_) => unreachable!("interfaces do not carry nominal conformances"),
        }
    }

    fn report_interface_obligation(
        &mut self,
        reported: &mut HashSet<(String, Vec<TypeId>, TypeId, bool)>,
        member: &InterfaceMemberInstance,
        span: ast::Span,
        host: &str,
        abstract_method: bool,
    ) {
        let sig = &member.signature;
        if reported.insert((
            sig.name.clone(),
            sig.parameters.clone(),
            sig.result,
            sig.is_suspend,
        )) {
            let interface = self.type_name(member.owner);
            let message = if abstract_method {
                format!(
                    "{host} leaves interface method `{interface}.{}` abstract",
                    sig.name
                )
            } else {
                format!(
                    "{host} does not implement interface method `{interface}.{}`",
                    sig.name
                )
            };
            self.error(span, message);
        }
    }

    fn select_interface_default(
        &mut self,
        interfaces: &[TypeId],
        signature: &InterfaceSignature,
    ) -> InterfaceDefaultSelection {
        let mut candidates = Vec::<InterfaceMemberInstance>::new();
        for &interface in interfaces {
            for candidate in self.conformance_members(interface) {
                if self.same_interface_signature(&candidate.signature, signature)
                    && !candidates.iter().any(|other| {
                        other.member == candidate.member && other.owner == candidate.owner
                    })
                {
                    candidates.push(candidate);
                }
            }
        }
        let mut defaults = Vec::new();
        let mut obligations = Vec::new();
        for candidate in &candidates {
            let overridden = candidates.iter().any(|other| {
                if self.types_equal(other.owner, candidate.owner) {
                    return false;
                }
                let mut closure = Vec::new();
                self.append_interface_closure(other.owner, &mut closure);
                closure
                    .into_iter()
                    .any(|ty| self.types_equal(ty, candidate.owner))
            });
            if !overridden {
                match candidate.implementation {
                    hir::InterfaceMemberImplementation::Body => defaults.push(candidate.clone()),
                    hir::InterfaceMemberImplementation::AbstractSlot => {
                        obligations.push(candidate.clone())
                    }
                }
            }
        }
        match defaults.len() {
            0 => InterfaceDefaultSelection::Obligation(
                obligations
                    .into_iter()
                    .next()
                    .expect("an unresolved interface member has a maximal abstract declaration"),
            ),
            1 => InterfaceDefaultSelection::Default(defaults.pop().expect("one default")),
            _ => InterfaceDefaultSelection::Conflict(defaults),
        }
    }

    pub(super) fn interface_method_candidates(
        &mut self,
        interfaces: &[TypeId],
    ) -> Vec<(FunctionId, Vec<TypeId>)> {
        let mut candidates = Vec::new();
        for &interface_ty in interfaces {
            for member in self.conformance_members(interface_ty) {
                let hir::InterfaceMethodReference::Local(id) = member.member else {
                    continue;
                };
                let function = self.interface_method_entities[id].function;
                let Type::Interface(owner) = self.types[member.owner] else {
                    unreachable!("a local interface member has a local owner application");
                };
                if self.function_is_accessible(function, None) {
                    let arguments = self.interface_applications[owner].arguments.clone();
                    if !candidates.contains(&(function, arguments.clone())) {
                        candidates.push((function, arguments));
                    }
                }
            }
        }
        candidates
    }
}
