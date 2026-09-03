use super::*;

impl Lowerer {
    /// The `override` rules for one member function: an overriding
    /// method must be marked, a marked method must override. For a
    /// class the candidates are the base chain's methods and the
    /// methods of every interface the class (or a base class)
    /// implements; for a value type they are the methods of its own
    /// interface list (spec 4.4.3 — implementations require the
    /// modifier there too, DESIGN.md 5.2). Interface methods have no
    /// candidates in M6 (no superinterfaces).
    pub(super) fn check_override_rules(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        owner: Owner,
    ) {
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
}
