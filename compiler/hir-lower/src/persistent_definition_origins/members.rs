use scoop_identity::DefinitionOriginSubject;

use super::*;

impl DefinitionOriginBuilder<'_> {
    pub(super) fn collect_properties(
        &mut self,
        identities: &hir::HirPropertyIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.properties.iter() {
            let subject = match &identities[id] {
                hir::HirPropertyIdentity::Ordinary(record) => {
                    DefinitionOriginSubject::Property(record.id())
                }
                hir::HirPropertyIdentity::Extension(record) => {
                    DefinitionOriginSubject::ExtensionProperty(record.id())
                }
            };
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.property_source_file(id),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_accessors(
        &mut self,
        identities: &hir::HirPropertyAccessorIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.property_getters.iter() {
            let identity = &identities[id];
            let subject = DefinitionOriginSubject::PropertyAccessor(identity.id());
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.property_source_file(identity.property()),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        for (id, declaration) in self.lowerer.property_setters.iter() {
            let identity = &identities[id];
            let subject = DefinitionOriginSubject::PropertyAccessor(identity.id());
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.property_source_file(identity.property()),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_aliases(
        &mut self,
        identities: &hir::HirTypeAliasIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.type_aliases.iter() {
            let subject = DefinitionOriginSubject::TypeAlias(identities[id].id());
            let file = usize::try_from(declaration.origin.file).map_err(|_| {
                PersistentDefinitionOriginError::new(
                    0,
                    declaration.origin.span,
                    PersistentDefinitionOriginErrorDetail::UnknownSourceFile {
                        subject,
                        file: usize::MAX,
                    },
                )
            })?;
            self.append(subject, file, declaration.origin.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_fields(
        &mut self,
        identities: &hir::HirFieldIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (structure, declaration) in self.lowerer.structs.iter() {
            let file = self.lowerer.struct_files.get(&structure).copied();
            for index in 0..declaration.semantic_fields().len() {
                let index = u32::try_from(index)
                    .expect("persistent struct-field identities already enforce u32 arity");
                let field = hir::StructFieldRef::checked(&self.lowerer.structs, structure, index)
                    .expect("a source struct field belongs to its checked declaration");
                let subject = DefinitionOriginSubject::Field(identities[field].id());
                let span = self.member_span(
                    subject,
                    declaration.span,
                    self.lowerer.struct_field_spans.get(&field).copied(),
                    file,
                )?;
                self.append(subject, span.0, span.1)?;
            }
        }
        for (field, declaration) in self.lowerer.class_fields.iter() {
            let subject = DefinitionOriginSubject::Field(identities[field].id());
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.class_files.get(&declaration.owner).copied(),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_enum_members(
        &mut self,
        identities: &hir::HirEnumMemberIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (enumeration, declaration) in self.lowerer.enums.iter() {
            let file = self.lowerer.enum_files.get(&enumeration).copied();
            for variant_index in 0..declaration.variants.len() {
                let variant_index = u32::try_from(variant_index)
                    .expect("persistent enum identities already enforce u32 arity");
                let variant =
                    hir::EnumVariantRef::checked(&self.lowerer.enums, enumeration, variant_index)
                        .expect("a source enum variant belongs to its checked declaration");
                let subject = DefinitionOriginSubject::EnumVariant(identities[variant].id());
                let (file, span) = self.member_span(
                    subject,
                    declaration.span,
                    self.lowerer.enum_variant_spans.get(&variant).copied(),
                    file,
                )?;
                self.append(subject, file, span)?;

                let fields = &declaration.variants[variant_index as usize].fields;
                for field_index in 0..fields.len() {
                    let field_index = u32::try_from(field_index)
                        .expect("persistent enum-field identities already enforce u32 arity");
                    let field = hir::EnumVariantFieldRef::checked(
                        &self.lowerer.enums,
                        variant,
                        field_index,
                    )
                    .expect("a source enum field belongs to its checked variant");
                    let subject = DefinitionOriginSubject::EnumVariantField(identities[field].id());
                    let (file, span) = self.member_span(
                        subject,
                        span,
                        self.lowerer.enum_variant_field_spans.get(&field).copied(),
                        Some(file),
                    )?;
                    self.append(subject, file, span)?;
                }
            }
        }
        Ok(())
    }

    fn member_span(
        &self,
        subject: DefinitionOriginSubject,
        fallback_span: scoop_ast::Span,
        span: Option<scoop_ast::Span>,
        file: Option<usize>,
    ) -> Result<(usize, scoop_ast::Span), PersistentDefinitionOriginError> {
        let file = self.source_file(subject, fallback_span, file)?;
        let span = span.ok_or_else(|| {
            PersistentDefinitionOriginError::new(
                file,
                fallback_span,
                PersistentDefinitionOriginErrorDetail::MissingMemberSpan { subject },
            )
        })?;
        Ok((file, span))
    }
}
