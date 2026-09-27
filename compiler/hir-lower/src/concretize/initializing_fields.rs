use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_initializing_class_field(
        &mut self,
        source: export::InitializingClassFieldRef,
        substitution: &[concrete::TypeId],
    ) -> (concrete::TypeId, concrete::FieldRef) {
        let receiver = self.lower_type(
            source.owner_type(&self.source.class_applications),
            substitution,
        );
        let field = match source {
            export::InitializingClassFieldRef::Declared { application, field } => self
                .lower_field_ref(
                    export::FieldRef::ClassField { application, field },
                    substitution,
                ),
            export::InitializingClassFieldRef::Imported { field, .. } => {
                let concrete::TypeKind::Class(class_id) = self.types[receiver].kind else {
                    unreachable!("a dependency initializer field retains its class owner")
                };
                let own_index = self.classes[class_id]
                    .declared_fields()
                    .iter()
                    .position(|candidate| candidate.identity == field)
                    .expect("a dependency backing field belongs to its actual declaration");
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
        };
        (receiver, field)
    }
}
