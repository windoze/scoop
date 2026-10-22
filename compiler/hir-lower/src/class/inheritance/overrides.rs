use super::interfaces::InterfaceMemberInstance;
use super::*;
use hir::ImportedCallableSource;

mod access;
mod imported;

pub(super) struct MemberOverride {
    pub name: ast::Ident,
    pub is_override: bool,
    pub parameter_spans: Vec<ast::Span>,
}

impl MemberOverride {
    pub fn source(source: &ast::FunctionSignature) -> Self {
        Self {
            name: source.name.clone(),
            is_override: source.is_override,
            parameter_spans: source
                .params
                .iter()
                .map(|parameter| parameter.span)
                .collect(),
        }
    }
}

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
        decl: &MemberOverride,
        owner: Owner,
    ) {
        if matches!(owner, Owner::Interface(_))
            && self.functions[id].access.declared == hir::DeclaredVisibility::Private
        {
            return;
        }
        let mut candidates: Vec<(FunctionId, Vec<TypeId>)> = match owner {
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
        let accessor_kind = self.function_accessor_kind(id);
        candidates
            .retain(|(candidate, _)| self.function_accessor_kind(*candidate) == accessor_kind);
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
        let signature = super::interfaces::InterfaceSignature::local(&short, &sig, accessor_kind);
        let class = match owner {
            Owner::Class(class) => Some(class),
            Owner::Object(object) => Some(self.objects[object].backing_class),
            _ => None,
        };
        let imported_class = class
            .map(|class| {
                self.inherited_imported_class_methods(class, &short, accessor_kind, decl.name.span)
            })
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
                self.defer_override_result(
                    id,
                    *candidate,
                    default_type_arguments.clone(),
                    decl.name.span,
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
                        decl.parameter_spans[index],
                        format!(
                            "parameter `{}` of `{short}` must have the same `vararg` shape as `{}`",
                            sig.params[index].name.text, self.functions[*candidate].name
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
                            accessor_kind,
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
}
