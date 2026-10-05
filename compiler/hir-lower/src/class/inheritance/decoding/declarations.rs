use super::super::interfaces::InterfaceMemberInstance;
use super::*;

impl Lowerer {
    pub(in crate::class) fn derive_missing_decoding(
        &mut self,
        owner: Owner,
        member: &InterfaceMemberInstance,
        interfaces: &[TypeId],
        candidates: &[crate::CallableCandidate],
    ) -> Option<FunctionId> {
        let decodable = self.core_coding_nominal("Decodable")?;
        let decoder = self.core_coding_nominal("Decoder")?;
        let signature = &member.signature;
        if signature.name != "decode"
            || signature.parameters.len() != 1
            || signature.is_suspend
            || !signature.context_parameters.is_empty()
            || !matches!(self.types[signature.parameters[0]], Type::Interface(application) if self.interface_applications[application].template == decoder)
        {
            return None;
        }
        interfaces.iter().find(|ty| matches!(self.types[**ty], Type::Interface(application) if self.interface_applications[application].template == decodable && self.interface_applications[application].arguments == [signature.result]))?;
        if self.coding_candidate_declared("decode", signature.parameters[0], candidates) {
            return None;
        }
        let function = self.register_coding_method(
            owner,
            "decode",
            "decoder",
            signature.parameters[0],
            signature.result,
        );
        self.derived_decoding_methods.push((function, decodable));
        Some(function)
    }
}
