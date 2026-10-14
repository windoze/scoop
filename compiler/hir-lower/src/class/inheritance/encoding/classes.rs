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
        for (name, wire, omit_none) in fields {
            self.check_encoding_wire_name(&wire, "field", span, &mut names)?;
            let Some(field) = self.coding_property(value.clone(), &name, span) else {
                self.error(
                    span,
                    format!("automatic encode cannot read property `{name}` from this codec"),
                );
                return None;
            };
            self.encode_field(
                context,
                field,
                container.clone(),
                Some(&EncodingMember { wire, omit_none }),
                span,
                sink,
            )?;
        }
        self.end_coding_container(container, span, sink)
    }

    fn current_encoding_properties(
        &mut self,
        class: ClassId,
        span: Span,
    ) -> Option<Vec<(String, String, bool)>> {
        let primary = self.classes[class].constructors.iter().copied().find(|id| {
            matches!(
                self.class_constructors[*id].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        });
        let mut omissions = std::collections::HashMap::new();
        if let Some(primary) = primary {
            let flags = self.current_coding_omissions(
                crate::call_resolution::candidates::NominalConstructorSource::Class(primary),
                span,
            )?;
            for (parameter, flag) in self.class_constructors[primary]
                .parameters
                .iter()
                .zip(flags)
            {
                omissions.insert(parameter.id, flag);
            }
        }
        let mut selected = Vec::new();
        for field in self.classes[class].fields.clone() {
            let Some(id) = self.class_fields[field].source.property() else {
                continue;
            };
            let property = &self.properties[id];
            let name = property.name.clone();
            let config = self.serialization_config(hir::SourceAnnotationTarget::Property(id));
            let Some(wire) = config.wire_name(&name) else {
                continue;
            };
            if !matches!(
                property.representation,
                hir::PropertyRepresentation::Stored(_)
            ) {
                self.error(
                    span,
                    format!(
                        "property `{name}` requires an explicit encode implementation or Serialize(ignore = true)"
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
                self.error(span, format!("stored property `{name}` has a custom accessor; provide an explicit encode implementation or mark it Serialize(ignore = true)"));
                return None;
            }
            let omit_none = !config.required
                && !config.emit_null
                && match self.class_fields[field].source {
                    hir::ClassFieldSource::PrimaryParameter { parameter, .. } => {
                        omissions[&parameter]
                    }
                    _ => self.as_option(property.ty).is_some(),
                };
            selected.push((name, wire, omit_none));
        }
        Some(selected)
    }

    fn dependency_encoding_properties(
        &mut self,
        owner: hir::SourceNominalId,
        span: Span,
    ) -> Option<Vec<(String, String, bool)>> {
        let declaration = self.loaded_class_definitions[&owner].declaration.clone();
        let mut omissions = std::collections::HashMap::new();
        if let Some(primary) = declaration
            .interface
            .declaration_details()
            .class_primary_constructor()
        {
            let flags = self.dependency_coding_omissions(
                scoop_identity::CallableTemplateOrigin::Constructor(primary.constructor()),
            );
            for (property, flag) in primary.properties().iter().zip(flags) {
                if let Some(property) = property {
                    omissions.insert(*property, flag);
                }
            }
        }
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
            let config =
                self.dependency_serialization_config(hir::AnnotationTargetV1::Property(property));
            let Some(wire) = config.wire_name(&field.name) else {
                continue;
            };
            let source = self
                .dependencies
                .as_ref()
                .expect("an imported property has a catalog")
                .property_declaration(hir::PropertyDeclarationId::Property(property))
                .expect("the stored property is declared");
            let accessors = source.accessors();
            let ordinary =
                |source: hir::PropertyAccessorSourceV1| source.implementation().is_storage();
            if !matches!(field.storage, hir::NominalFieldStorage::PropertyBacking(_))
                || !ordinary(accessors.getter_source())
                || accessors
                    .setter_source()
                    .is_some_and(|source| !ordinary(source))
            {
                self.error(span, format!("stored property `{}` has a custom accessor or delegate; provide an explicit encode implementation or mark it Serialize(ignore = true)", field.name));
                return None;
            }
            let omit_none = !config.required
                && !config.emit_null
                && omissions
                    .get(&property)
                    .copied()
                    .unwrap_or_else(|| self.signature_is_core_option(source.value_type()));
            selected.push((field.name.clone(), wire, omit_none));
        }
        Some(selected)
    }
}
