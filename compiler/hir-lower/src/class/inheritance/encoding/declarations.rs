use super::*;

impl Lowerer {
    pub(in crate::class) fn derive_missing_encoding(
        &mut self,
        owner: Owner,
        member: &InterfaceMemberInstance,
        interfaces: &[TypeId],
        candidates: &[crate::CallableCandidate],
    ) -> Option<FunctionId> {
        let encodable = self.core_coding_nominal("Encodable")?;
        let encoder = self.core_coding_nominal("Encoder")?;
        let encodable_ty = interfaces.iter().copied().find(|ty| matches!(self.types[*ty], Type::Interface(application) if self.interface_applications[application].template == encodable))?;
        let signature = &member.signature;
        if signature.name != "encode"
            || signature.parameters.len() != 1
            || signature.result != self.unit
            || signature.is_suspend
            || !signature.context_parameters.is_empty()
            || !matches!(self.types[signature.parameters[0]], Type::Interface(application) if self.interface_applications[application].template == encoder)
        {
            return None;
        }
        // Keep ordinary diagnostics for an invalid explicit override.
        if self.coding_candidate_declared("encode", signature.parameters[0], candidates) {
            return None;
        }
        let function = self.register_coding_method(
            owner,
            "encode",
            "encoder",
            signature.parameters[0],
            self.unit,
        );
        self.derived_encoding_methods
            .push((function, owner, encodable_ty));
        Some(function)
    }
}
