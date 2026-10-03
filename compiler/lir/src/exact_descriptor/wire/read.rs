use scoop_identity::{
    DecodedPersistentId, PersistentDispatchTableId, PersistentExactTypeId, PersistentLayoutId,
};

use crate::{
    DecodedOptionalStrongTypeDescriptorRefV2, DecodedRefScanV1, DecodedStrongTypeDescriptorRefV2,
    DecodedTypeInstanceShapeV1,
};

#[derive(Debug)]
pub struct DecodedExactDescriptorExportV1 {
    semantic: DecodedExactDescriptorSemanticProjectionV1,
    definition: crate::production::DecodedStrongShapeDefinitionV1<PersistentExactTypeId>,
    registration: crate::production::DecodedStrongShapeRegistrationV1<PersistentExactTypeId>,
}

#[derive(Debug)]
pub struct DecodedExactDescriptorSemanticProjectionV1 {
    exact: DecodedPersistentId<PersistentExactTypeId>,
    value_layout: DecodedPersistentId<PersistentLayoutId>,
    instance_layout: DecodedPersistentId<PersistentLayoutId>,
    shape: DecodedTypeInstanceShapeV1,
    object_scan: DecodedRefScanV1,
    ancestry: DecodedAncestry,
    dispatch: DecodedDispatch,
    diagnostic_name: String,
}

#[derive(Debug)]
struct DecodedAncestry {
    parent: DecodedOptionalStrongTypeDescriptorRefV2,
    interfaces: Vec<DecodedStrongTypeDescriptorRefV2>,
}

#[derive(Debug)]
struct DecodedDispatch {
    vtable: DecodedPersistentId<PersistentDispatchTableId>,
    itables: Vec<DecodedItable>,
}

#[derive(Debug)]
struct DecodedItable {
    interface: DecodedStrongTypeDescriptorRefV2,
    table: DecodedPersistentId<PersistentDispatchTableId>,
}

#[derive(Debug)]
pub struct DecodedCanonicalExactDescriptorExportsV1 {
    records: Vec<DecodedExactDescriptorExportV1>,
}

mod codec;
mod link;
mod validation;
pub use validation::ExactDescriptorWireError;
