use scoop_identity::{
    DecodedDispatchDeclarationOwner, DecodedExactCallableSignature, DecodedPersistentId,
    DecodedStrongCallableDefinitionOwner, GcEffect, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentExactTypeId,
};
use scoop_wire::{BudgetMeter, WireError, WirePath, encode_canonical_temporary_with_meter};

use super::*;
use crate::{DecodedStrongTypeDispatchCallableRefV2, production::DecodedStrongShapeDefinitionV1};

mod codec;

#[derive(Debug)]
pub struct DecodedExactDispatchExportV1 {
    table: DecodedPersistentId<PersistentDispatchTableId>,
    owner_exact: DecodedPersistentId<PersistentExactTypeId>,
    role: DecodedExactDispatchRoleV1,
    entries: Vec<DecodedExactDispatchEntryV1>,
    definition: DecodedStrongShapeDefinitionV1<PersistentDispatchTableId>,
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
        trap_target: DecodedStrongCallableDefinitionOwner,
        receiver: DecodedExactDispatchReceiverAdaptationV1,
    },
    DirectStrongTarget {
        target: DecodedStrongCallableDefinitionOwner,
        receiver: DecodedExactDispatchReceiverAdaptationV1,
    },
    InterfaceDefaultTarget {
        target: DecodedStrongCallableDefinitionOwner,
        receiver: DecodedExactDispatchReceiverAdaptationV1,
    },
    AdjustThunkTarget(DecodedStrongCallableDefinitionOwner),
}

impl DecodedExactDispatchExportV1 {
    pub fn validate_against(
        self,
        expected: &ExactDispatchExportV1,
        meter: &mut BudgetMeter,
    ) -> Result<ExactDispatchExportV1, ExactDispatchWireError> {
        let path = WirePath::root();
        meter.charge_work(self.entries.len() as u64, &path)?;
        for entry in &self.entries {
            meter.charge_work(entry.slot_signature.exact.parameter_count() as u64, &path)?;
        }
        let actual = encode_canonical_temporary_with_meter(&self, meter, &path)?;
        let wanted = encode_canonical_temporary_with_meter(expected, meter, &path)?;
        meter.charge_work(actual.len() as u64, &path)?;
        if actual != wanted {
            return Err(ExactDispatchWireError::Mismatch);
        }
        Ok(expected.clone())
    }
}

impl DecodedCanonicalExactDispatchExportsV1 {
    pub fn validate_against(
        self,
        expected: &CanonicalExactDispatchExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalExactDispatchExportsV1, ExactDispatchTableError> {
        meter.charge_work(self.records.len() as u64, &WirePath::root())?;
        if self.records.len() != expected.records().len() {
            return Err(ExactDispatchTableError::Count {
                expected: expected.records().len(),
                actual: self.records.len(),
            });
        }
        for (index, (record, expected)) in
            self.records.into_iter().zip(expected.records()).enumerate()
        {
            record
                .validate_against(expected, meter)
                .map_err(|source| ExactDispatchTableError::Record { index, source })?;
        }
        Ok(expected.clone())
    }
}

#[derive(Debug)]
pub enum ExactDispatchWireError {
    Mismatch,
    Resource(WireError),
}

impl From<WireError> for ExactDispatchWireError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExactDispatchWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dispatch wire differs from checked replay: {self:?}"
        )
    }
}

impl std::error::Error for ExactDispatchWireError {}
