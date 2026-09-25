use super::*;
use crate::{
    DefaultBodyOperationAuthority, DefaultCoreApplicationV1, DefaultOperationCoreTypeV1,
    DefaultOperationEntityShapeV1, DefaultOperationEntityV1, DefaultOperationIntrinsicV1,
    DefaultOperationTypeRelationV1, ExportDefaultBodyOperationTypingValidationError,
};
use scoop_identity::SignatureTypeKey;
use scoop_wire::WirePath;

pub trait ProtectedDefaultOperationTypingSemanticAuthority<E> {
    fn canonical_default_operation_type(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        role: DefaultOperationCoreTypeV1,

        path: &WirePath,
    ) -> Result<SignatureTypeKey, E>;
    fn classify_default_core_application(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        value: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, E>;
    fn default_operation_entity_shape(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        entity: DefaultOperationEntityV1<'_>,

        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, E>;
    fn default_operation_type_relation(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<bool, E>;
    fn validate_default_operation_intrinsic(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        intrinsic: DefaultOperationIntrinsicV1<'_>,

        path: &WirePath,
    ) -> Result<(), E>;
}
impl ProtectedDefaultTemplateV1 {
    /// Validates every body operation using the independently resolved provider facts.
    pub fn validate_operation_typing_semantics<
        A: ProtectedDefaultOperationTypingSemanticAuthority<E>,
        E,
    >(
        &self,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        DefaultBodyValidationInputV1::from(self).validate_operation_typing(
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
impl<A: ProtectedDefaultOperationTypingSemanticAuthority<E>, E> DefaultBodyOperationAuthority<E>
    for Authority<'_, A>
{
    fn canonical_default_operation_type(
        &mut self,
        role: DefaultOperationCoreTypeV1,

        path: &WirePath,
    ) -> Result<SignatureTypeKey, E> {
        self.authority
            .canonical_default_operation_type(self.template, role, path)
    }
    fn classify_default_core_application(
        &mut self,
        value: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, E> {
        self.authority
            .classify_default_core_application(self.template, value, path)
    }
    fn default_operation_entity_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,

        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, E> {
        self.authority
            .default_operation_entity_shape(self.template, entity, path)
    }
    fn default_operation_type_relation(
        &mut self,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<bool, E> {
        self.authority.default_operation_type_relation(
            self.template,
            relation,
            source,
            target,
            path,
        )
    }
    fn validate_default_operation_intrinsic(
        &mut self,
        intrinsic: DefaultOperationIntrinsicV1<'_>,

        path: &WirePath,
    ) -> Result<(), E> {
        self.authority
            .validate_default_operation_intrinsic(self.template, intrinsic, path)
    }
}
