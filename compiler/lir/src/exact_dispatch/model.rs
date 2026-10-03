use std::sync::Arc;

use scoop_identity::{
    CallableDefinitionOwner, DispatchDeclarationOwner, ExactCallableSignature, GcEffect,
    PersistentDispatchSlotId, PersistentDispatchTableId, PersistentExactTypeId,
};

use crate::{
    DispatchCallableAbiV1, ExactValueLayoutV1, LirTargetProfile, StrongShapeDefinitionRefV1,
    StrongShapeDefinitionV1, StrongTypeDispatchCallableRefV2,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactDispatchPositionV1(u32);

impl ExactDispatchPositionV1 {
    pub const fn from_u32(value: u32) -> Self {
        Self(value)
    }

    pub const fn into_u32(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDispatchSlotSignatureV1 {
    exact: ExactCallableSignature,
    gc_effect: GcEffect,
}

impl ExactDispatchSlotSignatureV1 {
    pub const fn new(exact: ExactCallableSignature, gc_effect: GcEffect) -> Self {
        Self { exact, gc_effect }
    }

    pub const fn exact(&self) -> &ExactCallableSignature {
        &self.exact
    }

    pub const fn gc_effect(&self) -> GcEffect {
        self.gc_effect
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDispatchReceiverAdaptationV1 {
    Identity,
    ReferenceDispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDispatchImplementationV1 {
    AbstractObligation {
        declaration: DispatchDeclarationOwner,
        trap_target: CallableDefinitionOwner,
        receiver: ExactDispatchReceiverAdaptationV1,
    },
    DirectStrongTarget {
        target: CallableDefinitionOwner,
        receiver: ExactDispatchReceiverAdaptationV1,
    },
    InterfaceDefaultTarget {
        target: CallableDefinitionOwner,
        receiver: ExactDispatchReceiverAdaptationV1,
    },
    AdjustThunkTarget(CallableDefinitionOwner),
}

impl ExactDispatchImplementationV1 {
    pub const fn target(self) -> CallableDefinitionOwner {
        match self {
            Self::AbstractObligation { trap_target, .. } => trap_target,
            Self::DirectStrongTarget { target, .. }
            | Self::InterfaceDefaultTarget { target, .. }
            | Self::AdjustThunkTarget(target) => target,
        }
    }

    pub const fn receiver_adaptation(self) -> ExactDispatchReceiverAdaptationV1 {
        match self {
            Self::AbstractObligation { receiver, .. }
            | Self::DirectStrongTarget { receiver, .. }
            | Self::InterfaceDefaultTarget { receiver, .. } => receiver,
            Self::AdjustThunkTarget(_) => ExactDispatchReceiverAdaptationV1::Identity,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDispatchRoleV1 {
    Vtable,
    Itable {
        interface_exact: PersistentExactTypeId,
    },
}

#[derive(Clone, Debug)]
pub struct ExactDispatchEntryInputV1<'a> {
    pub position: ExactDispatchPositionV1,
    pub slot: PersistentDispatchSlotId,
    pub slot_signature: ExactDispatchSlotSignatureV1,
    pub implementation: ExactDispatchImplementationV1,
    pub abi: DispatchCallableAbiV1<'a>,
    pub slot_receiver_layout: Option<&'a crate::ExactLayoutExportV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDispatchEntryV1 {
    position: ExactDispatchPositionV1,
    slot: PersistentDispatchSlotId,
    slot_signature: ExactDispatchSlotSignatureV1,
    implementation: ExactDispatchImplementationV1,
    abi: StrongTypeDispatchCallableRefV2,
    slot_receiver_layout: Option<Arc<ExactValueLayoutV1>>,
}

impl ExactDispatchEntryV1 {
    pub(super) fn from_parts(parts: ExactDispatchEntryPartsV1) -> Self {
        Self {
            position: parts.position,
            slot: parts.slot,
            slot_signature: parts.slot_signature,
            implementation: parts.implementation,
            abi: parts.abi,
            slot_receiver_layout: parts.slot_receiver_layout,
        }
    }

    pub const fn position(&self) -> ExactDispatchPositionV1 {
        self.position
    }

    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }

    pub const fn slot_signature(&self) -> &ExactDispatchSlotSignatureV1 {
        &self.slot_signature
    }

    pub const fn implementation(&self) -> ExactDispatchImplementationV1 {
        self.implementation
    }

    pub const fn abi(&self) -> StrongTypeDispatchCallableRefV2 {
        self.abi
    }

    pub fn slot_receiver_layout(&self) -> Option<&ExactValueLayoutV1> {
        self.slot_receiver_layout.as_deref()
    }
}

pub(super) struct ExactDispatchEntryPartsV1 {
    pub position: ExactDispatchPositionV1,
    pub slot: PersistentDispatchSlotId,
    pub slot_signature: ExactDispatchSlotSignatureV1,
    pub implementation: ExactDispatchImplementationV1,
    pub abi: StrongTypeDispatchCallableRefV2,
    pub slot_receiver_layout: Option<Arc<ExactValueLayoutV1>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDispatchExportV1(Arc<ExactDispatchBodyV1>);

#[derive(Debug, Eq, PartialEq)]
struct ExactDispatchBodyV1 {
    table: PersistentDispatchTableId,
    owner_exact: PersistentExactTypeId,
    target: LirTargetProfile,
    role: ExactDispatchRoleV1,
    entries: Vec<ExactDispatchEntryV1>,
    physical: StrongShapeDefinitionRefV1,
    definition: StrongShapeDefinitionV1<PersistentDispatchTableId>,
}

impl ExactDispatchExportV1 {
    pub(super) fn from_parts(parts: ExactDispatchBodyPartsV1) -> Self {
        Self(Arc::new(ExactDispatchBodyV1 {
            table: parts.table,
            owner_exact: parts.owner_exact,
            target: parts.target,
            role: parts.role,
            entries: parts.entries,
            physical: parts.physical,
            definition: parts.definition,
        }))
    }

    pub fn table(&self) -> PersistentDispatchTableId {
        self.0.table
    }

    pub fn owner_exact(&self) -> PersistentExactTypeId {
        self.0.owner_exact
    }

    pub fn target(&self) -> LirTargetProfile {
        self.0.target
    }

    pub fn role(&self) -> ExactDispatchRoleV1 {
        self.0.role
    }

    pub fn entries(&self) -> &[ExactDispatchEntryV1] {
        &self.0.entries
    }

    pub fn physical_definition(&self) -> StrongShapeDefinitionRefV1 {
        self.0.physical
    }

    pub fn definition(&self) -> StrongShapeDefinitionV1<PersistentDispatchTableId> {
        self.0.definition
    }
}

pub(super) struct ExactDispatchBodyPartsV1 {
    pub table: PersistentDispatchTableId,
    pub owner_exact: PersistentExactTypeId,
    pub target: LirTargetProfile,
    pub role: ExactDispatchRoleV1,
    pub entries: Vec<ExactDispatchEntryV1>,
    pub physical: StrongShapeDefinitionRefV1,
    pub definition: StrongShapeDefinitionV1<PersistentDispatchTableId>,
}
