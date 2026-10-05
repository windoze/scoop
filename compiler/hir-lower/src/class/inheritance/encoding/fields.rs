use super::*;

impl Lowerer {
    pub(super) fn encode_struct(
        &mut self,
        structure: hir::StructId,
        encodable: TypeId,
        span: Span,
    ) -> Option<ast::Block> {
        if matches!(
            self.structs[structure].representation,
            hir::StructRepresentation::Intrinsic(_)
        ) {
            self.error(
                span,
                "an intrinsic type requires an explicit encode implementation".into(),
            );
            return None;
        }
        let fields = self.structs[structure].semantic_fields().to_vec();
        let mut values = Vec::new();
        let mut names = std::collections::HashSet::new();
        for (index, field) in fields.into_iter().enumerate() {
            let source = hir::StructFieldRef::checked(&self.structs, structure, index as u32)
                .expect("the shape iterates this declaration's fields");
            let span = self.struct_field_spans[&source];
            let Some(name) =
                self.encoding_wire_name(hir::SourceAnnotationTarget::Field(source), &field.name)
            else {
                continue;
            };
            self.check_encoding_field(&name, field.ty, encodable, span, &mut names);
            values.push((name, syntax::field(&field.name, span), span));
        }
        Some(syntax::record(
            values,
            syntax::variable("encoder", span),
            span,
        ))
    }

    pub(super) fn encode_class(
        &mut self,
        class: ClassId,
        encodable: TypeId,
        span: Span,
    ) -> Option<ast::Block> {
        let declaration = &self.classes[class];
        if declaration.modifier != hir::ClassModifier::Final
            || declaration.base_class.is_some()
            || !matches!(
                declaration.representation,
                hir::ClassRepresentation::Declared
            )
        {
            self.error(span, "automatic encode requires a final class without a class base; provide an explicit encode implementation".into());
            return None;
        }
        let properties = declaration.properties.clone();
        let mut values = Vec::new();
        let mut names = std::collections::HashSet::new();
        for id in properties {
            let property = self.properties[id].clone();
            let stored = match &property.representation {
                hir::PropertyRepresentation::AccessorOnly
                | hir::PropertyRepresentation::Const { .. } => continue,
                hir::PropertyRepresentation::Stored(_) => true,
                hir::PropertyRepresentation::Delegated { .. }
                | hir::PropertyRepresentation::GenericDelegated { .. }
                | hir::PropertyRepresentation::NativeStorage { .. } => {
                    self.error(
                        property.span,
                        format!(
                            "property `{}` requires an explicit encode implementation",
                            property.name
                        ),
                    );
                    false
                }
            };
            if !stored {
                continue;
            }
            let Some(name) =
                self.encoding_wire_name(hir::SourceAnnotationTarget::Property(id), &property.name)
            else {
                continue;
            };
            // Public storage may use an ordinary generated accessor body.
            // Its source classification distinguishes it from custom code.
            if self.property_accessor_sources.iter().any(|source| {
                source.property == id
                    && source.body_kind == crate::properties::PropertyAccessorBodyKind::Source
            }) {
                self.error(property.span, format!("stored property `{}` has a custom accessor; provide an explicit encode implementation or mark it Transient", property.name));
                continue;
            }
            self.check_encoding_field(&name, property.ty, encodable, property.span, &mut names);
            values.push((
                name,
                syntax::field(&property.name, property.span),
                property.span,
            ));
        }
        Some(syntax::record(
            values,
            syntax::variable("encoder", span),
            span,
        ))
    }

    pub(super) fn encoding_wire_name(
        &self,
        target: hir::SourceAnnotationTarget,
        source_name: &str,
    ) -> Option<String> {
        let applications = self
            .annotation_metadata
            .targets
            .iter()
            .find(|value| value.target == target)
            .map_or(&[][..], |value| value.annotations.as_slice());
        let transient = self.core_source_annotation("Transient");
        if applications
            .iter()
            .any(|value| Some(value.annotation) == transient)
        {
            return None;
        }
        let serial_name = self.core_source_annotation("SerialName");
        applications
            .iter()
            .find(|value| Some(value.annotation) == serial_name)
            .map_or_else(
                || Some(source_name.to_owned()),
                |value| match &value.arguments[0] {
                    hir::CanonicalConstValueV1::String(name) => Some(name.clone()),
                    _ => unreachable!("SerialName has a checked String argument"),
                },
            )
    }

    pub(super) fn check_encoding_field(
        &mut self,
        name: &str,
        ty: TypeId,
        encodable: TypeId,
        span: Span,
        names: &mut std::collections::HashSet<String>,
    ) {
        if !names.insert(name.to_owned()) {
            self.error(
                span,
                format!("automatic encode has duplicate field name `{name}`"),
            );
        }
        if !self.is_subtype(ty, encodable) {
            self.error(
                span,
                format!(
                    "automatic encode requires field `{name}` of type {} to implement Encodable",
                    self.type_name(ty)
                ),
            );
        }
    }
}
