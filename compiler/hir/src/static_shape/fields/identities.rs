use super::*;

impl StaticNominalShape<'_> {
    pub(super) fn field_identity(
        self,
        index: usize,
    ) -> (
        PersistentFieldId,
        NominalFieldStorage,
        StaticAnnotationTarget,
    ) {
        match self.origin {
            StaticNominalOrigin::Current(NominalOwner::Struct(structure)) => {
                let field = StructFieldRef::checked(&self.module.structs, structure, index as u32)
                    .expect("the view iterates this declaration's fields");
                (
                    self.module.field_identities[field].id(),
                    NominalFieldStorage::Declared,
                    StaticAnnotationTarget::Current(SourceAnnotationTarget::Field(field)),
                )
            }
            StaticNominalOrigin::Current(NominalOwner::Class(class)) => {
                self.class_field_identity(class, index)
            }
            StaticNominalOrigin::Current(NominalOwner::Object(object)) => {
                self.class_field_identity(self.module.objects[object].backing_class, index)
            }
            StaticNominalOrigin::Current(NominalOwner::Enum(_) | NominalOwner::Interface(_)) => {
                unreachable!("enum payloads and interface properties are separate views")
            }
            StaticNominalOrigin::Dependency(owner) => {
                let declaration = match self.definition {
                    nominals::Definition::Struct(_) => {
                        &self.module.loaded_struct_definitions[&owner].declaration
                    }
                    nominals::Definition::Class(_) => {
                        &self.module.loaded_class_definitions[&owner].declaration
                    }
                    _ => unreachable!("only structs and classes have directly declared fields"),
                };
                let field = declaration.interface.source_shape().declared_fields()[index].field();
                let storage = declaration.field_sources[index].storage;
                let annotation = if storage == NominalFieldStorage::Declared {
                    StaticAnnotationTarget::Dependency(AnnotationTargetV1::Field(field))
                } else {
                    StaticAnnotationTarget::Unannotated
                };
                (field, storage, annotation)
            }
        }
    }

    fn class_field_identity(
        self,
        class: ClassId,
        index: usize,
    ) -> (
        PersistentFieldId,
        NominalFieldStorage,
        StaticAnnotationTarget,
    ) {
        let field = self.module.classes[class].fields[index];
        let record = &self.module.field_identities[field];
        (
            record.id(),
            NominalFieldStorage::from_key(record.key()),
            StaticAnnotationTarget::Unannotated,
        )
    }
}

impl StaticVariantShape<'_> {
    pub fn identity(self) -> PersistentEnumVariantId {
        match self.owner.origin {
            StaticNominalOrigin::Current(NominalOwner::Enum(enumeration)) => {
                self.owner.module.enum_member_identities[self.source_variant(enumeration)].id()
            }
            StaticNominalOrigin::Dependency(owner) => {
                self.owner.module.loaded_enum_definitions[&owner].variant_identity(self.index)
            }
            _ => unreachable!("a variant belongs to an enum"),
        }
    }

    pub(super) fn source_variant(self, enumeration: EnumId) -> EnumVariantRef {
        EnumVariantRef::checked(&self.owner.module.enums, enumeration, self.index as u32)
            .expect("the view iterates this declaration's variants")
    }

    pub(super) fn field_identity(self, index: usize) -> PersistentEnumVariantFieldId {
        match self.owner.origin {
            StaticNominalOrigin::Current(NominalOwner::Enum(enumeration)) => {
                let field = EnumVariantFieldRef::checked(
                    &self.owner.module.enums,
                    self.source_variant(enumeration),
                    index as u32,
                )
                .expect("the view iterates this variant's fields");
                self.owner.module.enum_member_identities[field].id()
            }
            StaticNominalOrigin::Dependency(owner) => {
                self.owner.module.loaded_enum_definitions[&owner].field_identity(self.index, index)
            }
            _ => unreachable!("a variant belongs to an enum"),
        }
    }

    pub(super) fn field_annotation(
        self,
        index: usize,
        identity: PersistentEnumVariantFieldId,
    ) -> StaticAnnotationTarget {
        match self.owner.origin {
            StaticNominalOrigin::Current(NominalOwner::Enum(enumeration)) => {
                let field = EnumVariantFieldRef::checked(
                    &self.owner.module.enums,
                    self.source_variant(enumeration),
                    index as u32,
                )
                .expect("the view iterates this variant's fields");
                StaticAnnotationTarget::Current(SourceAnnotationTarget::VariantField(field))
            }
            StaticNominalOrigin::Dependency(_) => {
                StaticAnnotationTarget::Dependency(AnnotationTargetV1::VariantField(identity))
            }
            _ => unreachable!("a variant belongs to an enum"),
        }
    }
}
