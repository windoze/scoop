use super::{
    AnnotationRecord, CanonicalHirFoundation, CborIdentityRecord, HirFoundationBuildError,
    HirFoundationTable, PersistentAnnotationId, SourceDeclarationKey,
};

impl CanonicalHirFoundation {
    pub fn set_annotations(
        &mut self,
        records: Vec<AnnotationRecord>,
    ) -> Result<(), HirFoundationBuildError> {
        self.annotations = super::sort_unique(
            records,
            HirFoundationTable::Annotation,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn annotation(&self, id: PersistentAnnotationId) -> Option<&AnnotationRecord> {
        self.annotations
            .binary_search_by_key(&id, CborIdentityRecord::id)
            .ok()
            .map(|index| &self.annotations[index])
    }

    pub(crate) fn annotation_by_bytes(
        &self,
        bytes: &[u8; 32],
    ) -> Option<(PersistentAnnotationId, &SourceDeclarationKey)> {
        self.annotations.iter().find_map(|record| {
            (record.id().as_array() == bytes).then_some((record.id(), record.key()))
        })
    }
}
