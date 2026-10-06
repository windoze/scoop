use super::*;
use crate::NominalTarget;

impl Lowerer {
    pub(super) fn decode_class_shape(
        &mut self,
        result: TypeId,
        span: Span,
    ) -> Option<DecodeRecord> {
        let application = self
            .nominal_application(result)
            .expect("a class has a nominal application");
        let definition = self.class_definition(application.template);
        let singleton = matches!(
            self.nominal_target_for_type(result),
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
        if singleton {
            self.error(span, format!("automatic decode cannot construct singleton {}; provide an explicit implementation", self.type_name(result)));
            return None;
        }
        if definition.modifier != hir::ClassModifier::Final || definition.base_class.is_some() {
            self.error(span, format!("automatic decode requires a final class without a class base; {} requires an explicit implementation", self.type_name(result)));
            return None;
        }
        let mut record = self.coding_primary(result, span)?;
        if let Some(class) = self.source_class_id(application.template) {
            self.current_decoding_class(class, &mut record, span)?;
        } else {
            self.dependency_decoding_class(application.template, &mut record, span)?;
        }
        Some(record)
    }

    fn current_decoding_class(
        &mut self,
        class: ClassId,
        record: &mut DecodeRecord,
        span: Span,
    ) -> Option<()> {
        let DecodeConstructor::Current {
            source: NominalConstructorSource::Class(constructor),
            ..
        } = record.constructor
        else {
            unreachable!("a current class has its source primary constructor")
        };
        let primary = self.class_constructors[constructor].parameters.clone();
        let fields = self.classes[class].fields.clone();
        for (parameter, source) in record.parameters.iter_mut().zip(primary) {
            let property = fields.iter().find_map(|&field| {
                let field = &self.class_fields[field];
                (field.source == hir::ClassFieldSource::PrimaryParameter(source.id))
                    .then_some(field.property)
            });
            parameter.wire = property.and_then(|property| {
                self.serialization_wire_name(
                    hir::SourceAnnotationTarget::Property(property),
                    &parameter.name,
                )
            });
        }
        for field in fields {
            let declaration = &self.class_fields[field];
            if declaration.source == hir::ClassFieldSource::Body {
                let property = declaration.property;
                let name = self.properties[property].name.clone();
                if self
                    .serialization_wire_name(hir::SourceAnnotationTarget::Property(property), &name)
                    .is_some()
                {
                    self.error(span, format!("automatic decode requires stored property `{name}` to be a primary constructor parameter or Transient"));
                    return None;
                }
            }
        }
        Some(())
    }

    fn dependency_decoding_class(
        &mut self,
        owner: hir::SourceNominalId,
        record: &mut DecodeRecord,
        span: Span,
    ) -> Option<()> {
        let declaration = self.loaded_class_definitions[&owner].declaration.clone();
        let primary = declaration
            .interface
            .declaration_details()
            .class_primary_constructor()
            .expect("the selected class has its primary mapping");
        for (parameter, property) in record.parameters.iter_mut().zip(primary.properties()) {
            parameter.wire = property.and_then(|property| {
                self.dependency_coding_wire_name(
                    hir::AnnotationTargetV1::Property(property),
                    &parameter.name,
                )
            });
        }
        for field in &declaration.field_sources {
            let property = match field.storage {
                hir::NominalFieldStorage::PropertyBacking(property)
                | hir::NominalFieldStorage::PropertyDelegate(property) => property,
                _ => {
                    self.error(span, "automatic decode requires class storage to map to a primary property or Transient property".into());
                    return None;
                }
            };
            if !primary.properties().contains(&Some(property))
                && self
                    .dependency_coding_wire_name(
                        hir::AnnotationTargetV1::Property(property),
                        "stored property",
                    )
                    .is_some()
            {
                self.error(
                    span,
                    "automatic decode requires non-primary stored properties to be Transient"
                        .into(),
                );
                return None;
            }
        }
        Some(())
    }
}
