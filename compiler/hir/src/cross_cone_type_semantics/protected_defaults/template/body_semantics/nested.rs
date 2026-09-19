use super::*;
use crate::{
    DefaultBodyNestedAuthority, DefaultNestedCallableAbiShapeV1,
    DefaultNestedCallableAbiValidationError, DefaultNestedCallableBodyArgumentsV1,
    DefaultNestedCallableIdentityShapeV1, DefaultNestedCallableIdentityV1,
};
use scoop_wire::{BudgetMeter, WirePath};

pub trait ProtectedDefaultNestedCallableSemanticAuthority<E> {
    fn default_nested_callable_identity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E>;
    fn default_nested_callable_abi_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E>;
}
impl ProtectedDefaultTemplateV1 {
    /// Validates nested descriptor identity, capture ABI and complete local-callable uses.
    pub fn validate_nested_callable_abi_semantics<
        A: ProtectedDefaultNestedCallableSemanticAuthority<E>,
        E,
    >(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        DefaultBodyValidationInputV1::from(self).validate_nested_callable_abi(
            self.body(),
            &mut Authority {
                template: self,
                authority,
            },
            meter,
            path,
        )
    }
}
struct Authority<'a, A> {
    template: &'a ProtectedDefaultTemplateV1,
    authority: &'a mut A,
}
impl<A: ProtectedDefaultNestedCallableSemanticAuthority<E>, E> DefaultBodyNestedAuthority<E>
    for Authority<'_, A>
{
    fn default_nested_callable_identity_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E> {
        self.authority
            .default_nested_callable_identity_shape(self.template, identity, meter, path)
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E> {
        self.authority.default_nested_callable_abi_shape(
            self.template,
            identity,
            body_arguments,
            meter,
            path,
        )
    }
}
