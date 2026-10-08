use super::*;

mod candidates;
mod equality;
mod interface_members;

impl Lowerer {
    /// All visible methods named `name` on a receiver type (M7 overload
    /// candidates). A class contributes its own methods followed by
    /// the base chain nearest-first; an overriding signature suppresses
    /// the corresponding base declaration so one dynamic-dispatch slot
    /// never appears as two ambiguous overload candidates.
    ///
    /// `Any` deliberately contributes no methods. Capabilities such as
    /// ToString, Hash and operator equals are ordinary declared interfaces or
    /// members and therefore enter this list only through their real owner.
    pub(crate) fn methods_by_name(
        &mut self,
        ty: TypeId,
        name: &str,
    ) -> Vec<crate::CallableCandidate> {
        let declared = self.method_candidates(ty);
        self.visible_method_candidates(declared, Some(ty), |this, candidate| {
            this.functions[candidate.function].name.rsplit('.').next() == Some(name)
        })
    }

    pub(crate) fn super_methods_by_name(
        &mut self,
        ty: TypeId,
        name: &str,
    ) -> Vec<crate::CallableCandidate> {
        let declared = self.method_candidates(ty);
        self.visible_method_candidates(declared, None, |this, candidate| {
            !matches!(
                this.function_owner.get(&candidate.function),
                Some(Owner::Interface(_))
            ) && this.functions[candidate.function].name.rsplit('.').next() == Some(name)
        })
    }

    /// All visible methods carrying one exact, declaration-validated operator
    /// identity. Operator consumers use this index instead of recovering a
    /// role from the source function name (notably for `componentN`).
    pub(crate) fn methods_by_operator(
        &mut self,
        ty: TypeId,
        operator: hir::OperatorKind,
    ) -> Vec<crate::CallableCandidate> {
        let declared = self.method_candidates(ty);
        self.visible_method_candidates(declared, Some(ty), |this, candidate| {
            this.signatures[&candidate.function].modifiers.operator == Some(operator)
        })
    }

    /// Explain an otherwise inaccessible same-name member after every lower
    /// callable layer has failed. Keeping this separate from candidate
    /// collection ensures an inaccessible member never blocks an applicable
    /// extension, while protected receiver violations still get a precise
    /// diagnostic.
    pub(crate) fn inaccessible_method_message(
        &mut self,
        receiver_ty: TypeId,
        name: &str,
    ) -> Option<String> {
        let candidate = self
            .method_candidates(receiver_ty)
            .into_iter()
            .map(|(candidate, _, _)| candidate)
            .find(|candidate| {
                self.functions[candidate.function].name.rsplit('.').next() == Some(name)
                    && !self.function_is_accessible(candidate.function, Some(receiver_ty))
            });
        let Some(candidate) = candidate else {
            return self.inaccessible_imported_method_message(receiver_ty, name);
        };
        let function = &self.functions[candidate.function];
        if function.access.declared == hir::DeclaredVisibility::Protected
            && let Some(base) = function
                .method
                .and_then(|method| self.receiver_class(method.owner))
            && let Some(current) = self.protected_access_class(base)
            && self.receiver_class(receiver_ty).is_some()
        {
            return Some(format!(
                "protected method `{name}` cannot be accessed through receiver of static type `{}`; receiver must be `{}` or one of its subclasses",
                self.type_name(receiver_ty),
                self.classes[current].name
            ));
        }
        Some(format!("method `{name}` is not accessible here"))
    }

