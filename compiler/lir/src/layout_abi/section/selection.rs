use super::*;

pub(super) struct SelectedLayoutAbiEntryV1<'a> {
    pub relation: LayoutAbiDependencyV1,
    pub terminal: &'a LayoutAbiExportConstituentsV1,
}

pub struct SelectedDependencyLayoutAbiSetV1<'a> {
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    semantic: Vec<SelectedLayoutAbiEntryV1<'a>>,
    physical: crate::CanonicalExternalShapeLinkImportsV1,
}

impl<'a> SelectedDependencyLayoutAbiSetV1<'a> {
    pub(super) fn from_closed(
        consumer: ConeIdentity,
        target: crate::LirTargetProfile,
        semantic: Vec<SelectedLayoutAbiEntryV1<'a>>,
        physical: crate::CanonicalExternalShapeLinkImportsV1,
    ) -> Self {
        Self {
            consumer,
            target,
            semantic,
            physical,
        }
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub const fn target_profile(&self) -> crate::LirTargetProfile {
        self.target
    }

    pub fn len(&self) -> usize {
        self.semantic.len()
    }

    pub fn is_empty(&self) -> bool {
        self.semantic.is_empty()
    }

    pub fn semantic_relations(&self) -> impl ExactSizeIterator<Item = LayoutAbiDependencyV1> + '_ {
        self.semantic.iter().map(|entry| entry.relation)
    }

    pub const fn physical_imports(&self) -> &crate::CanonicalExternalShapeLinkImportsV1 {
        &self.physical
    }

    pub fn record(
        &self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>> {
        let relation = LayoutAbiDependencyV1::new(provider, target);
        let index = self
            .semantic
            .binary_search_by_key(&relation, |entry| entry.relation)
            .ok()?;
        self.semantic[index].terminal.record(target)
    }
}

impl WireEncode for SelectedDependencyLayoutAbiSetV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.semantic.len() as u64)?;
        for entry in &self.semantic {
            entry.relation.encode(encoder)?;
        }
        encoder.field(2)?;
        self.physical.encode(encoder)
    }
}
