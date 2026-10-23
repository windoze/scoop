use super::*;

impl Lowerer {
    pub(in crate::editing) fn editing_source_constructors(&self) -> Vec<(C, usize)> {
        let nominals = self
            .nominal_identities
            .as_ref()
            .expect("constructor declarations have nominal identities");
        let classes = self
            .class_constructors
            .iter()
            .filter(|(_, declaration)| {
                declaration.identity_kind == hir::ClassConstructorIdentityKind::Source
                    && nominals[declaration.owner].source().is_some()
                    && self.editing_owner_available(crate::Owner::Class(declaration.owner))
            })
            .map(|(constructor, declaration)| {
                (
                    C::Constructor(Constructor::CurrentClass(constructor)),
                    declaration.parameters.len(),
                )
            });
        let structs = self
            .struct_constructors
            .iter()
            .filter(|(_, declaration)| {
                nominals[declaration.owner].source().is_some()
                    && self.editing_owner_available(crate::Owner::Struct(declaration.owner))
            })
            .map(|(constructor, declaration)| {
                (
                    C::Constructor(Constructor::CurrentStruct(constructor)),
                    declaration.parameters.len(),
                )
            });
        let variants = self
            .enums
            .iter()
            .filter(|(enumeration, _)| {
                nominals[*enumeration].source().is_some()
                    && self.editing_owner_available(crate::Owner::Enum(*enumeration))
            })
            .flat_map(|(enumeration, declaration)| {
                declaration
                    .variants
                    .iter()
                    .enumerate()
                    .map(move |(index, variant)| {
                        let source =
                            hir::EnumVariantRef::checked(&self.enums, enumeration, index as u32)
                                .expect("a source variant belongs to its enum");
                        (C::CurrentVariant(source), variant.fields.len())
                    })
            });
        classes.chain(structs).chain(variants).collect()
    }
}
