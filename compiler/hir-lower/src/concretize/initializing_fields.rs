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
        declared: &concrete::DeclaredClass,
        field: scoop_identity::PersistentFieldId,
    ) -> concrete::FieldRef {
        let own_index = declared
            .fields
            .iter()
            .position(|candidate| candidate.identity == field)
            .expect("a backing field belongs to its actual declaration");
        let mut index = u32::try_from(own_index).expect("class field indices fit in u32");
        let mut base = declared.base_class;
        while let Some(parent) = base {
            let parent = match &self.classes[parent].representation {
                concrete::ClassRepresentation::Declared(parent) => parent,
                concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::Throwable,
                    ..
                } => break,
                _ => unreachable!("only Throwable has an inheritable intrinsic prefix"),
            };
            index = index
                .checked_add(
                    u32::try_from(parent.fields.len()).expect("class field indices fit in u32"),
                )
                .expect("class field indices fit in u32");
            base = parent.base_class;
        }
        concrete::FieldRef::ClassField { class_id, index }
    }
}
