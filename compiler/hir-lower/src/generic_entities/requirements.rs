use super::*;

impl Lowerer {
    pub(crate) fn member_requirement_declarations(
        &self,
        id: hir::MemberRequirementId,
    ) -> Vec<hir::TypeParamDecl> {
        match self.member_requirement_owners[&id] {
            hir::MemberRequirementOwner::Source(function) => self.functions[function]
                .type_params()
                .into_iter()
                .cloned()
                .collect(),
            hir::MemberRequirementOwner::Imported(template) => self.imported_generic_templates
                [template]
                .type_parameters
                .declarations()
                .to_vec(),
        }
    }

    pub(crate) fn declared_member_requirement(
        &self,
        id: hir::MemberRequirementId,
    ) -> hir::MemberRequirement {
        self.member_requirement_declarations(id)
            .into_iter()
            .flat_map(|parameter| parameter.requirements)
            .find(|requirement| requirement.id == id)
            .expect("a requirement retains its original declaration")
    }

    pub(crate) fn record_member_requirement_owner(
        &mut self,
        owner: hir::MemberRequirementOwner,
        parameters: &[hir::TypeParamDecl],
    ) {
        for requirement in parameters
            .iter()
            .flat_map(|parameter| &parameter.requirements)
        {
            self.member_requirement_owners
                .entry(requirement.id)
                .or_insert(owner);
        }
    }

    pub(crate) fn member_requirement_bindings(
        &mut self,
        parameters: &[hir::TypeParamDecl],
        arguments: &[TypeId],
    ) -> Vec<hir::MemberRequirementBinding> {
        let substitution = parameters
            .iter()
            .map(|parameter| parameter.id)
            .zip(arguments.iter().copied())
            .collect::<Vec<_>>();
        parameters
            .iter()
            .flat_map(|parameter| &parameter.requirements)
            .map(|requirement| {
                let mut contract = requirement.contract.clone();
                contract.map_types(|ty| self.instantiate_method_ty(ty, &substitution));
                let value = self
                    .check_member_requirement(requirement.id, &contract)
                    .expect("selected generic applications have satisfied member requirements");
                hir::MemberRequirementBinding {
                    requirement: requirement.id,
                    value,
                }
            })
            .collect()
    }

    pub(crate) fn record_imported_callable_application(
        &mut self,
        mut application: hir::ImportedGenericCallableApplication,
    ) -> hir::ImportedGenericCallableApplicationId {
        if let hir::ImportedCallableTypeParameters::Declared(parameters) =
            &self.imported_generic_templates[application.template].type_parameters
        {
            let parameters = parameters.clone();
            self.record_member_requirement_owner(
                hir::MemberRequirementOwner::Imported(application.template),
                &parameters,
            );
            let arguments = application.arguments.substitution(
                &self.types,
                &self.enum_applications,
                &self.struct_applications,
                &self.class_applications,
                &self.interface_applications,
            );
            application.requirements = self.member_requirement_bindings(&parameters, &arguments);
        }
        self.imported_generic_applications.alloc(application)
    }

    /// Unused surrounding binders are absent from a cached query's environment.
    /// Retaining original substitution slots allows its already typed body to be reused.
    pub(crate) fn requirement_type_arguments(
        &mut self,
        contract: &hir::MemberRequirementContract,
    ) -> Vec<TypeId> {
        let mut pending = contract.types().collect::<Vec<_>>();
        let mut visited = HashSet::new();
        let mut parameters = Vec::new();
        while let Some(ty) = pending.pop() {
            if !visited.insert(ty) {
                continue;
            }
            if let Some(application) = self.nominal_application(ty) {
                pending.extend(application.arguments);
                continue;
            }
            match &self.types[ty] {
                Type::Param(id) | Type::Companion(id) => parameters.push(*id),
                Type::Ptr(ty) => pending.push(*ty),
                Type::Tuple(types) => pending.extend(types),
                Type::Function(id) | Type::FunPtr(id) => {
                    pending.extend(self.function_types[*id].parameters());
                    pending.push(self.function_types[*id].return_type);
                }
                _ => {}
            }
        }
        let length = parameters
            .iter()
            .map(|id| id.into_raw() as usize + 1)
            .max()
            .unwrap_or(0);
        let mut arguments = vec![self.unit; length];
        for parameter in parameters {
            arguments[parameter.into_raw() as usize] = self.intern_type(Type::Param(parameter));
        }
        arguments
    }
}
