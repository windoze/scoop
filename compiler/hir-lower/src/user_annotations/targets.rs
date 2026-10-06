use super::*;
use hir::SourceAnnotationTarget as Target;

impl Lowerer {
    pub(crate) fn annotate_struct(
        &mut self,
        id: hir::StructId,
        source: &ast::StructDecl,
        file: usize,
    ) {
        let owner = Owner::Struct(id);
        self.annotation_scope(owner, file);
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        for (index, field) in source.fields.iter().enumerate() {
            if let Some(field_ref) = hir::StructFieldRef::checked(&self.structs, id, index as u32) {
                self.bind_source_annotations(Target::Field(field_ref), &field.annotations);
            }
        }
        for member in &source.members {
            if let ast::StructMember::Property(property) = member {
                self.reject_value_property_annotations(property);
            }
        }
    }

    pub(crate) fn annotate_enum(&mut self, id: hir::EnumId, source: &ast::EnumDecl, file: usize) {
        let owner = Owner::Enum(id);
        self.annotation_scope(owner, file);
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        for (index, variant) in source.variants.iter().enumerate() {
            let Some(variant_ref) = hir::EnumVariantRef::checked(&self.enums, id, index as u32)
            else {
                continue;
            };
            self.bind_source_annotations(Target::Variant(variant_ref), &variant.annotations);
            let annotations = match &variant.kind {
                ast::VariantDeclKind::Unit => Vec::new(),
                ast::VariantDeclKind::Positional(fields) => fields
                    .iter()
                    .map(|field| field.annotations.as_slice())
                    .collect(),
                ast::VariantDeclKind::Named(fields) | ast::VariantDeclKind::Constructor(fields) => {
                    fields
                        .iter()
                        .map(|field| field.annotations.as_slice())
                        .collect()
                }
            };
            for (index, annotations) in annotations.into_iter().enumerate() {
                if let Some(field) =
                    hir::EnumVariantFieldRef::checked(&self.enums, variant_ref, index as u32)
                {
                    self.bind_source_annotations(Target::VariantField(field), annotations);
                }
            }
        }
        for property in &source.properties {
            self.reject_value_property_annotations(property);
        }
    }

    pub(crate) fn annotate_class(
        &mut self,
        id: hir::ClassId,
        source: &ast::ClassDecl,
        file: usize,
    ) {
        let owner = Owner::Class(id);
        self.annotation_scope(owner, file);
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        if let ast::ClassConstructorDecl::Declared(constructor) = &source.constructor {
            for parameter in &constructor.parameters {
                if !parameter.property.is_property() {
                    for annotation in &parameter.annotations {
                        self.error(
                            annotation.span,
                            "annotations on primary constructor parameters require val or var"
                                .into(),
                        );
                    }
                    continue;
                }
                self.reject_logical_property_annotations(
                    "a primary constructor property",
                    &parameter.annotations,
                );
                self.annotate_property(owner, &parameter.name.text, &parameter.annotations);
            }
        }
        self.annotate_class_members(owner, &source.members);
    }

    pub(crate) fn annotate_interface(
        &mut self,
        id: hir::InterfaceId,
        source: &ast::InterfaceDecl,
        file: usize,
    ) {
        let owner = Owner::Interface(id);
        self.annotation_scope(owner, file);
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        for property in &source.properties {
            self.annotate_property(owner, &property.name.text, &property.annotations);
        }
    }

    pub(crate) fn annotate_object(
        &mut self,
        id: hir::ObjectId,
        source: crate::declarations::ObjectSource<'_>,
        file: usize,
    ) {
        let owner = Owner::Object(id);
        self.annotation_scope(owner, file);
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            source.annotations(),
        );
        self.annotate_class_members(owner, source.members());
    }

    fn annotate_class_members(&mut self, owner: Owner, members: &[ast::ClassMember]) {
        for member in members {
            if let ast::ClassMember::StoredProperty(property) = member {
                self.annotate_property(owner, &property.name.text, &property.annotations);
            }
        }
    }

    fn annotate_property(&mut self, owner: Owner, name: &str, annotations: &[ast::Annotation]) {
        let properties = match owner {
            Owner::Struct(id) => &self.structs[id].properties,
            Owner::Enum(id) => &self.enums[id].properties,
            Owner::Class(id) => &self.classes[id].properties,
            Owner::Interface(id) => &self.interfaces[id].properties,
            Owner::Object(id) => &self.classes[self.objects[id].backing_class].properties,
        };
        if let Some(property) = properties
            .iter()
            .copied()
            .find(|property| self.properties[*property].name == name)
        {
            self.bind_source_annotations(Target::Property(property), annotations);
        }
    }

    fn annotation_scope(&mut self, owner: Owner, file: usize) {
        self.current_file = file;
        self.current_owner = Some(owner);
        self.type_params_in_scope.clear();
    }

    fn reject_value_property_annotations(&mut self, property: &ast::PropertyDecl) {
        for annotation in &property.annotations {
            if !crate::annotations::is_core_annotation(&annotation.name.text) {
                self.error(annotation.span, "user annotations on value types belong to constructor fields, not computed properties".into());
            }
        }
    }
}
