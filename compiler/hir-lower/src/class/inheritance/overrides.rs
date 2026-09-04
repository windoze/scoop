use super::*;

impl Lowerer {
    /// The `override` rules for one member function: an overriding
    /// method must be marked, a marked method must override. For a
    /// class the candidates are the base chain's methods and the
    /// methods of every interface the class (or a base class)
    /// implements; for a value type they are the methods of its own
    /// interface list (spec 4.4.3 — implementations require the
    /// modifier there too, DESIGN.md 5.2). An interface declaration checks
    /// the complete method set of its direct superinterfaces.
    pub(super) fn check_override_rules(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        owner: Owner,
    ) {
        if matches!(owner, Owner::Interface(_))
            && self.functions[id].access.declared == hir::DeclaredVisibility::Private
        {
            return;
        }
        let candidates: Vec<(FunctionId, Vec<TypeId>)> = match owner {
            Owner::Class(class_id) => {
                let mut candidates: Vec<_> = self
                    .base_chain_methods(class_id)
                    .into_iter()
                    .filter(|candidate| self.function_is_accessible(candidate.function, None))
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
                            .filter(|method| self.function_is_accessible(*method, None))
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
            Owner::Interface(interface) => {
                let parents = self.interfaces[interface]
                    .parents
                    .iter()
                    .map(|parent| self.interface_applications[*parent].canonical_type)
                    .collect::<Vec<_>>();
                self.interface_method_candidates(&parents)
            }
        };
        let sig = self.signatures[&id].clone();
        let short = decl.name.text.clone();
        let matching_overrides = candidates
            .iter()
            .filter(|(candidate, args)| {
                self.same_instantiated_signature(*candidate, &short, &sig, args)
            })
            .cloned()
            .collect::<Vec<_>>();
        let overrides = matching_overrides.first().cloned();
        if !matching_overrides.is_empty() {
            if decl.is_override {
                self.check_override_access_coverage(id, decl, &matching_overrides);
            }
            self.override_sources.insert(
                id,
                matching_overrides
                    .iter()
                    .map(|(candidate, _)| *candidate)
                    .collect(),
            );
            if matches!(owner, Owner::Interface(_)) {
                let hir::MethodDispatch::Interface(member) = self.functions[id]
                    .method
                    .expect("an interface declaration is a method")
                    .dispatch
                else {
                    unreachable!("a non-private interface declaration owns an interface slot")
                };
                let overridden = matching_overrides
                    .iter()
                    .filter_map(|(candidate, _)| {
                        let method = self.functions[*candidate].method?;
                        match method.dispatch {
                            hir::MethodDispatch::Interface(member) => Some(member),
                            hir::MethodDispatch::Direct
                            | hir::MethodDispatch::Virtual(_)
                            | hir::MethodDispatch::FinalOverride(_) => None,
                        }
                    })
                    .collect::<Vec<_>>();
                self.interface_method_entities[member].overrides = overridden;
            }
            for (candidate, arguments) in &matching_overrides {
                let mut default_type_arguments = arguments.clone();
                for parameter in &sig.type_params[sig.owner_type_param_count..] {
                    default_type_arguments.push(self.intern_type(Type::Param(parameter.id)));
                }
                assert_eq!(
                    default_type_arguments.len(),
                    self.signatures[candidate].type_params.len()
                );
                self.override_default_type_arguments
                    .insert((id, *candidate), default_type_arguments);
                let inherited = self.instantiated_signature(
                    *candidate,
                    arguments,
                    &sig.type_params[sig.owner_type_param_count..],
                );
                let mismatch = sig.params.iter().zip(&inherited.params).position(
                    |(implementation, declaration)| {
                        matches!(implementation.calling, crate::FnParamCalling::Vararg { .. })
                            != matches!(declaration.calling, crate::FnParamCalling::Vararg { .. })
                    },
                );
                if let Some(index) = mismatch {
                    self.error(
                        decl.params[index].span,
                        format!(
                            "parameter `{}` of `{short}` must have the same `vararg` shape as `{}`",
                            decl.params[index].name.text, self.functions[*candidate].name
                        ),
                    );
                    break;
                }
            }
        }
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
            } else if self.functions[*candidate].modifiers.operator != sig.modifiers.operator
                || self.functions[*candidate]
                    .modifiers
                    .property_delegate_operator
                    != sig.modifiers.property_delegate_operator
            {
                self.error(
                    decl.name.span,
                    format!("`{short}` must have the same `operator` modifier as `{target}`"),
                );
            } else if self.functions[*candidate].modifiers.is_infix != sig.modifiers.is_infix {
                self.error(
                    decl.name.span,
                    format!("`{short}` must have the same `infix` modifier as `{target}`"),
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

    fn check_override_access_coverage(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        inherited: &[(FunctionId, Vec<TypeId>)],
    ) {
        let Some(mut provided) = self.functions[id].access.slot.clone() else {
            unreachable!("an overriding method always owns a slot contract")
        };
        if self.functions[id].access.declared == hir::DeclaredVisibility::Protected
            && let Some(required) = self.functions[inherited[0].0].access.slot.clone()
        {
            // `protected override` preserves the inherited protected region;
            // anchoring it at the concrete subclass would incorrectly narrow
            // access for sibling descendants of the original declaring class.
            provided = required;
            self.functions[id].access.slot = Some(provided.clone());
        }

        let mut witnesses = Vec::new();
        for (candidate, _) in inherited {
            let Some(required) = self.functions[*candidate].access.slot.clone() else {
                continue;
            };
            if !self.access_domain_is_subset(&required.0, &provided.0) {
                let target = self.functions[*candidate].name.clone();
                self.error(
                    decl.name.span,
                    format!(
                        "visibility of `{}` does not cover inherited slot `{target}`",
                        decl.name.text
                    ),
                );
                continue;
            }
            witnesses.push(hir::OverrideAccessWitness {
                overriding: id,
                inherited: *candidate,
                required,
                provided: provided.clone(),
            });
        }
        self.functions[id].override_access = witnesses;
    }

    pub(super) fn check_member_access_contract(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        owner: Owner,
    ) {
        let access = self.functions[id].access.clone();
        let modifier = self.functions[id]
            .method
            .expect("inheritance checks receive member functions")
            .modifier;

        if access.declared == hir::DeclaredVisibility::Private
            && modifier != hir::MethodModifier::Final
        {
            self.error(
                decl.name.span,
                format!("private method `{}` must be final", decl.name.text),
            );
        }

        if let Owner::Interface(interface) = owner {
            if self.interfaces[interface].access.declared == hir::DeclaredVisibility::Public
                && !matches!(
                    access.declared,
                    hir::DeclaredVisibility::Public | hir::DeclaredVisibility::Private
                )
            {
                self.error(
                    decl.name.span,
                    format!(
                        "member `{}` of public interface `{}` must be explicitly public",
                        decl.name.text, self.interfaces[interface].name
                    ),
                );
            }
            if access.declared == hir::DeclaredVisibility::Private && decl.is_override {
                self.error(
                    decl.name.span,
                    format!(
                        "private interface method `{}` cannot be an override",
                        decl.name.text
                    ),
                );
            }
            return;
        }

        if modifier != hir::MethodModifier::Abstract {
            return;
        }
        let Owner::Class(class) = owner else {
            return;
        };
        let required = self.classes[class].access.inheritance.0.clone();
        let Some(provided) = access.slot else {
            unreachable!("an abstract method always has a slot contract")
        };
        if !self.access_domain_is_subset(&required, &provided.0) {
            self.error(
                decl.name.span,
                format!(
                    "abstract method `{}` is not visible throughout the inheritance domain of class `{}`",
                    decl.name.text, self.classes[class].name
                ),
            );
        }
    }
}
