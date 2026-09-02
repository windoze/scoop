use super::*;

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
        let mut declared = Vec::<(crate::CallableCandidate, usize, usize)>::new();
        match self.types[ty].clone() {
            Type::Int => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::Int, &mut declared)
            }
            Type::UInt => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::UInt, &mut declared)
            }
            Type::Boolean => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::Boolean, &mut declared)
            }
            Type::String => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::String, &mut declared)
            }
            Type::Class(mut application) => {
                let mut depth = 0;
                loop {
                    let application_value = self.class_applications[application].clone();
                    let class = application_value.template;
                    declared.extend(self.classes[class].methods.iter().copied().map(|function| {
                        (
                            crate::CallableCandidate::method(
                                function,
                                hir::MethodOwnerApplication::Class(application),
                            ),
                            depth,
                            0,
                        )
                    }));
                    let Some((base, _)) = self.classes[class].base_class.clone() else {
                        break;
                    };
                    let base = self.instantiate_ty(base, &application_value.arguments);
                    let Type::Class(base_application) = self.types[base] else {
                        unreachable!("resolved class bases are class applications")
                    };
                    application = base_application;
                    depth += 1;
                }
            }
            Type::Interface(application) => {
                self.collect_interface_method_candidates(
                    application,
                    0,
                    0,
                    None,
                    &mut Vec::new(),
                    &mut declared,
                );
            }
            Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                declared.extend(
                    self.structs[application_value.template]
                        .methods
                        .iter()
                        .copied()
                        .map(|function| {
                            (
                                crate::CallableCandidate::method(
                                    function,
                                    hir::MethodOwnerApplication::Struct(application),
                                ),
                                0,
                                0,
                            )
                        }),
                );
            }
            Type::Ptr(pointee) => {
                if let Some(owner) = self.ffi_ptr {
                    let application = self.struct_application_id(owner, vec![pointee]);
                    declared.extend(self.structs[owner].methods.iter().copied().map(|function| {
                        (
                            crate::CallableCandidate::method(
                                function,
                                hir::MethodOwnerApplication::Struct(application),
                            ),
                            0,
                            0,
                        )
                    }));
                }
            }
            Type::FunPtr(_) => {}
            Type::Enum(application) => {
                let application_value = self.enum_applications[application].clone();
                declared.extend(
                    self.enums[application_value.template]
                        .methods
                        .iter()
                        .copied()
                        .map(|function| {
                            (
                                crate::CallableCandidate::method(
                                    function,
                                    hir::MethodOwnerApplication::Enum(application),
                                ),
                                0,
                                0,
                            )
                        }),
                );
            }
            Type::Any => {}
            Type::Param(receiver_parameter) => {
                let bounds = self
                    .type_params_in_scope
                    .iter()
                    .find(|parameter| parameter.id == receiver_parameter)
                    .expect("the receiver parameter is in the active declaration scope")
                    .interface_bounds()
                    .to_vec();
                for (root, bound) in bounds.into_iter().enumerate() {
                    self.collect_interface_method_candidates(
                        bound.application,
                        0,
                        root,
                        Some((receiver_parameter, bound.application)),
                        &mut Vec::new(),
                        &mut declared,
                    );
                }
            }
            _ => {}
        }

        let mut visible = Vec::new();
        for (candidate, depth, root) in declared {
            if self.functions[candidate.function].name.rsplit('.').next() != Some(name) {
                continue;
            }
            let mut duplicate = false;
            for (existing, existing_depth, existing_root) in &visible {
                if *existing_root == root
                    && *existing_depth < depth
                    && self.same_applied_method_signature(existing, &candidate, name)
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
        for &member in &self.interfaces[application_value.template].methods {
            let function = self.interface_method_entities[member].function;
            let source = match bound {
                Some((receiver_parameter, bound)) => crate::CallableCandidateSource::Bound {
                    receiver_parameter,
                    bound,
                    member,
                },
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
        for parent in self.interfaces[application_value.template].parents.clone() {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &application_value.arguments);
            let Type::Interface(parent) = self.types[parent] else {
                unreachable!("interface parent substitutions stay interface applications")
            };
            self.collect_interface_method_candidates(parent, depth + 1, root, bound, seen, out);
        }
    }

    pub(super) fn interface_member_instances(
        &mut self,
        application: hir::InterfaceApplicationId,
    ) -> Vec<(hir::InterfaceMethodId, FunctionId, Vec<TypeId>)> {
        let mut result = Vec::new();
        self.collect_interface_member_instances(application, &mut Vec::new(), &mut result);
        result
    }

    fn collect_interface_member_instances(
        &mut self,
        application: hir::InterfaceApplicationId,
        seen: &mut Vec<hir::InterfaceApplicationId>,
        out: &mut Vec<(hir::InterfaceMethodId, FunctionId, Vec<TypeId>)>,
    ) {
        if seen.contains(&application) {
            return;
        }
        seen.push(application);
        let application_value = self.interface_applications[application].clone();
        for &member in &self.interfaces[application_value.template].methods {
            out.push((
                member,
                self.interface_method_entities[member].function,
                application_value.arguments.clone(),
            ));
        }
        for parent in self.interfaces[application_value.template].parents.clone() {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &application_value.arguments);
            let Type::Interface(parent) = self.types[parent] else {
                unreachable!("interface parent substitutions stay interface applications")
            };
            self.collect_interface_member_instances(parent, seen, out);
        }
    }

    fn same_applied_method_signature(
        &mut self,
        left: &crate::CallableCandidate,
        right: &crate::CallableCandidate,
        name: &str,
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
        self.functions[left.function].name.rsplit('.').next() == Some(name)
            && self.functions[right.function].name.rsplit('.').next() == Some(name)
            && left_sig.is_suspend == right_sig.is_suspend
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
