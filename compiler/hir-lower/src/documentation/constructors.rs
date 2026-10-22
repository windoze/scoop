use super::*;

impl Lowerer {
    pub(super) fn document_struct_primary(
        &mut self,
        id: hir::StructId,
        source: ast::StructSource<'_>,
    ) {
        let constructor = self.structs[id].constructors.iter().copied().find(|id| {
            matches!(
                self.struct_constructors[*id].kind,
                hir::StructConstructorKind::Primary
            )
        });
        let Some(constructor) = constructor else {
            return;
        };
        let callable = Callable::StructConstructor(constructor);
        let mut signature = vec![Part::Text("constructor".into())];
        signatures::list(
            &mut signature,
            "(",
            ")",
            source.fields.iter().map(|field| field.span),
        );
        self.document_declaration(
            Source::Callable(callable).into(),
            Some(Source::Nominal(hir::NominalOwner::Struct(id)).into()),
            Kind::Constructor,
            "constructor",
            source.span,
            self.struct_constructors[constructor].access.declared,
            signature,
            false,
        );
        for (index, field) in source.fields.iter().enumerate() {
            self.document_parameter(callable, index, &field.name.text, field.span, false);
        }
    }

    pub(super) fn document_class_primary(
        &mut self,
        id: hir::ClassId,
        source: ast::ClassSource<'_>,
    ) {
        let constructor = self.classes[id].constructors.iter().copied().find(|id| {
            matches!(
                self.class_constructors[*id].kind,
                hir::ClassConstructorKind::Primary { .. }
            ) && self.class_constructors[*id].identity_kind
                == hir::ClassConstructorIdentityKind::Source
        });
        let Some(constructor) = constructor else {
            return;
        };
        let callable = Callable::ClassConstructor(constructor);
        let (span, parameters, attach) = match &source.constructor {
            ast::ClassConstructorDecl::Declared(primary) => {
                (primary.span, primary.parameters.as_slice(), true)
            }
            ast::ClassConstructorDecl::Omitted => (source.name.span, &[][..], false),
        };
        let mut signature = vec![Part::Text("constructor".into())];
        signatures::list(
            &mut signature,
            "(",
            ")",
            parameters.iter().map(|parameter| parameter.span),
        );
        let owner = Source::Nominal(hir::NominalOwner::Class(id));
        self.document_declaration(
            Source::Callable(callable).into(),
            Some(owner.into()),
            Kind::Constructor,
            "constructor",
            span,
            self.class_constructors[constructor].access.declared,
            signature,
            attach,
        );
        for (index, parameter) in parameters.iter().enumerate() {
            if parameter.property.is_property() {
                let parameter_id = self.class_constructors[constructor].parameters[index].id;
                let property = self.class_fields.iter().find_map(|(_, field)| {
                    if field.owner != id {
                        return None;
                    }
                    match field.source {
                        hir::ClassFieldSource::PrimaryParameter {
                            property,
                            parameter,
                        } if parameter == parameter_id => Some(property),
                        _ => None,
                    }
                });
                if let Some(property) = property {
                    self.document_declaration(
                        Source::Property(property).into(),
                        Some(owner.into()),
                        Kind::Property,
                        &parameter.name.text,
                        parameter.span,
                        self.properties[property].access.declared,
                        vec![Part::Source(parameter.span)],
                        true,
                    );
                }
            }
            self.document_parameter(
                callable,
                index,
                &parameter.name.text,
                parameter.span,
                !parameter.property.is_property(),
            );
        }
    }

    pub(crate) fn document_secondary_constructor(
        &mut self,
        callable: Callable,
        source: &ast::SecondaryConstructorDecl,
    ) {
        if self.documentation.is_none() {
            return;
        }
        let visibility = match callable {
            Callable::ClassConstructor(id) => self.class_constructors[id].access.declared,
            Callable::StructConstructor(id) => self.struct_constructors[id].access.declared,
            _ => unreachable!("a secondary constructor has a typed class or struct owner"),
        };
        let mut signature = vec![Part::Text("constructor".into())];
        signatures::list(
            &mut signature,
            "(",
            ")",
            source.params.iter().map(|parameter| parameter.span),
        );
        self.document_declaration(
            Source::Callable(callable).into(),
            Self::documentation_owner(self.current_owner),
            Kind::Constructor,
            "constructor",
            source.span,
            visibility,
            signature,
            true,
        );
        for (index, parameter) in source.params.iter().enumerate() {
            self.document_parameter(callable, index, &parameter.name.text, parameter.span, true);
        }
    }
}
