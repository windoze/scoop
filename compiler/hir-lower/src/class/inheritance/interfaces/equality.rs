use super::*;

impl Lowerer {
    pub(super) fn prepare_derived_operator_implementation(
        &mut self,
        owner: Owner,
        interfaces: &[TypeId],
        span: ast::Span,
    ) {
        let function = match owner {
            Owner::Struct(id) => self.structs[id].derived_equality,
            Owner::Enum(id) => self.enums[id].derived_equality,
            _ => None,
        };
        let Some(function) = function else { return };
        let signature =
            InterfaceSignature::local(&self.functions[function].name, &self.signatures[&function]);
        let required = interfaces.iter().any(|&interface| {
            self.conformance_members(interface)
                .iter()
                .any(|member| self.same_interface_signature(&signature, &member.signature))
        });
        if !required {
            return;
        }
        let parameters = self.owner_type_params(owner);
        let previous_parameters = std::mem::replace(&mut self.type_params_in_scope, parameters);
        let ty = self.owner_ty(owner);
        if let Err(reason) = self.derived_equality_candidate(ty, span) {
            self.error(span, reason);
        }
        self.type_params_in_scope = previous_parameters;
    }
}
