use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MirTypeSelectionBrand(u64);

/// Request-local semantic selection. It is not a machine-definition proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedDependencyMirTypeRefV1 {
    brand: MirTypeSelectionBrand,
    index: u32,
}

pub(super) struct SelectedMirTypeEntryV1<'a> {
    pub relation: MirTypeBridgeDependencyV1,
    pub terminal: &'a CrossConeMirTypeBridgeSectionV1<'a>,
}

/// Only complete local-source and terminal-provider closure construction can
/// create this set. Wire DTOs and arbitrary constituent indexes cannot.
pub struct SelectedDependencyMirTypeSetV1<'a> {
    consumer: ConeIdentity,
    brand: MirTypeSelectionBrand,
    entries: Vec<SelectedMirTypeEntryV1<'a>>,
}
impl<'a> SelectedDependencyMirTypeSetV1<'a> {
    pub(super) fn from_closed<E>(
        consumer: ConeIdentity,
        entries: Vec<SelectedMirTypeEntryV1<'a>>,
    ) -> Result<Self, MirTypeBridgeSectionError<E>> {
        if u32::try_from(entries.len()).is_err() {
            return Err(MirTypeBridgeSectionError::ArithmeticOverflow);
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let brand = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map(MirTypeSelectionBrand)
            .map_err(|_| MirTypeBridgeSectionError::SelectionIdentityExhausted)?;
        Ok(Self {
            consumer,
            brand,
            entries,
        })
    }
    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn relations(&self) -> impl ExactSizeIterator<Item = MirTypeBridgeDependencyV1> + '_ {
        self.entries.iter().map(|entry| entry.relation)
    }
    pub fn reference(
        &self,
        provider: ConeIdentity,
        target: MirTypeBridgeTargetV1,
    ) -> Option<SelectedDependencyMirTypeRefV1> {
        let relation = MirTypeBridgeDependencyV1::new(provider, target);
        self.entries
            .binary_search_by_key(&relation, |entry| entry.relation)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(|index| SelectedDependencyMirTypeRefV1 {
                brand: self.brand,
                index,
            })
    }
    pub fn relation(
        &self,
        reference: SelectedDependencyMirTypeRefV1,
    ) -> Option<MirTypeBridgeDependencyV1> {
        self.entry(reference).map(|entry| entry.relation)
    }
    pub fn resolve(
        &self,
        reference: SelectedDependencyMirTypeRefV1,
    ) -> Option<MirTypeBridgeSemanticRecordV1<'a>> {
        let entry = self.entry(reference)?;
        entry.terminal.record(entry.relation.target())
    }
    fn entry(
        &self,
        reference: SelectedDependencyMirTypeRefV1,
    ) -> Option<&SelectedMirTypeEntryV1<'a>> {
        if reference.brand != self.brand {
            return None;
        }
        self.entries.get(reference.index as usize)
    }
}
impl WireEncode for SelectedDependencyMirTypeSetV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.entries.len() as u64)?;
        for entry in &self.entries {
            entry.relation.encode(encoder)?;
        }
        Ok(())
    }
}
