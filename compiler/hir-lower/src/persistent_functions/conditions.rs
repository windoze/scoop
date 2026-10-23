use super::*;
use scoop_identity::SignatureConditionKey;

impl FunctionIdentityBuilder<'_> {
    pub(super) fn function_conditions(
        &self,
        function: hir::FunctionId,
        binders: &[SignatureBinder],
    ) -> Result<Vec<SignatureConditionKey>, PersistentFunctionIdentityError> {
        let signature = &self.lowerer.signatures[&function];
        let parameters = &signature.type_params[signature.owner_type_param_count..];
        if parameters.iter().any(|parameter| {
            !binders
                .iter()
                .any(|binder| binder.parameter == parameter.id)
        }) {
            return Err(self.failure(function, Detail::InvalidBinderInheritance));
        }
        crate::persistent_types::signature_parameter_conditions(parameters, binders, |ty| {
            self.type_mapper
                .map(ty, binders)
                .map_err(|error| self.failure(function, Detail::InvalidSignatureType(error)))
        })
    }
}
