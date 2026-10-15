use scoop_identity::{
    DecodedCallableDefinitionOwner, DecodedDispatchDeclarationOwner, DecodedExactCallableSignature,
    DecodedPersistentId, GcEffect, PersistentDispatchSlotId, PersistentDispatchTableId,
    PersistentExactTypeId,
};

use super::*;
use crate::{DecodedStrongTypeDispatchCallableRefV2, production::DecodedStrongShapeDefinitionV1};

mod codec;
mod resolve;

#[derive(Debug)]
pub struct DecodedExactDispatchExportV1 {
    semantic: DecodedExactDispatchSemanticProjectionV1,
    definition: DecodedStrongShapeDefinitionV1<PersistentDispatchTableId>,
}

#[derive(Debug)]
pub struct DecodedExactDispatchSemanticProjectionV1 {
    table: DecodedPersistentId<PersistentDispatchTableId>,
    owner_exact: DecodedPersistentId<PersistentExactTypeId>,
    role: DecodedExactDispatchRoleV1,
    entries: Vec<DecodedExactDispatchEntryV1>,
}

#[derive(Debug)]
pub struct DecodedCanonicalExactDispatchExportsV1 {
    records: Vec<DecodedExactDispatchExportV1>,
}

#[derive(Debug)]
enum DecodedExactDispatchRoleV1 {
    Vtable,
    Itable(DecodedPersistentId<PersistentExactTypeId>),
}

#[derive(Debug)]
struct DecodedExactDispatchEntryV1 {
    position: u32,
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    slot_signature: DecodedExactDispatchSlotSignatureV1,
    implementation: DecodedExactDispatchImplementationV1,
    abi: DecodedStrongTypeDispatchCallableRefV2,
}

#[derive(Debug)]
struct DecodedExactDispatchSlotSignatureV1 {
    exact: DecodedExactCallableSignature,
    gc_effect: GcEffect,
}

#[derive(Debug)]
enum DecodedExactDispatchReceiverAdaptationV1 {
    Identity,
    ReferenceDispatch,
}

#[derive(Debug)]
enum DecodedExactDispatchImplementationV1 {
    AbstractObligation {
        declaration: DecodedDispatchDeclarationOwner,
        trap_target: DecodedCallableDefinitionOwner,
        receiver: DecodedExactDispatchReceiverAdaptationV1,
    },
    DirectStrongTarget {
        target: DecodedCallableDefinitionOwner,
        receiver: DecodedExactDispatchReceiverAdaptationV1,
    },
    InterfaceDefaultTarget {
        target: DecodedCallableDefinitionOwner,
        receiver: DecodedExactDispatchReceiverAdaptationV1,
    },
    AdjustThunkTarget(DecodedCallableDefinitionOwner),
}
