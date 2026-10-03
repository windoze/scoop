use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_initializing_class_field(
        &mut self,
        source: export::InitializingClassFieldRef,
        substitution: &[concrete::TypeId],
    ) -> (concrete::TypeId, concrete::FieldRef) {
        let receiver = self.lower_type(source.owner, substitution);
        let field = self.lower_field_ref(
            export::FieldRef::ClassField {
                owner: source.owner,
                field: source.field,
            },
            substitution,
        );
        (receiver, field)
    }
    pub(super) fn class_field_ref(
        &self,
        class_id: concrete::ClassId,
        field: scoop_identity::PersistentFieldId,
    ) -> concrete::FieldRef {
        let own_index = self.classes[class_id]
            .declared_fields()
            .iter()
            .position(|candidate| candidate.identity == field)
            .expect("a backing field belongs to its actual declaration");
        let mut index = u32::try_from(own_index).expect("class field indices fit in u32");
        let mut base = self.classes[class_id].base_class();
        while let Some(parent) = base {
            index = index
                .checked_add(
                    u32::try_from(self.classes[parent].declared_fields().len())
                        .expect("class field indices fit in u32"),
                )
                .expect("class field indices fit in u32");
            base = self.classes[parent].base_class();
        }
        concrete::FieldRef::ClassField { class_id, index }
    }
}
