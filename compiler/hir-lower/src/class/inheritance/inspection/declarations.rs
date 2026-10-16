use super::*;

impl Lowerer {
    pub(in crate::class) fn derive_missing_inspection(
        &mut self,
        owner: Owner,
        member: &InterfaceMemberInstance,
        interfaces: &[TypeId],
        candidates: &[crate::CallableCandidate],
    ) -> Option<FunctionId> {
        let inspectable = self.core_coding_nominal("Inspectable")?;
        let view = self.core_coding_nominal("ValueView")?;
        let signature = &member.signature;
        if signature.name != "inspect"
            || signature.parameters.len() != 1
            || signature.is_suspend
            || !signature.context_parameters.is_empty()
            || !matches!(self.types[signature.result], Type::Interface(application) if self.interface_applications[application].template == view)
        {
            return None;
        }
        interfaces.iter().find(|ty| matches!(self.types[**ty], Type::Interface(application) if self.interface_applications[application].template == inspectable && self.interface_applications[application].arguments == signature.parameters))?;
        if self.coding_candidate_declared("inspect", &signature.parameters, candidates) {
            return None;
        }
        let function = self.register_coding_method(
            owner,
            "inspect",
            &[("value", signature.parameters[0])],
            signature.result,
        );
        self.derived_inspection_methods
            .push((function, inspectable));
        Some(function)
    }
}
