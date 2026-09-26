use super::*;

pub(super) struct SelectedMirTypeEntryV1<'a> {
    pub relation: MirTypeBridgeDependencyV1,
    pub terminal: MirTypeBridgeDependencyViewV1<'a>,
}

/// Selected dependency records and their actual providers.
pub struct SelectedDependencyMirTypeSetV1<'a> {
    consumer: ConeIdentity,
    entries: Vec<SelectedMirTypeEntryV1<'a>>,
}
impl<'a> SelectedDependencyMirTypeSetV1<'a> {
    pub(super) fn from_closed(
        consumer: ConeIdentity,
        entries: Vec<SelectedMirTypeEntryV1<'a>>,
    ) -> Self {
        Self { consumer, entries }
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
    pub fn record(
        &self,
        provider: ConeIdentity,
        target: MirTypeBridgeTargetV1,
    ) -> Option<MirTypeBridgeSemanticRecordV1<'a>> {
        let relation = MirTypeBridgeDependencyV1::new(provider, target);
        let index = self
            .entries
            .binary_search_by_key(&relation, |entry| entry.relation)
            .ok()?;
        self.entries[index].terminal.record(target)
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
