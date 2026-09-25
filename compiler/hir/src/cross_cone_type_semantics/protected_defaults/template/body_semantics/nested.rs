use super::*;
use crate::{
    DefaultBodyNestedAuthority, DefaultNestedCallableAbiShapeV1,
    DefaultNestedCallableAbiValidationError, DefaultNestedCallableBodyArgumentsV1,
    DefaultNestedCallableIdentityShapeV1, DefaultNestedCallableIdentityV1,
};
use scoop_wire::WirePath;

pub trait ProtectedDefaultNestedCallableSemanticAuthority<E> {
    fn default_nested_callable_identity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: crate::DefaultNestedCallableSiteV1,

        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E>;
    fn default_nested_callable_abi_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: crate::DefaultNestedCallableSiteV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,

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

        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        DefaultBodyValidationInputV1::from(self).validate_nested_callable_abi(
            self.body(),
            &mut Authority {
                template: self,
                authority,
            },
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
        site: crate::DefaultNestedCallableSiteV1,

        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E> {
        self.authority
            .default_nested_callable_identity_shape(self.template, identity, site, path)
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        site: crate::DefaultNestedCallableSiteV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,

        path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E> {
        self.authority.default_nested_callable_abi_shape(
            self.template,
            identity,
            site,
            body_arguments,
            path,
        )
    }
}
