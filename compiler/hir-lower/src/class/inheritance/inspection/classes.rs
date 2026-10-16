use super::records::InspectionMember;
use super::*;

impl Lowerer {
    pub(super) fn inspect_class(
        &mut self,
        context: CodingContext,
        target: TypeId,
        value: hir::Expr,
        reference: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let application = self.nominal_application(target).unwrap();
        let definition = self.class_definition(application.template);
        let singleton = matches!(
            self.nominal_target_for_type(target),
            Some(crate::NominalTarget::Object(_))
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
            || definition.modifier != hir::ClassModifier::Final
            || definition.base_class.is_some()
            || !matches!(
                definition.representation,
                hir::ClassRepresentation::Declared
            )
        {
            self.error(span, "automatic inspect requires a final class without a class base; provide an explicit implementation for other targets".into());
            return None;
        }
        let fields = match self.source_class_id(application.template) {
            Some(class) => self.current_inspection_properties(class, span)?,
            None => self.dependency_inspection_properties(application.template, span)?,
        };
        let info = self.inspection_shape_info(reference, "Class", span, sink)?;
        let metadata = self.inspection_field(info, "properties", span)?;
        self.inspection_record(context, value, metadata, fields, true, span, sink)
    }

    fn current_inspection_properties(
        &mut self,
        class: ClassId,
        span: Span,
    ) -> Option<Vec<InspectionMember>> {
        let mut fields = Vec::new();
        for (index, property) in self.classes[class]
            .properties
            .clone()
            .into_iter()
            .enumerate()
        {
            if !self.classes[class]
                .fields
                .iter()
                .any(|field| self.class_fields[*field].source.property() == Some(property))
            {
                continue;
            }
            let property = &self.properties[property];
            let capability = property.capability;
            if !matches!(
                property.representation,
                hir::PropertyRepresentation::Stored(_)
            ) || !self.property_getters[capability.getter()]
                .implementation
                .is_storage()
                || capability.setter().is_some_and(|setter| {
                    !self.property_setters[setter].implementation.is_storage()
                })
            {
                self.error(span, format!("stored property `{}` has a custom accessor or delegate; provide an explicit inspect implementation", property.name));
                return None;
            }
            fields.push(InspectionMember {
                name: property.name.clone(),
                index,
            });
        }
        Some(fields)
    }

    fn dependency_inspection_properties(
        &mut self,
        owner: hir::SourceNominalId,
        span: Span,
    ) -> Option<Vec<InspectionMember>> {
        let declaration = &self.loaded_class_definitions[&owner].declaration;
        let dependencies = self.dependencies.as_ref().unwrap();
        let mut properties = declaration
            .interface
            .declaration_details()
            .members()
            .values()
            .iter()
            .filter_map(|member| match member {
                hir::NestedSourceMemberRefV1::Property(id) => Some(*id),
                _ => None,
            })
            .collect::<Vec<_>>();
        properties.sort_by_key(|id| {
            dependencies
                .property_definition_origin(hir::PropertyDeclarationId::Property(*id))
                .expect("a declared property has its source origin")
                .span()
                .start_byte()
        });
        let mut fields = Vec::new();
        for (index, id) in properties.into_iter().enumerate() {
            let Some(field) = declaration.field_sources.iter().find(|field| matches!(field.storage, hir::NominalFieldStorage::PropertyBacking(property) | hir::NominalFieldStorage::PropertyDelegate(property) if property == id)) else { continue };
            let property = dependencies
                .property_declaration(hir::PropertyDeclarationId::Property(id))
                .unwrap();
            let accessors = property.accessors();
            if !matches!(field.storage, hir::NominalFieldStorage::PropertyBacking(_))
                || !accessors.getter_source().implementation().is_storage()
                || accessors
                    .setter_source()
                    .is_some_and(|source| !source.implementation().is_storage())
            {
                self.error(span, format!("stored property `{}` has a custom accessor or delegate; provide an explicit inspect implementation", field.name));
                return None;
            }
            fields.push(InspectionMember {
                name: field.name.clone(),
                index,
            });
        }
        Some(fields)
    }
}
