use super::*;
use crate::DefaultBodyNestedAuthority;

pub(super) struct PublicAuthority<'a, A> {
    pub template: &'a ExportDefaultTemplateV1,
    pub authority: &'a mut A,
}
impl<A: DefaultNestedCallableSemanticAuthority<E>, E> DefaultBodyNestedAuthority<E>
    for PublicAuthority<'_, A>
{
    fn default_nested_callable_identity_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E> {
        self.authority
            .default_nested_callable_identity_shape(self.template, identity)
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E> {
        self.authority
            .default_nested_callable_abi_shape(self.template, identity, body_arguments)
    }
}
