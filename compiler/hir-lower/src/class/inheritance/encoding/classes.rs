use super::*;
use crate::NominalTarget;

impl Lowerer {
    pub(super) fn encode_class(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        encoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let application = self
            .nominal_application(context.target)
            .expect("a class has a nominal application");
        let declaration = self.class_definition(application.template);
        let singleton = matches!(
            self.nominal_target_for_type(context.target),
            Some(NominalTarget::Object(_))
        ) || self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(application.template))
            .is_some_and(|declaration| {
                matches!(
                    declaration.interface.source_shape(),
                    hir::NominalSourceShapeV1::Object(_)
                )
            });
        if singleton
            || declaration.modifier != hir::ClassModifier::Final
            || declaration.base_class.is_some()
            || !matches!(
                declaration.representation,
                hir::ClassRepresentation::Declared
            )
        {
            self.error(span, "automatic encode requires a struct, enum, tuple, or final class without a class base; provide an explicit implementation for other targets".into());
            return None;
        }
        let fields = match self.source_class_id(application.template) {
            Some(class) => self.current_encoding_properties(class, span)?,
            None => self.dependency_encoding_properties(application.template, span)?,
        };
        let container = self.encoding_container(encoder, true, span, sink)?;
        let mut names = std::collections::HashSet::new();
        for (name, wire) in fields {
            self.check_encoding_wire_name(&wire, "field", span, &mut names)?;
            let Some(field) = self.coding_property(value.clone(), &name, span) else {
                self.error(
                    span,
                    format!("automatic encode cannot read property `{name}` from this codec"),
                );
                return None;
            };
            self.encode_field(context, field, container.clone(), Some(&wire), span, sink)?;
        }
        self.end_coding_container(container, span, sink)
    }

    fn current_encoding_properties(
        &mut self,
        class: ClassId,
        span: Span,
    ) -> Option<Vec<(String, String)>> {
        let mut selected = Vec::new();
        for field in self.classes[class].fields.clone() {
            let id = self.class_fields[field].property;
            let property = &self.properties[id];
            let name = property.name.clone();
            let Some(wire) =
                self.serialization_wire_name(hir::SourceAnnotationTarget::Property(id), &name)
            else {
                continue;
            };
            if !matches!(
                property.representation,
                hir::PropertyRepresentation::Stored(_)
            ) {
                self.error(
                    span,
                    format!(
                        "property `{name}` requires an explicit encode implementation or Transient"
                    ),
                );
                return None;
            }
            let capability = property.capability;
            if !self.property_getters[capability.getter()]
                .implementation
                .is_storage()
                || capability.setter().is_some_and(|setter| {
                    !self.property_setters[setter].implementation.is_storage()
                })
            {
                self.error(span, format!("stored property `{name}` has a custom accessor; provide an explicit encode implementation or mark it Transient"));
                return None;
            }
            selected.push((name, wire));
        }
        Some(selected)
    }

    fn dependency_encoding_properties(
        &mut self,
        owner: hir::SourceNominalId,
        span: Span,
    ) -> Option<Vec<(String, String)>> {
        let declaration = self.loaded_class_definitions[&owner].declaration.clone();
        let mut selected = Vec::new();
        for field in &declaration.field_sources {
            let property = match field.storage {
                hir::NominalFieldStorage::PropertyBacking(property)
                | hir::NominalFieldStorage::PropertyDelegate(property) => property,
                _ => {
                    self.error(
                        span,
                        "automatic encode requires class storage to map to an ordinary property"
                            .into(),
                    );
                    return None;
                }
            };
            let Some(wire) = self.dependency_coding_wire_name(
                hir::AnnotationTargetV1::Property(property),
                &field.name,
            ) else {
                continue;
            };
            let accessors = self
                .dependencies
                .as_ref()
                .expect("an imported property has a catalog")
                .property_declaration(hir::PropertyDeclarationId::Property(property))
                .expect("the stored property is declared")
                .accessors();
            let ordinary =
                |source: hir::PropertyAccessorSourceV1| source.implementation().is_storage();
            if !matches!(field.storage, hir::NominalFieldStorage::PropertyBacking(_))
                || !ordinary(accessors.getter_source())
                || accessors
                    .setter_source()
                    .is_some_and(|source| !ordinary(source))
            {
                self.error(span, format!("stored property `{}` has a custom accessor or delegate; provide an explicit encode implementation or mark it Transient", field.name));
                return None;
            }
            selected.push((field.name.clone(), wire));
        }
        Some(selected)
    }
}
