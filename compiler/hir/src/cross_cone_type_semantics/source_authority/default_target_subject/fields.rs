use super::*;

impl Query<'_, '_, '_> {
    pub(super) fn field(
        &mut self,
        target: Target,
        id: PersistentFieldId,
    ) -> Result<Subject, Error> {
        let canonical = self.foundation.foundation.as_canonical();
        let key = self.key(canonical.type_source_field_records(), id, || {
            Error::MissingTarget(target)
        })?;
        let (owner, property) = match (target, key.view()) {
            (Target::StructField(_), FieldIdentityView::SourceDeclared { owner, .. }) => {
                return self.nominal(owner, SourceDeclarationKind::Struct);
            }
            (
                Target::ClassField(_),
                FieldIdentityView::SourcePropertyBacking { owner, property }
                | FieldIdentityView::SourcePropertyDelegate { owner, property },
            ) => {
                self.nominal(owner, SourceDeclarationKind::Class)?;
                (owner, property)
            }
            (Target::ClassField(_), FieldIdentityView::Generated { owner, key }) => {
                let property = key.object_backing_property().ok_or(Error::Role(target))?;
                let backing = self.key(canonical.type_source_generated_records(), owner, || {
                    Error::MissingGenerated(owner)
                })?;
                let GeneratedNominalKey::ObjectBackingClass { object } = backing else {
                    return Err(Error::Role(target));
                };
                let owner = SourceNominalId::Concrete(*object);
                self.nominal(owner, SourceDeclarationKind::Object)?;
                (owner, property)
            }
            _ => return Err(Error::Role(target)),
        };
        let subject = Subject::Property(property);
        let key = self.declaration(subject)?;
        let expected = match owner {
            SourceNominalId::Concrete(id) => DefinitionOwnerAtom::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
        };
        if key.owners().owners().last() != Some(&expected) {
            return Err(Error::PropertyOwner {
                field: id,
                property,
            });
        }
        Ok(subject)
    }
}
