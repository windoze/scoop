use super::*;
use crate::call_resolution::candidates::DeclarationSignature;
use crate::expr::ResolvedCallTypeArgument;

impl Lowerer {
    pub(crate) fn alias_forwarding_signature<D: Clone>(
        &mut self,
        alias: &AliasTemplate,
        signature: &DeclarationSignature<D>,
    ) -> DeclarationSignature<D> {
        let alias = self
            .reload_alias_template(alias)
            .expect("an applicable alias retains its declaration");
        let mapped = self
            .alias_constructor_signature(&alias, signature, &[], alias.name.span)
            .expect("an applicable alias has complete host arguments");
        let mut signature = mapped.signature;
        let fixed_count = alias
            .arguments
            .as_ref()
            .map_or(alias.outer_parameter_count, Vec::len);
        let bindings = signature
            .callable_parameters
            .iter()
            .zip(&mapped.explicit)
            .take(fixed_count)
            .map(|(parameter, argument)| {
                let ResolvedCallTypeArgument::Explicit { ty, .. } = argument else {
                    unreachable!("alias host arguments are fixed");
                };
                (parameter.id, *ty)
            })
            .collect::<Vec<_>>();
        signature.callable_parameters.drain(..fixed_count);
        signature.substitute_types(self, &bindings);
        signature
    }
}