    fn collect_class_method_candidates(
        &mut self,
        mut application: hir::ClassApplicationId,
        root: usize,
        bound: Option<(hir::TypeParamId, hir::ClassApplicationId)>,
        out: &mut Vec<(crate::CallableCandidate, usize, usize)>,
    ) {
        let mut depth = 0;
        loop {
            let application_value = self.class_applications[application].clone();
            let Some(class) = self.source_class_id(application_value.template) else {
                break;
            };
            let owner_application = match self.object_by_backing_class.get(&class).copied() {
                Some(object) => self.method_owner_application(
                    Owner::Object(object),
                    application_value.arguments.clone(),
                ),
                None => hir::MethodOwnerApplication::Class(application),
            };
            out.extend(self.classes[class].methods.iter().copied().map(|function| {
                let source = match bound {
                    Some((receiver_parameter, bound)) => {
                        crate::CallableCandidateSource::ClassBound {
                            receiver_parameter,
                            bound,
                            member: function,
                        }
                    }
                    None => crate::CallableCandidateSource::Direct,
                };
                (
                    crate::CallableCandidate {
                        function,
                        owner: crate::CallableCandidateOwner::Method(owner_application),
                        source,
                    },
                    depth,
                    root,
                )
            }));
            if let Some((receiver_parameter, _)) = bound {
                for interface in self.classes[class].interfaces.clone() {
                    let interface = self.instantiate_ty(interface, &application_value.arguments);
                    let Type::Interface(interface) = self.types[interface] else {
                        continue;
                    };
                    self.collect_interface_method_candidates(
                        interface,
                        depth + 1,
                        root,
                        Some((receiver_parameter, interface)),
                        &mut Vec::new(),
                        out,
                    );
                }
            }
            let Some(base) = self.classes[class].base_class else {
                break;
            };
            let base = self.instantiate_ty(base, &application_value.arguments);
            let base_application = match self.types[base] {
                Type::Class(application) => application,
                _ => unreachable!("resolved class bases have class types"),
            };
            application = base_application;
            depth += 1;
        }
    }

