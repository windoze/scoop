use super::{
    AnnotationRecord, CanonicalHirFoundation, CborIdentityRecord, HirFoundationBuildError,
    HirFoundationTable, PersistentAnnotationId, SourceDeclarationKey,
};

impl CanonicalHirFoundation {
    /// Private aliases and lexical callables have source keys without a public lookup entry.
    pub fn retains_local_annotation_target(&self, target: crate::AnnotationTargetV1) -> bool {
        use crate::{AnnotationCallableV1 as Callable, AnnotationTargetV1 as Target};
        match target {
            Target::TypeAlias(id) => self
                .type_aliases
                .binary_search_by_key(&id, CborIdentityRecord::id)
                .is_ok(),
            Target::Callable(Callable::Local(id))
            | Target::Parameter {
                callable: Callable::Local(id),
                ..
            }
            | Target::CallableReceiver(Callable::Local(id)) => self
                .generated_callables
                .binary_search_by_key(&id, CborIdentityRecord::id)
                .ok()
                .is_some_and(|index| {
                    matches!(
                        self.generated_callables[index].key(),
                        scoop_identity::GeneratedCallableKey::Lexical { .. }
                    )
                }),
            _ => false,
        }
    }

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
