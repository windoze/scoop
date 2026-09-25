use super::*;
use crate::DefaultBodyOperationAuthority;

pub(super) struct PublicAuthority<'a, A> {
    pub template: &'a ExportDefaultTemplateV1,
    pub authority: &'a mut A,
}
impl<A: DefaultOperationTypingSemanticAuthority<E>, E> DefaultBodyOperationAuthority<E>
    for PublicAuthority<'_, A>
{
    fn canonical_default_operation_type(
        &mut self,
        role: DefaultOperationCoreTypeV1,

        _path: &WirePath,
    ) -> Result<SignatureTypeKey, E> {
        self.authority
            .canonical_default_operation_type(self.template, role)
    }
    fn classify_default_core_application(
        &mut self,
        value: &SignatureTypeKey,

        _path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, E> {
        self.authority
            .classify_default_core_application(self.template, value)
    }
    fn default_operation_entity_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,

        _path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, E> {
        self.authority
            .default_operation_entity_shape(self.template, entity)
    }
    fn default_operation_type_relation(
        &mut self,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,

        _path: &WirePath,
    ) -> Result<bool, E> {
        self.authority
            .default_operation_type_relation(self.template, relation, source, target)
    }
    fn validate_default_operation_intrinsic(
        &mut self,
        intrinsic: DefaultOperationIntrinsicV1<'_>,

        _path: &WirePath,
    ) -> Result<(), E> {
        self.authority
            .validate_default_operation_intrinsic(self.template, intrinsic)
    }
}