    fn visible_method_candidates(
        &mut self,
        declared: Vec<(crate::CallableCandidate, usize, usize)>,
        receiver_ty: Option<TypeId>,
        mut matches: impl FnMut(&Self, &crate::CallableCandidate) -> bool,
    ) -> Vec<crate::CallableCandidate> {
        let mut visible = Vec::new();
        for (candidate, depth, root) in declared {
            if !self.function_is_accessible(candidate.function, receiver_ty)
                || !matches(self, &candidate)
            {
                continue;
            }
            let mut duplicate = false;
            for (existing, existing_depth, existing_root) in &visible {
                if *existing_root == root
                    && *existing_depth < depth
                    && self.same_applied_method_signature(existing, &candidate)
                {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                visible.push((candidate, depth, root));
            }
        }
        visible
            .into_iter()
            .map(|(candidate, _, _)| candidate)
            .collect()
    }

    /// Add the ordinary source methods declared on a compiler-represented
    /// intrinsic type.  The type representation stays built in, but its
    /// semantic surface comes exclusively from the exact declaration owner
    /// recorded by the intrinsic-type registry.
    fn collect_intrinsic_type_methods(
        &self,
        kind: hir::IntrinsicTypeKind,
        out: &mut Vec<(crate::CallableCandidate, usize, usize)>,
    ) {
        let Some(&(owner, _provider)) = self.intrinsic_type_owners.get(&kind) else {
            return;
        };
        let (methods, owner) = match owner {
            crate::IntrinsicTypeOwner::Struct(owner) => (
                self.structs[owner].methods.as_slice(),
                hir::MethodOwnerApplication::Struct(self.structs[owner].self_application),
            ),
            crate::IntrinsicTypeOwner::Class(owner) => (
                self.classes[owner].methods.as_slice(),
                hir::MethodOwnerApplication::Class(self.classes[owner].self_application),
            ),
        };
        out.extend(
            methods
                .iter()
                .copied()
                .map(|function| (crate::CallableCandidate::method(function, owner), 0, 0)),
        );
    }

    fn collect_interface_method_candidates(
        &mut self,
        application: hir::InterfaceApplicationId,
        depth: usize,
        root: usize,
        bound: Option<(hir::TypeParamId, hir::InterfaceApplicationId)>,
        seen: &mut Vec<hir::InterfaceApplicationId>,
        out: &mut Vec<(crate::CallableCandidate, usize, usize)>,
    ) {
        if seen.contains(&application) {
            return;
        }
        seen.push(application);
        let application_value = self.interface_applications[application].clone();
        let Some(source_id) = self.source_interface_id(application_value.template) else {
            return;
        };
        if depth == 0 && bound.is_none() && self.current_owner == Some(Owner::Interface(source_id))
        {
            out.extend(
                self.interfaces[source_id]
                    .private_methods
                    .iter()
                    .copied()
                    .map(|function| {
                        (
                            crate::CallableCandidate {
                                function,
                                owner: crate::CallableCandidateOwner::Method(
                                    hir::MethodOwnerApplication::Interface(application),
                                ),
                                source: crate::CallableCandidateSource::Direct,
                            },
                            depth,
                            root,
                        )
                    }),
            );
        }
        for &member in &self.interfaces[source_id].methods {
            let function = self.interface_method_entities[member].function;
            let source = match bound {
                Some((receiver_parameter, bound)) => {
                    crate::CallableCandidateSource::InterfaceBound {
                        receiver_parameter,
                        bound,
                        member,
                    }
                }
                None => crate::CallableCandidateSource::Direct,
            };
            out.push((
                crate::CallableCandidate {
                    function,
                    owner: crate::CallableCandidateOwner::Method(
                        hir::MethodOwnerApplication::Interface(application),
                    ),
                    source,
                },
                depth,
                root,
            ));
        }
        for parent in self.interfaces[source_id].parents.clone() {
            let parent = self.instantiate_ty(parent, &application_value.arguments);
            if let Type::Interface(parent) = self.types[parent] {
                self.collect_interface_method_candidates(parent, depth + 1, root, bound, seen, out);
            }
        }
    }

    fn same_applied_method_signature(
        &mut self,
        left: &crate::CallableCandidate,
        right: &crate::CallableCandidate,
    ) -> bool {
        if self.functions[left.function].method_type_param_count()
            != self.functions[right.function].method_type_param_count()
        {
            return false;
        }
        let left_arguments = match &left.owner {
            crate::CallableCandidateOwner::Method(owner) => {
                self.method_owner_arguments(*owner).to_vec()
            }
            crate::CallableCandidateOwner::Function { owner_arguments } => owner_arguments.clone(),
        };
        let right_arguments = match &right.owner {
            crate::CallableCandidateOwner::Method(owner) => {
                self.method_owner_arguments(*owner).to_vec()
            }
            crate::CallableCandidateOwner::Function { owner_arguments } => owner_arguments.clone(),
        };
        let canonical_method_parameters = match &self.functions[left.function].genericity {
            hir::FunctionGenericity::GenericMethod {
                method_parameters, ..
            } => method_parameters.iter().cloned().collect::<Vec<_>>(),
            hir::FunctionGenericity::Plain
            | hir::FunctionGenericity::OwnerParameterizedMethod { .. } => Vec::new(),
            hir::FunctionGenericity::Generic { .. } => {
                unreachable!("member candidates do not use generic-function identity")
            }
        };
        let left_sig = self.instantiated_signature(
            left.function,
            &left_arguments,
            &canonical_method_parameters,
        );
        let right_sig = self.instantiated_signature(
            right.function,
            &right_arguments,
            &canonical_method_parameters,
        );
        left_sig.is_suspend == right_sig.is_suspend
            && left_sig.attributes == right_sig.attributes
            && left_sig.type_params.len() == right_sig.type_params.len()
            && left_sig.params.len() == right_sig.params.len()
            && left_sig
                .params
                .iter()
                .zip(&right_sig.params)
                .all(|(left, right)| self.types_equal(left.ty, right.ty))
            && self.types_equal(left_sig.return_ty, right_sig.return_ty)
    }
}
