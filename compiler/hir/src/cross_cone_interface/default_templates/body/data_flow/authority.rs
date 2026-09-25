use super::*;
use crate::DefaultBodyDataFlowAuthority;

pub(super) struct PublicAuthority<'a, A> {
    pub template: &'a ExportDefaultTemplateV1,
    pub authority: &'a mut A,
}
impl<A: DefaultLocalDataFlowSemanticAuthority<E>, E> DefaultBodyDataFlowAuthority<E>
    for PublicAuthority<'_, A>
{
    fn default_binding_struct_field_index(
        &mut self,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
    ) -> Result<u32, E> {
        self.authority
            .default_binding_struct_field_index(self.template, declaration, owner_type)
    }
}
