use super::*;

impl Lowerer {
    pub(crate) fn class_is_same_or_subclass_of(
        &self,
        class: hir::ClassId,
        base: hir::ClassId,
    ) -> bool {
        self.class_inherits_declaration(
            class,
            self.nominal_identity(Owner::Class(base)).declaration_id(),
        )
    }

    pub(super) fn class_inherits_declaration(
        &self,
        class: hir::ClassId,
        base: hir::SourceNominalId,
    ) -> bool {
        self.class_declaration_is_same_or_subclass_of(
            self.nominal_identity(Owner::Class(class)).declaration_id(),
            base,
        )
    }

    pub(super) fn class_declaration_is_same_or_subclass_of(
        &self,
        derived: hir::SourceNominalId,
        base: hir::SourceNominalId,
    ) -> bool {
        let mut current = Some(derived);
        let mut seen = Vec::new();
        while let Some(owner) = current {
            if owner == base {
                return true;
            }
            if seen.contains(&owner) {
                return false;
            }
            seen.push(owner);
            current = self.class_parent_declaration(owner);
        }
        false
    }

    fn class_parent_declaration(
        &self,
        owner: hir::SourceNominalId,
    ) -> Option<hir::SourceNominalId> {
        if let Some(owner) = self.nominal_owners.get(&owner) {
            let Owner::Class(class) = *owner else {
                unreachable!("class ancestry refers to class declarations")
            };
            return self.classes[class]
                .base_class
                .map(|ty| match &self.types[ty] {
                    hir::Type::Class(application) => self
                        .nominal_identity(Owner::Class(
                            self.class_applications[*application].template,
                        ))
                        .declaration_id(),
                    hir::Type::ImportedClass(class) => class.declaration.owner(),
                    _ => unreachable!("resolved class bases are class applications"),
                });
        }
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("class ancestry retains its dependency declarations");
        dependencies
            .nominal_declaration(owner)
            .expect("class ancestry retains each declaration")
            .interface
            .exact_supertypes()
            .values()
            .iter()
            .find_map(|parent| {
                let owner = match parent {
                    scoop_identity::SignatureTypeKey::Nominal(id) => {
                        hir::SourceNominalId::Concrete(*id)
                    }
                    scoop_identity::SignatureTypeKey::NominalApplication { origin, .. } => {
                        hir::SourceNominalId::GenericTemplate(*origin)
                    }
                    _ => unreachable!("resolved nominal parents are nominal references"),
                };
                let parent = dependencies
                    .nominal_declaration(owner)
                    .expect("class parents retain their declarations");
                matches!(
                    parent.interface.source_shape(),
                    hir::NominalSourceShapeV1::Class(_)
                )
                .then_some(owner)
            })
    }
}
