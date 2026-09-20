use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LayoutAbiSelectionBrand(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedDependencyLayoutAbiRefV1 {
    brand: LayoutAbiSelectionBrand,
    index: u32,
}

pub(super) struct SelectedLayoutAbiEntryV1<'a> {
    pub relation: LayoutAbiDependencyV1,
    pub terminal: &'a CrossConeLayoutAbiSectionV1<'a>,
}

pub struct SelectedDependencyLayoutAbiSetV1<'a> {
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    brand: LayoutAbiSelectionBrand,
    semantic: Vec<SelectedLayoutAbiEntryV1<'a>>,
    physical: crate::CanonicalExternalShapeLinkImportsV1<'a>,
}

impl<'a> SelectedDependencyLayoutAbiSetV1<'a> {
    pub(super) fn from_closed<E>(
        consumer: ConeIdentity,
        target: crate::LirTargetProfile,
        semantic: Vec<SelectedLayoutAbiEntryV1<'a>>,
        physical: crate::CanonicalExternalShapeLinkImportsV1<'a>,
    ) -> Result<Self, LayoutAbiSectionError<E>> {
        if u32::try_from(semantic.len()).is_err() {
            return Err(LayoutAbiSectionError::Semantic(
                LayoutAbiSemanticClosureError::ArithmeticOverflow,
            ));
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let brand = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map(LayoutAbiSelectionBrand)
            .map_err(|_| LayoutAbiSectionError::SelectionIdentityExhausted)?;
        Ok(Self {
            consumer,
            target,
            brand,
            semantic,
            physical,
        })
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

    pub const fn physical_imports(&self) -> &crate::CanonicalExternalShapeLinkImportsV1<'a> {
        &self.physical
    }

    pub fn reference(
        &self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<SelectedDependencyLayoutAbiRefV1> {
        let relation = LayoutAbiDependencyV1::new(provider, target);
        self.semantic
            .binary_search_by_key(&relation, |entry| entry.relation)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(|index| SelectedDependencyLayoutAbiRefV1 {
                brand: self.brand,
                index,
            })
    }

    pub fn relation(
        &self,
        reference: SelectedDependencyLayoutAbiRefV1,
    ) -> Option<LayoutAbiDependencyV1> {
        self.entry(reference).map(|entry| entry.relation)
    }

    pub fn resolve(
        &self,
        reference: SelectedDependencyLayoutAbiRefV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>> {
        let entry = self.entry(reference)?;
        entry.terminal.record(entry.relation.target())
    }

    fn entry(
        &self,
        reference: SelectedDependencyLayoutAbiRefV1,
    ) -> Option<&SelectedLayoutAbiEntryV1<'a>> {
        (reference.brand == self.brand)
            .then(|| self.semantic.get(reference.index as usize))
            .flatten()
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
