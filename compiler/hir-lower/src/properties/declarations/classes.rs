use super::*;

impl Lowerer {
    pub(crate) fn allocate_class_field(
        &mut self,
        owner: hir::ClassId,
        property: hir::PropertyId,
        definition: hir::Field,
        source: hir::ClassFieldSource,
        span: ast::Span,
    ) -> hir::ClassFieldId {
        let fields = &mut self.classes[owner].definition.fields;
        let definition_index = fields.len();
        fields.push(definition);
        self.class_fields.alloc(hir::ClassField {
            owner,
            property,
            definition_index,
            source,
            span,
        })
    }

    pub(crate) fn allocate_primary_class_property(
        &mut self,
        owner: hir::ClassId,
        parameter: &ast::PrimaryClassParameter,
        ty: TypeId,
        parameter_id: hir::ConstructorParamId,
        access: hir::DeclarationAccess,
    ) -> hir::ClassFieldId {
        let expected_property = self.next_property_id();
        let field = self.allocate_class_field(
            owner,
            expected_property,
            hir::Field {
                name: parameter.name.text.clone(),
                ty,
            },
            hir::ClassFieldSource::PrimaryParameter(parameter_id),
            parameter.span,
        );
        let modifier =
            if parameter.is_override && self.classes[owner].modifier != hir::ClassModifier::Final {
                hir::MethodModifier::Open
            } else {
                hir::MethodModifier::Final
            };
        let source = ast::PropertyDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            modifier: match modifier {
                hir::MethodModifier::Final => ast::MethodModifier::Final,
                hir::MethodModifier::Open => ast::MethodModifier::Open,
                hir::MethodModifier::Abstract => {
                    unreachable!("primary-constructor properties are never abstract")
                }
            },
            is_override: parameter.is_override,
            mutable: parameter.property.is_mutable(),
            receiver_ty: None,
            type_params: Vec::new(),
            where_clause: None,
            name: parameter.name.clone(),
            ty: parameter.ty.clone(),
            body: ast::PropertyBodySyntax::Initializer {
                expression: Box::new(ast::Expr::UnitLiteral {
                    span: parameter.span,
                }),
                accessors: ast::AccessorSyntax::default(),
            },
            span: parameter.span,
        };
        let capability = self
            .allocate_property_accessors(
                expected_property,
                hir::PropertyOwner::Class(owner),
                access.clone(),
                &source,
                Some(hir::PropertyBacking::ClassField {
                    field,
                    initializer: hir::ClassPropertyInitializer::PrimaryParameter(parameter_id),
                }),
                modifier,
            )
            .expect("a primary property always has complete storage accessors");
        let property = self.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::Class(owner),
            name: parameter.name.text.clone(),
            access,
            modifier,
            is_override: parameter.is_override,
            overrides: Vec::new(),
            ty,
            capability,
            representation: hir::PropertyRepresentation::Stored(hir::StoredProperty {
                backing: hir::PropertyBacking::ClassField {
                    field,
                    initializer: hir::ClassPropertyInitializer::PrimaryParameter(parameter_id),
                },
            }),
            span: parameter.span,
        });
        assert_eq!(property, expected_property);
        self.classes[owner].properties.push(property);
        field
    }

    pub(crate) fn allocate_class_property(
        &mut self,
        owner: hir::ClassId,
        declaration: &ast::PropertyDecl,
        ty: TypeId,
        access: hir::DeclarationAccess,
    ) -> Option<hir::ClassFieldId> {
        self.allocate_reference_property(
            owner,
            hir::PropertyOwner::Class(owner),
            declaration,
            ty,
            access,
            crate::imports::PropertyImportSource::OutsideCurrentUnitSurface,
        )
    }

    pub(crate) fn allocate_object_property(
        &mut self,
        owner: hir::ObjectId,
        declaration: &ast::PropertyDecl,
        ty: TypeId,
        access: hir::DeclarationAccess,
        import_source: crate::imports::PropertyImportSource,
    ) -> Option<hir::ClassFieldId> {
        let backing = self.objects[owner].backing_class;
        self.allocate_reference_property(
            backing,
            hir::PropertyOwner::Object(owner),
            declaration,
            ty,
            access,
            import_source,
        )
    }

    fn allocate_reference_property(
        &mut self,
        backing_class: hir::ClassId,
        property_owner: hir::PropertyOwner,
        declaration: &ast::PropertyDecl,
        ty: TypeId,
        access: hir::DeclarationAccess,
        import_source: crate::imports::PropertyImportSource,
    ) -> Option<hir::ClassFieldId> {
        self.reject_logical_property_annotations("a class property", &declaration.annotations);
        if declaration.receiver_ty.is_some() || !declaration.type_params.is_empty() {
            self.error(
                declaration.span,
                "extension properties may only be declared at top level".to_string(),
            );
            return None;
        }
        if matches!(property_owner, hir::PropertyOwner::Object(_)) {
            if declaration.modifier == ast::MethodModifier::Abstract {
                self.error(
                    declaration.name.span,
                    format!(
                        "abstract property `{}` is not allowed in an object declaration",
                        declaration.name.text
                    ),
                );
                return None;
            }
            if declaration.modifier == ast::MethodModifier::Open && !declaration.is_override {
                self.error(
                    declaration.name.span,
                    format!(
                        "open property `{}` is not allowed in an object declaration",
                        declaration.name.text
                    ),
                );
                return None;
            }
        }
        let modifier = self.class_property_modifier(backing_class, declaration);
        let abstract_body = matches!(declaration.body, ast::PropertyBodySyntax::Abstract);
        if (modifier == hir::MethodModifier::Abstract) != abstract_body {
            self.error(
                declaration.span,
                format!(
                    "abstract property `{}` must omit initializer, delegate, and accessor bodies",
                    declaration.name.text
                ),
            );
            return None;
        }
        let expected_property = self.next_property_id();
        let (field, representation) = match &declaration.body {
            ast::PropertyBodySyntax::Initializer { .. } => {
                let field = self.allocate_class_field(
                    backing_class,
                    expected_property,
                    hir::Field {
                        name: declaration.name.text.clone(),
                        ty,
                    },
                    hir::ClassFieldSource::Body,
                    declaration.span,
                );
                (
                    Some(field),
                    hir::PropertyRepresentation::Stored(hir::StoredProperty {
                        backing: hir::PropertyBacking::ClassField {
                            field,
                            initializer: hir::ClassPropertyInitializer::Expression,
                        },
                    }),
                )
            }
            ast::PropertyBodySyntax::OptionalOmitted => {
                if !declaration.mutable || self.as_option(ty).is_none() {
                    self.error(
                        declaration.span,
                        format!(
                            "property `{}` without an initializer must be a mutable Option property",
                            declaration.name.text
                        ),
                    );
                    return None;
                }
                let field = self.allocate_class_field(
                    backing_class,
                    expected_property,
                    hir::Field {
                        name: declaration.name.text.clone(),
                        ty,
                    },
                    hir::ClassFieldSource::Body,
                    declaration.span,
                );
                (
                    Some(field),
                    hir::PropertyRepresentation::Stored(hir::StoredProperty {
                        backing: hir::PropertyBacking::ClassField {
                            field,
                            initializer: hir::ClassPropertyInitializer::SyntheticNone,
                        },
                    }),
                )
            }
            ast::PropertyBodySyntax::Computed(_) => {
                (None, hir::PropertyRepresentation::AccessorOnly)
            }
            ast::PropertyBodySyntax::Abstract => (None, hir::PropertyRepresentation::AccessorOnly),
            ast::PropertyBodySyntax::Delegated { .. } => {
                (None, hir::PropertyRepresentation::AccessorOnly)
            }
            ast::PropertyBodySyntax::ExternStorage => {
                self.error(
                    declaration.span,
                    "`@Extern` storage is only allowed on top-level properties".to_string(),
                );
                return None;
            }
            ast::PropertyBodySyntax::Const(_) => {
                self.error(
                    declaration.span,
                    "const properties are not allowed on ordinary class instances".to_string(),
                );
                return None;
            }
        };
        let backing = field.map(|_| match &representation {
            hir::PropertyRepresentation::Stored(stored) => stored.backing,
            hir::PropertyRepresentation::AccessorOnly
            | hir::PropertyRepresentation::Delegated { .. }
            | hir::PropertyRepresentation::GenericDelegated { .. }
            | hir::PropertyRepresentation::Const { .. }
            | hir::PropertyRepresentation::NativeStorage { .. } => {
                unreachable!("a class field is allocated only for stored properties")
            }
        });
        let capability = self.allocate_property_accessors(
            expected_property,
            property_owner,
            access.clone(),
            declaration,
            backing,
            modifier,
        )?;
        let property = self.properties.alloc(hir::Property {
            owner: property_owner,
            name: declaration.name.text.clone(),
            access,
            modifier,
            is_override: declaration.is_override,
            overrides: Vec::new(),
            ty,
            capability,
            representation,
            span: declaration.span,
        });
        assert_eq!(property, expected_property);
        self.classes[backing_class].properties.push(property);
        self.imports.bind_property(import_source, property);
        field
    }

    fn class_property_modifier(
        &mut self,
        owner: hir::ClassId,
        declaration: &ast::PropertyDecl,
    ) -> hir::MethodModifier {
        let modifier = match declaration.modifier {
            ast::MethodModifier::Final => hir::MethodModifier::Final,
            ast::MethodModifier::Open => hir::MethodModifier::Open,
            ast::MethodModifier::Abstract => hir::MethodModifier::Abstract,
        };
        if modifier == hir::MethodModifier::Abstract
            && self.classes[owner].modifier != hir::ClassModifier::Abstract
        {
            self.error(
                declaration.name.span,
                format!(
                    "abstract property `{}` is only allowed in abstract classes",
                    declaration.name.text
                ),
            );
        }
        if modifier == hir::MethodModifier::Open
            && !declaration.is_override
            && self.classes[owner].modifier == hir::ClassModifier::Final
        {
            self.error(
                declaration.name.span,
                format!(
                    "open property `{}` is only allowed in open or abstract classes",
                    declaration.name.text
                ),
            );
        }
        if declaration.is_override
            && self.classes[owner].modifier == hir::ClassModifier::Final
            && modifier == hir::MethodModifier::Open
        {
            hir::MethodModifier::Final
        } else {
            modifier
        }
    }
}
