use std::sync::Arc;

use scoop_identity::{
    CanonicalExactTypeDiagnosticName, CborIdentityRecord, ExactTypeKey, PersistentDispatchTableId,
    PersistentExactTypeId,
};

use crate::{
    ExactInstanceLayoutV1, ExactValueLayoutV1, RefScan, StrongShapeDefinitionRefV1,
    StrongShapeDefinitionV1, StrongShapeRegistrationV1, StrongTypeDescriptorRefV2,
    TypeInstanceShapeV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDescriptorExportV1(Arc<ExactDescriptorBodyV1>);

#[derive(Debug, Eq, PartialEq)]
struct ExactDescriptorBodyV1 {
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    value_layout: Arc<ExactValueLayoutV1>,
    instance_layout: Arc<ExactInstanceLayoutV1>,
    shape: TypeInstanceShapeV1,
    object_scan: RefScan,
    ancestry: ExactDescriptorAncestryV1,
    dispatch: ExactDescriptorDispatchV1,
    diagnostic_name: CanonicalExactTypeDiagnosticName,
    physical: StrongShapeDefinitionRefV1,
    definition: StrongShapeDefinitionV1<PersistentExactTypeId>,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDescriptorAncestryV1 {
    pub(super) parent: Option<StrongTypeDescriptorRefV2>,
    pub(super) interfaces: Vec<StrongTypeDescriptorRefV2>,
}

impl ExactDescriptorAncestryV1 {
    pub const fn parent(&self) -> Option<StrongTypeDescriptorRefV2> {
        self.parent
    }

    pub fn interfaces(&self) -> &[StrongTypeDescriptorRefV2] {
        &self.interfaces
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactDescriptorItableV1 {
    pub(super) interface: StrongTypeDescriptorRefV2,
    pub(super) table: PersistentDispatchTableId,
}

impl ExactDescriptorItableV1 {
    pub const fn interface(&self) -> StrongTypeDescriptorRefV2 {
        self.interface
    }

    pub const fn table(&self) -> PersistentDispatchTableId {
        self.table
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDescriptorDispatchV1 {
    pub(super) vtable: PersistentDispatchTableId,
    pub(super) itables: Vec<ExactDescriptorItableV1>,
}

impl ExactDescriptorDispatchV1 {
    pub const fn vtable(&self) -> PersistentDispatchTableId {
        self.vtable
    }

    pub fn itables(&self) -> &[ExactDescriptorItableV1] {
        &self.itables
    }
}

impl ExactDescriptorExportV1 {
    pub(super) fn from_parts(parts: DescriptorBodyPartsV1) -> Self {
        Self(Arc::new(ExactDescriptorBodyV1 {
            exact: parts.exact,
            value_layout: parts.value_layout,
            instance_layout: parts.instance_layout,
            shape: parts.shape,
            object_scan: parts.object_scan,
            ancestry: parts.ancestry,
            dispatch: parts.dispatch,
            diagnostic_name: parts.diagnostic_name,
            physical: parts.physical,
            definition: parts.definition,
            registration: parts.registration,
        }))
    }

    pub fn exact_record(&self) -> &CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
        &self.0.exact
    }

    pub fn exact(&self) -> PersistentExactTypeId {
        self.0.exact.id()
    }

    pub fn value_layout(&self) -> &ExactValueLayoutV1 {
        &self.0.value_layout
    }

    pub fn instance_layout(&self) -> &ExactInstanceLayoutV1 {
        &self.0.instance_layout
    }

    pub fn shape(&self) -> &TypeInstanceShapeV1 {
        &self.0.shape
    }

    pub fn object_scan(&self) -> &RefScan {
        &self.0.object_scan
    }

    pub fn ancestry(&self) -> &ExactDescriptorAncestryV1 {
        &self.0.ancestry
    }

    pub fn dispatch(&self) -> &ExactDescriptorDispatchV1 {
        &self.0.dispatch
    }

    pub fn diagnostic_name(&self) -> &CanonicalExactTypeDiagnosticName {
        &self.0.diagnostic_name
    }

    pub fn physical_definition(&self) -> StrongShapeDefinitionRefV1 {
        self.0.physical
    }

    pub fn definition(&self) -> StrongShapeDefinitionV1<PersistentExactTypeId> {
        self.0.definition
    }

    pub fn registration(&self) -> StrongShapeRegistrationV1<PersistentExactTypeId> {
        self.0.registration
    }
}

pub(super) struct DescriptorBodyPartsV1 {
    pub exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    pub value_layout: Arc<ExactValueLayoutV1>,
    pub instance_layout: Arc<ExactInstanceLayoutV1>,
    pub shape: TypeInstanceShapeV1,
    pub object_scan: RefScan,
    pub ancestry: ExactDescriptorAncestryV1,
    pub dispatch: ExactDescriptorDispatchV1,
    pub diagnostic_name: CanonicalExactTypeDiagnosticName,
    pub physical: StrongShapeDefinitionRefV1,
    pub definition: StrongShapeDefinitionV1<PersistentExactTypeId>,
    pub registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
}
