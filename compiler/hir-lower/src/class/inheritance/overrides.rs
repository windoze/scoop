use super::*;
use hir::ImportedCallableSource;

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
                        let arguments = self.callable_candidate_owner_arguments(&candidate);
                        (candidate.function, arguments)
                    })
                    .collect();
                let interfaces = self.class_interfaces_all(class_id);
                candidates.extend(self.interface_method_candidates(&interfaces));
                candidates
            }
            Owner::Object(object) => {
                let class_id = self.objects[object].backing_class;
                let mut candidates: Vec<_> = self
                    .base_chain_methods(class_id)
                    .into_iter()
                    .filter(|candidate| self.function_is_accessible(candidate.function, None))
                    .map(|candidate| {
                        let arguments = self.callable_candidate_owner_arguments(&candidate);
                        (candidate.function, arguments)
                    })
                    .collect();
                let interfaces = self.class_interfaces_all(class_id);
                candidates.extend(self.interface_method_candidates(&interfaces));
                candidates
            }
            Owner::Struct(struct_id) => {
                self.interface_method_candidates(&self.structs[struct_id].interfaces.clone())
            }
            Owner::Enum(enum_id) => {
                self.interface_method_candidates(&self.enums[enum_id].interfaces.clone())
            }
            Owner::Interface(interface) => {
                let parents = self.interfaces[interface].parents.clone();
                self.interface_method_candidates(&parents)
            }
        };
        let sig = self.signatures[&id].clone();
        let short = decl.name.text.clone();
        let matching_overrides = candidates
            .iter()
            .filter(|(candidate, args)| {
                self.same_instantiated_signature(owner, *candidate, &short, &sig, args)
            })
            .cloned()
            .collect::<Vec<_>>();
        let interfaces = self.owner_interfaces(owner);
        let signature = super::interfaces::InterfaceSignature::local(&short, &sig);
        let class = match owner {
            Owner::Class(class) => Some(class),
            Owner::Object(object) => Some(self.objects[object].backing_class),
            _ => None,
        };
        let imported_class = class
            .map(|class| self.inherited_imported_class_methods(class, &short, decl.name.span))
            .unwrap_or_default();
        let imported_class_matches = imported_class
            .iter()
            .filter(|method| {
                sig.type_params.len() == sig.owner_type_param_count
                    && self.same_interface_signature(&signature, &method.signature)
            })
            .cloned()
            .collect::<Vec<_>>();
        if decl.is_override {
            for method in &imported_class_matches {
                self.check_imported_class_override(id, decl, &sig, method);
            }
        }
        let mut imported = Vec::new();
        for interface in interfaces {
            if self.dependency_interface_definition(interface).is_some() {
                for member in self.conformance_members(interface) {
                    if !imported
                        .iter()
                        .any(|other: &super::interfaces::InterfaceMemberInstance| {
                            other.member == member.member
                        })
                    {
                        imported.push(member);
                    }
                }
            }
        }
        let imported_matches = imported
            .iter()
            .filter(|member| {
                sig.type_params.len() == sig.owner_type_param_count
                    && self.implementation_satisfies_interface_signature(
                        owner,
                        &signature,
                        &member.signature,
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
        if decl.is_override {
            self.check_imported_interface_override(id, decl, &sig, &imported_matches);
        }
        for method in &imported_class_matches {
            self.check_override_context(
                &sig,
                &method.signature.context_parameters,
                decl.name.span,
                &format!("{}.{}", self.type_name(method.owner), method.signature.name),
            );
        }
        for member in &imported_matches {
            self.check_override_context(
                &sig,
                &member.signature.context_parameters,
                decl.name.span,
                &format!("{}.{}", self.type_name(member.owner), member.signature.name),
            );
        }
        let overrides = matching_overrides.first().cloned();
        if !matching_overrides.is_empty() {
            if decl.is_override {
                self.check_override_access_coverage(id, decl, &matching_overrides);
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
                self.override_default_sources.entry(id).or_default().push(
                    crate::defaults::DefaultOverrideSource::Local {
                        function: *candidate,
                        type_arguments: default_type_arguments,
                    },
                );
                let inherited = self.instantiated_signature(
                    *candidate,
                    arguments,
                    &sig.type_params[sig.owner_type_param_count..],
                );
                self.check_override_context(
                    &sig,
                    &inherited
                        .context_parameters
                        .iter()
                        .map(|p| p.ty)
                        .collect::<Vec<_>>(),
                    decl.name.span,
                    &self.functions[*candidate].name.clone(),
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
        if matches!(owner, Owner::Interface(_)) {
            let hir::MethodDispatch::Interface(member) = self.functions[id]
                .method
                .expect("an interface declaration is a method")
                .dispatch
            else {
                unreachable!("a non-private interface declaration owns an interface slot")
            };
            let mut overridden = matching_overrides
                .iter()
                .filter_map(|(candidate, _)| {
                    let method = self.functions[*candidate].method?;
                    match method.dispatch {
                        hir::MethodDispatch::Interface(member) => {
                            Some(hir::InterfaceMethodReference::Local(member))
                        }
                        hir::MethodDispatch::Direct
                        | hir::MethodDispatch::Virtual(_)
                        | hir::MethodDispatch::FinalOverride(_) => None,
                    }
                })
                .collect::<Vec<_>>();
            overridden.extend(imported_matches.iter().map(|member| member.member));
            self.interface_method_entities[member].overrides = overridden;
        }
        if overrides.is_none() && imported_matches.is_empty() && imported_class_matches.is_empty() {
            let local_shape = candidates
                .iter()
                .find(|(candidate, args)| {
                    self.same_instantiated_signature_shape(*candidate, &short, &sig, args)
                })
                .map(|(candidate, _)| {
                    (
                        self.functions[*candidate].name.clone(),
                        super::interfaces::InterfaceSignature::local(
                            &short,
                            &self.signatures[candidate],
                        ),
                    )
                });
            let inherited = local_shape
                .or_else(|| {
                    imported_class
                        .iter()
                        .find(|method| {
                            self.same_interface_signature_shape(&signature, &method.signature)
                        })
                        .map(|method| {
                            (
                                format!(
                                    "{}.{}",
                                    self.type_name(method.owner),
                                    method.signature.name
                                ),
                                method.signature.clone(),
                            )
                        })
                })
                .or_else(|| {
                    imported
                        .iter()
                        .find(|member| {
                            self.same_interface_signature_shape(&signature, &member.signature)
                        })
                        .map(|member| {
                            (
                                format!(
                                    "{}.{}",
                                    self.type_name(member.owner),
                                    member.signature.name
                                ),
                                member.signature.clone(),
                            )
                        })
                });
            if let Some((target, inherited)) = inherited {
                let modifier = if inherited.is_suspend != signature.is_suspend {
                    "suspend"
                } else if inherited.operator != signature.operator {
                    "operator"
                } else if inherited.infix != signature.infix {
                    "infix"
                } else {
                    "effect"
                };
                self.error(
                    decl.name.span,
                    format!("`{short}` must have the same `{modifier}` modifier as `{target}`"),
                );
                return;
            }
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
        if let Some(method) = imported_class_matches.first()
            && method.declaration.interface().modality() == hir::CallableModalityV1::Final
        {
            self.error(
                decl.name.span,
                format!(
                    "`{short}` cannot override final method `{}.{short}`",
                    self.type_name(method.owner)
                ),
            );
            return;
        }
        if matches!(owner, Owner::Class(_) | Owner::Object(_)) {
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
            let inherited_family = inherited_family.or_else(|| {
                imported_class_matches.first().map(|method| {
                    let slot = method
                        .declaration
                        .interface()
                        .slot_relations()
                        .values()
                        .first()
                        .expect("an overridable dependency class member has a virtual family");
                    self.imported_virtual_family(*slot)
                        .expect("dependency class retains its actual virtual family")
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
                    hir::MethodDispatch::Virtual(self.fresh_virtual_method(id))
                }
            };
            self.functions[id]
                .method
                .as_mut()
                .expect("the checked declaration is a method")
                .dispatch = dispatch;
        }
        let overridden_name = overrides
            .map(|(candidate, _)| self.functions[candidate].name.clone())
            .or_else(|| {
                imported_class_matches.first().map(|method| {
                    format!("{}.{}", self.type_name(method.owner), method.signature.name)
                })
            })
            .or_else(|| {
                imported_matches.first().map(|member| {
                    format!("{}.{}", self.type_name(member.owner), member.signature.name)
                })
            });
        match (overridden_name, decl.is_override) {
            (Some(target), false) => self.error(
                decl.name.span,
                format!("`{short}` overrides `{target}` and must be marked `override`"),
            ),
            (None, true) => self.error(
                decl.name.span,
                format!("`{short}` is marked `override` but does not override any method"),
            ),
            _ => {}
        }
    }

    fn check_imported_interface_override(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        signature: &FnSig,
        inherited: &[super::interfaces::InterfaceMemberInstance],
    ) {
        use hir::ImportedCallableSource;
        for member in inherited {
            let target = format!("{}.{}", self.type_name(member.owner), member.signature.name);
            let covers = self.functions[id]
                .access
                .slot
                .as_ref()
                .is_some_and(|provided| {
                    self.access_domain_is_subset(&hir::AccessDomain::universal(), &provided.0)
                });
            if !covers {
                self.error(
                    decl.name.span,
                    format!(
                        "visibility of `{}` does not cover inherited slot `{target}`",
                        decl.name.text
                    ),
                );
            }
            let hir::InterfaceMethodReference::Imported { owner, slot } = member.member else {
                unreachable!("imported candidates retain imported slots")
            };
            let Some(interface) = self.dependency_interface_definition(owner) else {
                unreachable!("imported slot owner is an interface")
            };
            let declaration = self
                .dependencies
                .as_ref()
                .expect("an imported interface has a dependency catalog")
                .callable_for_slot(interface.declaration.owner(), slot)
                .expect("resolved interface callable")
                .expect("resolved interface slot");
            for (index, parameter) in signature.params.iter().enumerate() {
                let inherited_vararg = declaration.source_interface().is_some_and(|source| {
                    source.parameters().parameters()[index]
                        .calling()
                        .is_vararg()
                });
                if matches!(parameter.calling, crate::FnParamCalling::Vararg { .. })
                    != inherited_vararg
                {
                    self.error(decl.params[index].span, format!("parameter `{}` of `{}` must have the same `vararg` shape as `{target}`", decl.params[index].name.text, decl.name.text));
                    break;
                }
            }
            let hir::PublicDeclarationOwnerV1::Nominal(nominal) = declaration.interface().owner()
            else {
                unreachable!("an inherited interface method has a nominal owner")
            };
            let owner = self
                .imported_member_owner_type(owner, nominal)
                .expect("the inherited method retains its declaring application");
            self.override_default_sources.entry(id).or_default().push(
                crate::defaults::DefaultOverrideSource::Imported {
                    declaration: declaration.interface().declaration(),
                    owner,
                },
            );
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
            && self.functions[inherited[0].0].access.declared == hir::DeclaredVisibility::Protected
            && let Some(required) = self.functions[inherited[0].0].access.slot.clone()
        {
            // `protected override` preserves the inherited protected region;
            // anchoring it at the concrete subclass would incorrectly narrow
            // access for sibling descendants of the original declaring class.
            provided = required;
            self.functions[id].access.slot = Some(provided.clone());
        }

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
        }
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
