use scoop_identity::{CborIdentityRecord, DispatchTableKey, PersistentDispatchTableId};
use scoop_wire::WireError;

use super::{ExactDispatchEntryInputV1, ExactDispatchError, ExactDispatchExportV1};
use crate::{
    CallableRef, DispatchEntry, ItableRecord, StrongTypeDispatchCallableRefV2, VtableRecord,
};

impl ExactDispatchExportV1 {
    pub fn replay(
        target: crate::LirTargetProfile,
        physical: ExactDispatchPhysicalTableV1<'_>,
        inputs: &[ExactDispatchEntryInputV1<'_>],
        foundation: &crate::ConeLirFoundation,
        resolver: &mut impl ExactDispatchPhysicalCallableResolverV1,
    ) -> Result<Self, ExactDispatchError> {
        if physical.slots().len() != inputs.len() {
            return Err(ExactDispatchError::SlotCount {
                expected: physical.slots().len(),
                actual: inputs.len(),
            });
        }
        let replayed = Self::replay_from_schema(target, physical.identity(), inputs, foundation)?;
        for (entry, emitted) in replayed.entries().iter().zip(physical.slots()) {
            let position = entry.position().into_u32();
            let actual = resolver
                .resolve(emitted.callable)?
                .ok_or(ExactDispatchError::MissingPhysicalCallable(position))?;
            if actual != entry.abi() {
                return Err(ExactDispatchError::PhysicalCallable(position));
            }
        }
        Ok(replayed)
    }
}

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
    ) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError>;
}

impl<F> ExactDispatchPhysicalCallableResolverV1 for F
where
    F: FnMut(CallableRef) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError>,
{
    fn resolve(
        &mut self,
        callable: CallableRef,
    ) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError> {
        self(callable)
    }
}
