use crate::{
    DefaultCoreApplicationV1, DefaultNestedCallableAbiShapeV1,
    DefaultNestedCallableBodyArgumentsV1, DefaultNestedCallableIdentityShapeV1,
    DefaultNestedCallableIdentityV1, DefaultOperationCoreTypeV1, DefaultOperationEntityShapeV1,
    DefaultOperationEntityV1, DefaultOperationIntrinsicV1, DefaultOperationTypeRelationV1,
};
use scoop_identity::{PersistentFieldId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WirePath};

pub(crate) trait DefaultBodyDataFlowAuthority<E> {
    fn default_binding_struct_field_index(
        &mut self,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, E>;
}
pub(crate) trait DefaultBodyOperationAuthority<E> {
    fn canonical_default_operation_type(
        &mut self,
        role: DefaultOperationCoreTypeV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<SignatureTypeKey, E>;
    fn classify_default_core_application(
        &mut self,
        value: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, E>;
    fn default_operation_entity_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, E>;
    fn default_operation_type_relation(
        &mut self,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, E>;
    fn validate_default_operation_intrinsic(
        &mut self,
        intrinsic: DefaultOperationIntrinsicV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>;
}
pub(crate) trait DefaultBodyNestedAuthority<E> {
    fn default_nested_callable_identity_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E>;
    fn default_nested_callable_abi_shape(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E>;
}
