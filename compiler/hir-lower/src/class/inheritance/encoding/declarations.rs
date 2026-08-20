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
        let signature = &member.signature;
        if signature.name != "encode"
            || signature.parameters.len() != 2
            || signature.result != self.unit
            || signature.is_suspend
            || !signature.context_parameters.is_empty()
            || !matches!(self.types[signature.parameters[1]], Type::Interface(application) if self.interface_applications[application].template == encoder)
        {
            return None;
        }
        interfaces.iter().find(|ty| matches!(self.types[**ty], Type::Interface(application) if self.interface_applications[application].template == encodable && self.interface_applications[application].arguments == [signature.parameters[0]]))?;
        // An invalid explicit implementation keeps its ordinary override diagnostic.
        if self.coding_candidate_declared("encode", &signature.parameters, candidates) {
            return None;
        }
        let function = self.register_coding_method(
            owner,
            "encode",
            &[
                ("value", signature.parameters[0]),
                ("encoder", signature.parameters[1]),
            ],
            self.unit,
        );
        self.derived_encoding_methods.push((function, encodable));
        Some(function)
    }
}
