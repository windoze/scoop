use super::*;
use crate::{DefaultBodyDataFlowAuthority, ExportDefaultLocalDataFlowValidationError};
use scoop_identity::{PersistentFieldId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WirePath};

pub trait ProtectedDefaultLocalDataFlowSemanticAuthority<E> {
    fn default_binding_struct_field_index(
        &mut self,
        template: &ProtectedDefaultTemplateV1,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, E>;
}
impl ProtectedDefaultTemplateV1 {
    /// Validates complete definition-before-use, mutability and binding schedules.
    pub fn validate_local_data_flow_semantics<
        A: ProtectedDefaultLocalDataFlowSemanticAuthority<E>,
        E,
    >(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        DefaultBodyValidationInputV1::from(self).validate_local_data_flow(
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
impl<A: ProtectedDefaultLocalDataFlowSemanticAuthority<E>, E> DefaultBodyDataFlowAuthority<E>
    for Authority<'_, A>
{
    fn default_binding_struct_field_index(
        &mut self,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, E> {
        self.authority.default_binding_struct_field_index(
            self.template,
            declaration,
            owner_type,
            meter,
            path,
        )
    }
}
