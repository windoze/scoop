use super::*;

impl Lowerer {
    pub(super) fn query_enum_variants(
        &mut self,
        application: hir::EnumApplicationId,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
        dependency: Option<&hir::ImportedNominalDeclaration>,
    ) -> Result<Vec<hir::SemanticVariantDescription>, Diagnostic> {
        let template = self.enum_applications[application].template;
        let variants = self.enum_definition(template).variants.clone();
        variants
            .into_iter()
            .enumerate()
            .map(|(index, variant)| {
                let index = u32::try_from(index).expect("source variant arity fits u32");
                let reference = self.enum_variant_at(application, index);
                let (annotations, origin, source_variant) = if let Some(dependency) = dependency {
                    (
                        hir::SemanticAnnotationTarget::Imported(hir::AnnotationTargetV1::Variant(
                            reference.variant,
                        )),
                        imported_location(
                            dependency.variant_sources[index as usize].origin.origin(),
                        )?,
                        None,
                    )
                } else {
                    let enumeration = self.enum_id(template);
                    let variant = hir::EnumVariantRef::checked(&self.enums, enumeration, index)
                        .expect("the variant comes from its actual declaration");
                    (
                        hir::SemanticAnnotationTarget::Current(
                            hir::SourceAnnotationTarget::Variant(variant),
                        ),
                        self.query_member_location(
                            Owner::Enum(enumeration),
                            self.enum_variant_spans[&variant],
                        ),
                        Some(variant),
                    )
                };
                let fields = variant
                    .fields
                    .into_iter()
                    .enumerate()
                    .map(|(field_index, field)| {
                        let field_index = u32::try_from(field_index)
                            .expect("source variant field arity fits u32");
                        let field_reference = self.enum_variant_field_at(reference, field_index);
                        let (annotations, origin) = if let Some(dependency) = dependency {
                            let source = &dependency.variant_sources[index as usize].fields
                                [field_index as usize];
                            (
                                hir::SemanticAnnotationTarget::Imported(
                                    hir::AnnotationTargetV1::VariantField(field_reference.field),
                                ),
                                imported_location(source.origin.origin())?,
                            )
                        } else {
                            let variant = source_variant
                                .expect("a current variant has its original declaration");
                            let reference = hir::EnumVariantFieldRef::checked(
                                &self.enums,
                                variant,
                                field_index,
                            )
                            .expect("the field comes from its actual variant");
                            (
                                hir::SemanticAnnotationTarget::Current(
                                    hir::SourceAnnotationTarget::VariantField(reference),
                                ),
                                self.query_member_location(
                                    Owner::Enum(self.enum_id(template)),
                                    self.enum_variant_field_spans[&reference],
                                ),
                            )
                        };
                        Ok(hir::SemanticFieldDescription {
                            identity: hir::StaticFieldIdentity::VariantField(field_reference.field),
                            name: field.name,
                            value_type: self.query_type_use(field.ty, bindings),
                            storage: hir::NominalFieldStorage::Declared,
                            annotations,
                            origin,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                Ok(hir::SemanticVariantDescription {
                    identity: reference.variant,
                    name: variant.name,
                    style: variant.style,
                    fields,
                    annotations,
                    origin,
                })
            })
            .collect()
    }
}
