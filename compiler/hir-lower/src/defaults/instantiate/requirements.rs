use super::*;

impl Lowerer {
    /// Substitution preserves a selected implementation. Only a forwarded fact
    /// changes owner when its caller's type parameters are replaced.
    pub(super) fn instantiate_application_requirements(
        &mut self,
        source: &[hir::MemberRequirementBinding],
        context: &InstantiationContext,
    ) -> Vec<hir::MemberRequirementBinding> {
        source
            .iter()
            .map(|binding| {
                let value = match &binding.value {
                    hir::MemberRequirementBindingValue::Implementation {
                        implementation,
                        type_arguments,
                    } => hir::MemberRequirementBindingValue::Implementation {
                        implementation: *implementation,
                        type_arguments: type_arguments
                            .iter()
                            .map(|ty| self.instantiate_method_ty(*ty, &context.bindings))
                            .collect(),
                    },
                    hir::MemberRequirementBindingValue::Forwarded(id) => {
                        let requirement = self.declared_member_requirement(*id);
                        let mut contract = requirement.contract.clone();
                        contract.map_types(|ty| self.instantiate_method_ty(ty, &context.bindings));
                        self.check_member_requirement(requirement.id, &contract)
                            .expect("a selected application keeps its forwarded contract")
                    }
                };
                hir::MemberRequirementBinding {
                    requirement: binding.requirement,
                    value,
                }
            })
            .collect()
    }
}
