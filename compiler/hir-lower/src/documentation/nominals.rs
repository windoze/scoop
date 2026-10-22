use super::*;

impl Lowerer {
    pub(crate) fn document_struct(&mut self, id: hir::StructId, source: ast::StructSource<'_>) {
        if self.documentation.is_none() {
            return;
        }
        let target = Source::Nominal(hir::NominalOwner::Struct(id));
        let mut signature = signatures::prefix(source.span, source.name.span);
        signatures::generics(&mut signature, &source.type_params);
        if let ast::StructRepresentationDecl::Declared(fields) = &source.fields {
            signatures::list(
                &mut signature,
                "(",
                ")",
                fields.iter().map(|field| field.span),
            );
        }
        signatures::parents(&mut signature, &source.supertypes);
        signatures::constraints(&mut signature, source.where_clause.as_ref());
        self.document_declaration(
            target.into(),
            self.structs[id]
                .owner
                .map(|owner| Source::Nominal(owner).into()),
            Kind::Struct,
            &source.name.text,
            source.span,
            self.structs[id].access.declared,
            signature,
            true,
        );
        self.document_type_parameters(
            target,
            &source.type_params,
            &self.structs[id].type_params.clone(),
        );
        for (index, field) in source.fields.iter().enumerate() {
            if let Some(field_id) = hir::StructFieldRef::checked(&self.structs, id, index as u32) {
                self.document_declaration(
                    Source::Field(field_id).into(),
                    Some(target.into()),
                    Kind::Field,
                    &field.name.text,
                    field.span,
                    hir::DeclaredVisibility::Public,
                    vec![Part::Source(field.span)],
                    true,
                );
            }
        }
        self.document_struct_primary(id, source);
    }

    pub(crate) fn document_class(&mut self, id: hir::ClassId, source: ast::ClassSource<'_>) {
        if self.documentation.is_none() {
            return;
        }
        let target = Source::Nominal(hir::NominalOwner::Class(id));
        let mut signature = signatures::prefix(source.span, source.name.span);
        signatures::generics(&mut signature, &source.type_params);
        if let ast::ClassConstructorDecl::Declared(constructor) = &source.constructor {
            signature.push(Part::Text(" ".into()));
            signature.push(Part::Source(constructor.span));
        }
        signatures::parents(&mut signature, &source.supertypes);
        signatures::constraints(&mut signature, source.where_clause.as_ref());
        self.document_declaration(
            target.into(),
            self.classes[id]
                .owner
                .map(|owner| Source::Nominal(owner).into()),
            Kind::Class,
            &source.name.text,
            source.span,
            self.classes[id].access.declared,
            signature,
            true,
        );
        let parameters = self.classes[id].type_params.clone();
        self.document_type_parameters(
            target,
            &source.type_params,
            &parameters[self.enclosing_parameter_count(id)..],
        );
        self.document_class_primary(id, source);
    }

    pub(crate) fn document_interface(
        &mut self,
        id: hir::InterfaceId,
        source: ast::InterfaceSource<'_>,
    ) {
        if self.documentation.is_none() {
            return;
        }
        let target = Source::Nominal(hir::NominalOwner::Interface(id));
        let mut signature = signatures::prefix(source.span, source.name.span);
        signatures::generics(&mut signature, &source.type_params);
        signatures::parents(&mut signature, &source.supertypes);
        signatures::constraints(&mut signature, source.where_clause.as_ref());
        self.document_declaration(
            target.into(),
            self.interfaces[id]
                .owner
                .map(|owner| Source::Nominal(owner).into()),
            Kind::Interface,
            &source.name.text,
            source.span,
            self.interfaces[id].access.declared,
            signature,
            true,
        );
        self.document_type_parameters(
            target,
            &source.type_params,
            &self.interfaces[id].type_params.clone(),
        );
    }

    pub(crate) fn document_object(
        &mut self,
        id: hir::ObjectId,
        source: crate::declarations::ObjectSource<'_>,
    ) {
        if self.documentation.is_none() {
            return;
        }
        let (name, name_span) = source.name();
        let mut signature = signatures::prefix(source.span(), name_span);
        signatures::parents(&mut signature, source.supertypes());
        self.document_declaration(
            Source::Nominal(hir::NominalOwner::Object(id)).into(),
            self.objects[id]
                .owner
                .map(|owner| Source::Nominal(owner).into()),
            Kind::Object,
            name,
            source.span(),
            self.objects[id].access.declared,
            signature,
            true,
        );
    }

    pub(crate) fn document_enum(&mut self, id: hir::EnumId, source: ast::EnumSource<'_>) {
        if self.documentation.is_none() {
            return;
        }
        let target = Source::Nominal(hir::NominalOwner::Enum(id));
        let mut signature = signatures::prefix(source.span, source.name.span);
        signatures::generics(&mut signature, &source.type_params);
        if !source.interfaces.is_empty() {
            signatures::list(
                &mut signature,
                " : ",
                "",
                source.interfaces.iter().map(|ty| ty.span),
            );
        }
        signatures::constraints(&mut signature, source.where_clause.as_ref());
        self.document_declaration(
            target.into(),
            self.enums[id]
                .owner
                .map(|owner| Source::Nominal(owner).into()),
            Kind::Enum,
            &source.name.text,
            source.span,
            self.enums[id].access.declared,
            signature,
            true,
        );
        self.document_type_parameters(
            target,
            &source.type_params,
            &self.enums[id].type_params.clone(),
        );
        for (index, variant) in source.variants.iter().enumerate() {
            let Some(id) = hir::EnumVariantRef::checked(&self.enums, id, index as u32) else {
                continue;
            };
            self.document_declaration(
                Source::Variant(id).into(),
                Some(target.into()),
                Kind::Variant,
                &variant.name.text,
                variant.span,
                hir::DeclaredVisibility::Public,
                vec![Part::Source(variant.span)],
                true,
            );
            let fields = match &variant.kind {
                ast::VariantDeclKind::Unit | ast::VariantDeclKind::Constant(_) => Vec::new(),
                ast::VariantDeclKind::Positional(fields) => {
                    fields.iter().map(|field| (None, field.span)).collect()
                }
                ast::VariantDeclKind::Named(fields) | ast::VariantDeclKind::Constructor(fields) => {
                    fields
                        .iter()
                        .map(|field| (Some(field.name.text.as_str()), field.span))
                        .collect()
                }
            };
            for (index, (name, span)) in fields.into_iter().enumerate() {
                let Some(field) = hir::EnumVariantFieldRef::checked(&self.enums, id, index as u32)
                else {
                    continue;
                };
                self.document_declaration(
                    Source::VariantField(field).into(),
                    Some(Source::Variant(id).into()),
                    Kind::Parameter,
                    name.unwrap_or("_"),
                    span,
                    hir::DeclaredVisibility::Public,
                    vec![Part::Source(span)],
                    true,
                );
            }
        }
    }
}
