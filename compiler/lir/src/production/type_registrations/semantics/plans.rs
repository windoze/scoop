//! Shared semantic products; the reference types keep V1 and V2 distinct.

use super::{StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1};
use crate::{
    StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2, TypeDescriptorInlineScanV1,
    TypeInstanceShapeV1,
};
use scoop_identity::{
    ConeIdentity, PersistentDispatchTableId, PersistentExactTypeId, PersistentLayoutId,
    PersistentScanId,
};

pub type StrongTypeVtableSemanticPlanV1 =
    StrongTypeVtableSemanticPlan<StrongTypeDispatchCallableRefV1>;
pub type StrongTypeItableSemanticPlanV1 =
    StrongTypeItableSemanticPlan<StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1>;
pub type StrongTypeDescriptorSemanticPlanV1 =
    StrongTypeDescriptorSemanticPlan<StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1>;
pub type StrongTypeVtableSemanticPlanV2 =
    StrongTypeVtableSemanticPlan<StrongTypeDispatchCallableRefV2>;
pub type StrongTypeItableSemanticPlanV2 =
    StrongTypeItableSemanticPlan<StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2>;
pub type StrongTypeDescriptorSemanticPlanV2 =
    StrongTypeDescriptorSemanticPlan<StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2>;

/// Canonical semantic payload of one exact type's vtable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeVtableSemanticPlan<C> {
    pub(super) table: PersistentDispatchTableId,
    pub(super) slots: Vec<C>,
}

impl<C> StrongTypeVtableSemanticPlan<C> {
    pub(crate) const fn from_artifact(table: PersistentDispatchTableId, slots: Vec<C>) -> Self {
        Self { table, slots }
    }

    pub const fn table(&self) -> PersistentDispatchTableId {
        self.table
    }

    pub fn slots(&self) -> &[C] {
        &self.slots
    }
}

/// Canonical semantic payload of one exact type's implementation table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeItableSemanticPlan<D, C> {
    pub(super) table: PersistentDispatchTableId,
    pub(super) interface: D,
    pub(super) slots: Vec<C>,
}

impl<D: Copy, C> StrongTypeItableSemanticPlan<D, C> {
    pub(crate) const fn from_artifact(
        table: PersistentDispatchTableId,
        interface: D,
        slots: Vec<C>,
    ) -> Self {
        Self {
            table,
            interface,
            slots,
        }
    }

    pub const fn table(&self) -> PersistentDispatchTableId {
        self.table
    }

    pub const fn interface(&self) -> D {
        self.interface
    }

    pub fn slots(&self) -> &[C] {
        &self.slots
    }
}

/// Arena-independent semantic definition of one local M23 TypeDescriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeDescriptorSemanticPlan<D, C> {
    pub(super) exact_type: PersistentExactTypeId,
    pub(super) diagnostic_name: String,
    pub(super) instance_layout: PersistentLayoutId,
    pub(super) instance_scan: PersistentScanId,
    pub(super) instance_shape: TypeInstanceShapeV1,
    pub(super) inline_scan: TypeDescriptorInlineScanV1,
    pub(super) parent: Option<D>,
    pub(super) vtable: StrongTypeVtableSemanticPlan<C>,
    pub(super) itables: Vec<StrongTypeItableSemanticPlan<D, C>>,
    pub(super) relations: crate::TypeDescriptorRelations<Option<D>>,
}

impl<D: Copy, C> StrongTypeDescriptorSemanticPlan<D, C> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_artifact(
        exact_type: PersistentExactTypeId,
        diagnostic_name: String,
        instance_layout: PersistentLayoutId,
        instance_scan: PersistentScanId,
        instance_shape: TypeInstanceShapeV1,
        inline_scan: TypeDescriptorInlineScanV1,
        parent: Option<D>,
        vtable: StrongTypeVtableSemanticPlan<C>,
        itables: Vec<StrongTypeItableSemanticPlan<D, C>>,
        relations: crate::TypeDescriptorRelations<Option<D>>,
    ) -> Self {
        Self {
            exact_type,
            diagnostic_name,
            instance_layout,
            instance_scan,
            instance_shape,
            inline_scan,
            parent,
            vtable,
            itables,
            relations,
        }
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    pub const fn instance_layout(&self) -> PersistentLayoutId {
        self.instance_layout
    }

    pub const fn instance_scan(&self) -> PersistentScanId {
        self.instance_scan
    }

    pub const fn instance_shape(&self) -> &TypeInstanceShapeV1 {
        &self.instance_shape
    }

    pub const fn inline_scan(&self) -> TypeDescriptorInlineScanV1 {
        self.inline_scan
    }

    pub const fn parent(&self) -> Option<D> {
        self.parent
    }

    pub const fn vtable(&self) -> &StrongTypeVtableSemanticPlan<C> {
        &self.vtable
    }

    pub fn itables(&self) -> &[StrongTypeItableSemanticPlan<D, C>] {
        &self.itables
    }

    pub const fn relations(&self) -> &crate::TypeDescriptorRelations<Option<D>> {
        &self.relations
    }
}

/// Complete canonical semantic authority for every local TypeDescriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeDescriptorSemanticPlanSet<D, C> {
    pub(super) producer: ConeIdentity,
    pub(super) target: scoop_identity::TargetProfileWireId,
    pub(super) descriptors: Vec<StrongTypeDescriptorSemanticPlan<D, C>>,
}

impl<D: Copy, C> StrongTypeDescriptorSemanticPlanSet<D, C> {
    pub(crate) const fn from_artifact(
        producer: ConeIdentity,
        target: scoop_identity::TargetProfileWireId,
        descriptors: Vec<StrongTypeDescriptorSemanticPlan<D, C>>,
    ) -> Self {
        Self {
            producer,
            target,
            descriptors,
        }
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn target(&self) -> &scoop_identity::TargetProfileWireId {
        &self.target
    }

    pub fn descriptors(&self) -> &[StrongTypeDescriptorSemanticPlan<D, C>] {
        &self.descriptors
    }
}

pub type StrongTypeDescriptorSemanticPlanSetV1 =
    StrongTypeDescriptorSemanticPlanSet<StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1>;
pub type StrongTypeDescriptorSemanticPlanSetV2 =
    StrongTypeDescriptorSemanticPlanSet<StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2>;
