use super::*;
use hir::SourceAnnotationTarget as Target;

impl Lowerer {
    pub(crate) fn annotate_struct(
        &mut self,
        id: hir::StructId,
        source: ast::StructSource<'_>,
        file: usize,
    ) {
        let owner = Owner::Struct(id);
        self.annotation_scope(owner, file);
        self.document_struct(id, source);
        self.annotate_type_bounds(
            Target::Nominal(owner.as_nominal_owner()),
            &source.type_params,
            source.where_clause.as_ref(),
        );
        self.annotate_parent_specs(
            Target::Nominal(owner.as_nominal_owner()),
            &source.supertypes,
        );
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        for (index, field) in source.fields.iter().enumerate() {
            if let Some(field_ref) = hir::StructFieldRef::checked(&self.structs, id, index as u32) {
                self.bind_source_annotations(Target::Field(field_ref), &field.annotations);
                self.annotate_type_site(
                    Target::Field(field_ref),
                    hir::AnnotationTypeSlot::Value,
                    &field.ty,
                );
                if let Some(constructor) =
                    self.structs[id].constructors.iter().copied().find(|id| {
                        matches!(
                            self.struct_constructors[*id].kind,
                            hir::StructConstructorKind::Primary
                        )
                    })
                {
                    self.annotate_type_site(
                        Target::Parameter {
                            callable: hir::SourceAnnotationCallable::StructConstructor(constructor),
                            index: index as u32,
                        },
                        hir::AnnotationTypeSlot::Value,
                        &field.ty,
                    );
                }
            }
        }
        self.annotate_struct_constructors(id, source);
        self.annotate_interface_blocks(
            owner,
            source.supertypes.len(),
            source.members.iter().filter_map(|member| match member {
                ast::StructMember::InterfaceImplementation(block) => Some(block.as_ref()),
                _ => None,
            }),
        );
        for member in source.members {
            if let ast::StructMember::Property(property) = member {
                self.annotate_property_declaration(property, file);
            }
        }
    }

    pub(crate) fn annotate_enum(
        &mut self,
        id: hir::EnumId,
        source: ast::EnumSource<'_>,
        file: usize,
    ) {
        let owner = Owner::Enum(id);
        self.annotation_scope(owner, file);
        self.document_enum(id, source);
        self.annotate_type_bounds(
            Target::Nominal(owner.as_nominal_owner()),
            &source.type_params,
            source.where_clause.as_ref(),
        );
        self.annotate_supertypes(
            Target::Nominal(owner.as_nominal_owner()),
            &source.interfaces,
        );
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
                ast::VariantDeclKind::Unit | ast::VariantDeclKind::Constant(_) => Vec::new(),
                ast::VariantDeclKind::Positional(fields) => fields
                    .iter()
                    .map(|field| (field.annotations.as_slice(), &field.ty))
                    .collect(),
                ast::VariantDeclKind::Named(fields) | ast::VariantDeclKind::Constructor(fields) => {
                    fields
                        .iter()
                        .map(|field| (field.annotations.as_slice(), &field.ty))
                        .collect()
                }
            };
            for (index, (annotations, ty)) in annotations.into_iter().enumerate() {
                if let Some(field) =
                    hir::EnumVariantFieldRef::checked(&self.enums, variant_ref, index as u32)
                {
                    self.bind_source_annotations(Target::VariantField(field), annotations);
                    self.annotate_type_site(
                        Target::VariantField(field),
                        hir::AnnotationTypeSlot::Value,
                        ty,
                    );
                    self.annotate_type_site(
                        Target::Parameter {
                            callable: hir::SourceAnnotationCallable::VariantConstructor(
                                variant_ref,
                            ),
                            index: index as u32,
                        },
                        hir::AnnotationTypeSlot::Value,
                        ty,
                    );
                }
            }
        }
        for property in source.properties {
            self.annotate_property_declaration(property, file);
        }
        self.annotate_interface_blocks(
            owner,
            source.interfaces.len(),
            source.interface_implementations.iter(),
        );
    }

    pub(crate) fn annotate_class(
        &mut self,
        id: hir::ClassId,
        source: ast::ClassSource<'_>,
        file: usize,
    ) {
        let owner = Owner::Class(id);
        self.annotation_scope(owner, file);
        self.document_class(id, source);
        self.annotate_type_bounds(
            Target::Nominal(owner.as_nominal_owner()),
            &source.type_params,
            source.where_clause.as_ref(),
        );
        self.annotate_parent_specs(
            Target::Nominal(owner.as_nominal_owner()),
            &source.supertypes,
        );
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        if let ast::ClassConstructorDecl::Declared(constructor) = &source.constructor {
            for parameter in &constructor.parameters {
                if !parameter.property.is_property() {
                    continue;
                }
                self.reject_logical_property_annotations(
                    "a primary constructor property",
                    &parameter.annotations,
                );
                self.annotate_property(
                    owner,
                    &parameter.name.text,
                    &parameter.annotations,
                    &parameter.ty,
                );
            }
        }
        self.annotate_class_constructors(id, source);
        self.annotate_class_members(owner, source.members, source.supertypes.len());
    }

    pub(crate) fn annotate_interface(
        &mut self,
        id: hir::InterfaceId,
        source: ast::InterfaceSource<'_>,
        file: usize,
    ) {
        let owner = Owner::Interface(id);
        self.annotation_scope(owner, file);
        self.document_interface(id, source);
        self.annotate_type_bounds(
            Target::Nominal(owner.as_nominal_owner()),
            &source.type_params,
            source.where_clause.as_ref(),
        );
        self.annotate_parent_specs(
            Target::Nominal(owner.as_nominal_owner()),
            &source.supertypes,
        );
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            &source.annotations,
        );
        for property in source.properties {
            self.annotate_property_declaration(property, file);
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
        self.document_object(id, source);
        self.annotate_parent_specs(
            Target::Nominal(owner.as_nominal_owner()),
            source.supertypes(),
        );
        self.bind_source_annotations(
            Target::Nominal(owner.as_nominal_owner()),
            source.annotations(),
        );
        self.annotate_class_members(owner, source.members(), source.supertypes().len());
    }

    fn annotate_class_members(
        &mut self,
        owner: Owner,
        members: &[ast::ClassMember],
        ordinary_count: usize,
    ) {
        for member in members {
            if let ast::ClassMember::StoredProperty(property) = member {
                self.annotate_property_declaration(property, self.current_file);
            }
        }
        self.annotate_interface_blocks(
            owner,
            ordinary_count,
            members.iter().filter_map(|member| match member {
                ast::ClassMember::InterfaceImplementation(block) => Some(block.as_ref()),
                _ => None,
            }),
        );
    }

    fn annotate_property(
        &mut self,
        owner: Owner,
        name: &str,
        annotations: &[ast::Annotation],
        ty: &ast::TypeRef,
    ) {
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
            self.annotate_property_type(property, ty);
        }
    }

    fn annotation_scope(&mut self, owner: Owner, file: usize) {
        self.current_file = file;
        self.current_owner = Some(owner);
        self.type_params_in_scope = self.owner_type_params(owner);
    }
}
