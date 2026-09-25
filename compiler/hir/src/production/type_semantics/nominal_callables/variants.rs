use super::*;

impl Projection<'_> {
    pub(super) fn variants(&mut self) -> Result<(), Error> {
        for (id, enumeration) in self.export.enums.iter() {
            for (index, variant) in enumeration.variants.iter().enumerate() {
                let index = u32::try_from(index).map_err(invalid)?;
                let reference = EnumVariantRef::checked(&self.export.enums, id, index)
                    .ok_or_else(|| invalid("nominal source variant has no declaration"))?;
                let identity = self
                    .export
                    .enum_member_identities
                    .get_variant(reference)
                    .ok_or_else(|| invalid("nominal source variant has no typed identity"))?;
                let declaration = CallableTemplateOrigin::VariantConstructor(identity.id());
                if !self.take(declaration) {
                    continue;
                }
                let source = self
                    .export
                    .nominal_identities
                    .get_enum(id)
                    .and_then(HirNominalIdentity::source)
                    .ok_or_else(|| invalid("nominal source variant has no source enum owner"))?;
                let owner = match source {
                    HirSourceNominalIdentity::Concrete(record) => {
                        SourceNominalId::Concrete(record.id())
                    }
                    HirSourceNominalIdentity::Generic(record) => {
                        SourceNominalId::GenericTemplate(record.id())
                    }
                };
                if source.declaration().origin() != self.export.cone
                    || identity.key().source_owner() != Some(owner)
                {
                    return Err(invalid("nominal source variant belongs to another owner"));
                }
                let path = WirePath::root();
                let parameters = &enumeration.type_params;

                let binders = self
                    .signatures
                    .binder_frame(parameters, 0)
                    .map_err(invalid)?;
                let mut expected = Vec::new();

                scoop_wire::allocation::try_reserve(&mut expected, variant.fields.len(), &path)
                    .map_err(resource)?;
                for field in &variant.fields {
                    expected.push(
                        self.signatures
                            .map_type(field.ty, &binders)
                            .map_err(invalid)?,
                    );
                }
                let parameters = self.parameters(
                    ExportParameterOwner::VariantConstructor(reference),
                    &binders,
                    &expected,
                )?;
                let result_type =
                    self.export.enum_applications[enumeration.self_application].canonical_type;

                let result = self
                    .signatures
                    .map_type(result_type, &binders)
                    .map_err(invalid)?;

                let mut owners = super::super::nominals::lexical_owners(source.declaration())?;
                scoop_wire::allocation::try_reserve(&mut owners, 1, &path).map_err(resource)?;
                owners.push(owner);
                let origin = self.origin(DefinitionOriginSubject::EnumVariant(identity.id()))?;
                let access = DeclarationAccessSourceV1::try_new(
                    DeclaredVisibilityV1::Public,
                    owners,
                    origin,
                )
                .map_err(invalid)?;
                let payload = NominalSourceCallablePayloadV1::try_new(
                    declaration,
                    owner,
                    CanonicalBinderListV1::try_new(Vec::new()).map_err(invalid)?,
                    parameters,
                    result,
                    callable_interfaces::source_constructor_effects(
                        Safety::Safe,
                        GcEffect::Managed,
                    )
                    .map_err(invalid)?,
                    CallableModalityV1::Final,
                    CanonicalProtectedSlotRefsV1::try_new(Vec::new()).map_err(invalid)?,
                )
                .map_err(invalid)?;
                self.push(declaration, access, payload)?;
            }
        }
        Ok(())
    }
}
