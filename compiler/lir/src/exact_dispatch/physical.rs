use scoop_identity::{CborIdentityRecord, DispatchTableKey, PersistentDispatchTableId};
use scoop_wire::{BudgetMeter, WireError};

use crate::{
    CallableRef, DispatchEntry, ItableRecord, StrongTypeDispatchCallableRefV2, VtableRecord,
};

/// Borrowed proof that the dispatch payload comes from an actual LIR table.
/// Its fields stay private so an identity and an unrelated slot slice cannot
/// be paired by a caller.
#[derive(Clone, Copy)]
pub struct ExactDispatchPhysicalTableV1<'a> {
    identity: &'a CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
    slots: &'a [DispatchEntry],
}

impl<'a> From<&'a VtableRecord> for ExactDispatchPhysicalTableV1<'a> {
    fn from(value: &'a VtableRecord) -> Self {
        Self {
            identity: value.identity_record(),
            slots: value.slots(),
        }
    }
}

impl<'a> From<&'a ItableRecord> for ExactDispatchPhysicalTableV1<'a> {
    fn from(value: &'a ItableRecord) -> Self {
        Self {
            identity: value.identity_record(),
            slots: value.slots(),
        }
    }
}

impl<'a> ExactDispatchPhysicalTableV1<'a> {
    pub(super) const fn identity(
        self,
    ) -> &'a CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey> {
        self.identity
    }

    pub(super) const fn slots(self) -> &'a [DispatchEntry] {
        self.slots
    }
}

/// Resolves arena-local physical callables to the persistent typed reference
/// domain used by Strong metadata. Returning `None` rejects an unresolved
/// physical entry without inventing an identity from its arena index.
pub trait ExactDispatchPhysicalCallableResolverV1 {
    fn resolve(
        &mut self,
        callable: CallableRef,
        meter: &mut BudgetMeter,
    ) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError>;
}

impl<F> ExactDispatchPhysicalCallableResolverV1 for F
where
    F: FnMut(
        CallableRef,
        &mut BudgetMeter,
    ) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError>,
{
    fn resolve(
        &mut self,
        callable: CallableRef,
        meter: &mut BudgetMeter,
    ) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError> {
        self(callable, meter)
    }
}
