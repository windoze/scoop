use scoop_ast as ast;
use scoop_hir as hir;

use super::{BackingFieldContext, PropertyAccessorKind, PropertyAccessorSource};
use crate::annotations::FunctionTarget;
use crate::{Function, FunctionKind, Lowerer, Owner, TypeId};

mod interface;

struct AccessorFunctionAllocation {
    property: hir::PropertyId,
    kind: PropertyAccessorKind,
    owner: hir::PropertyOwner,
    access: hir::DeclarationAccess,
    backing: Option<hir::PropertyBacking>,
    modifier: hir::MethodModifier,
}

impl Lowerer {
    pub(crate) fn resolve_interface_properties(
        &mut self,
        owner: hir::InterfaceId,
        declaration: &ast::InterfaceDecl,
    ) {
        self.type_params_in_scope = self.interfaces[owner].type_params.clone();
        let mut names = std::collections::HashSet::new();
        for property in &declaration.properties {
            if !names.insert(property.name.text.clone()) {
                self.error(
                    property.name.span,
                    format!(
                        "duplicate property `{}` in interface `{}`",
                        property.name.text, declaration.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&property.ty) else {
                continue;
            };
            let private = matches!(
                property.visibility,
                ast::VisibilitySyntax::Explicit {
                    visibility: ast::DeclaredVisibility::Private,
                    ..
                }
            );
            let access = self.member_access(
                property.visibility,
                property.name.span,
                "property",
                Owner::Interface(owner),
                self.current_file,
                if private {
                    crate::visibility::MemberSlotAccess::None
                } else if property.is_override {
                    crate::visibility::MemberSlotAccess::Override
                } else {
                    crate::visibility::MemberSlotAccess::Declared
                },
            );
            if self.interfaces[owner].access.declared == hir::DeclaredVisibility::Public
                && !matches!(
                    access.declared,
                    hir::DeclaredVisibility::Public | hir::DeclaredVisibility::Private
                )
            {
                self.error(
                    property.name.span,
                    format!(
                        "member `{}` of public interface `{}` must be explicitly public",
                        property.name.text, declaration.name.text
                    ),
                );
            }
            if private && property.is_override {
                self.error(
                    property.name.span,
                    format!(
                        "private interface property `{}` cannot be an override",
                        property.name.text
                    ),
                );
            }
            self.allocate_interface_property(owner, property, ty, access);
        }
        self.type_params_in_scope.clear();
    }

    pub(crate) fn next_property_id(&self) -> hir::PropertyId {
        hir::PropertyId::from_raw((self.properties.len() as u32).into())
    }

    pub(crate) fn allocate_property_accessors(
        &mut self,
        property: hir::PropertyId,
        owner: hir::PropertyOwner,
        access: hir::DeclarationAccess,
        declaration: &ast::PropertyDecl,
        backing: Option<hir::PropertyBacking>,
        modifier: hir::MethodModifier,
    ) -> Option<hir::PropertyCapability> {
        let accessors = match &declaration.body {
            ast::PropertyBodySyntax::Initializer { accessors, .. }
            | ast::PropertyBodySyntax::Computed(accessors) => Some(accessors),
            ast::PropertyBodySyntax::OptionalOmitted => None,
            ast::PropertyBodySyntax::Delegated { .. }
            | ast::PropertyBodySyntax::Abstract
            | ast::PropertyBodySyntax::ExternStorage
            | ast::PropertyBodySyntax::Const(_) => None,
        };
        let computed = matches!(declaration.body, ast::PropertyBodySyntax::Computed(_));
        let abstract_property = matches!(declaration.body, ast::PropertyBodySyntax::Abstract);
        let interface_property = matches!(owner, hir::PropertyOwner::Interface(_));
        let dispatch_storage = matches!(
            owner,
            hir::PropertyOwner::Class(_) | hir::PropertyOwner::Object(_)
        ) && (modifier != hir::MethodModifier::Final
            || declaration.is_override);
        let runtime_storage = matches!(
            backing,
            Some(hir::PropertyBacking::TopLevelGlobal {
                initialization: hir::TopLevelInitialization::Runtime(_),
                ..
            })
        );
        let exported_storage = matches!(backing, Some(hir::PropertyBacking::TopLevelGlobal { .. }))
            && Self::declaration_is_exported(&access);
        if matches!(declaration.body, ast::PropertyBodySyntax::Delegated { .. }) {
            return Some(self.allocate_delegated_property_accessors(
                property,
                owner,
                access,
                declaration,
                modifier,
            ));
        }

        let getter_source = accessors.and_then(|accessors| accessors.getter.as_ref());
        let (getter_implementation, getter_attributes) = match getter_source {
            Some(source) if !matches!(source.body, ast::AccessorBodySyntax::Omitted) => {
                let function_declaration = self.getter_function_declaration(declaration, source);
                let function = self.allocate_accessor_function(
                    function_declaration,
                    AccessorFunctionAllocation {
                        property,
                        kind: PropertyAccessorKind::Getter,
                        owner,
                        access: access.clone(),
                        backing,
                        modifier,
                    },
                );
                (
                    hir::PropertyAccessorImplementation::Body(function),
                    self.functions[function].attributes,
                )
            }
            _ if abstract_property || interface_property => {
                let function_declaration = self.implicit_getter_function_declaration(
                    declaration,
                    ast::AccessorBodySyntax::Omitted,
                );
                let function = self.allocate_accessor_function(
                    function_declaration,
                    AccessorFunctionAllocation {
                        property,
                        kind: PropertyAccessorKind::Getter,
                        owner,
                        access: access.clone(),
                        backing,
                        modifier: hir::MethodModifier::Abstract,
                    },
                );
                (
                    hir::PropertyAccessorImplementation::AbstractSlot(function),
                    self.functions[function].attributes,
                )
            }
            _ if backing.is_some()
                && (runtime_storage
                    || exported_storage
                    || dispatch_storage
                    || getter_source.is_some()) =>
            {
                let function_declaration = self.implicit_getter_function_declaration(
                    declaration,
                    ast::AccessorBodySyntax::Expr(Box::new(ast::Expr::Var(ast::Ident {
                        text: "field".to_string(),
                        span: declaration.name.span,
                    }))),
                );
                let function = self.allocate_accessor_function(
                    function_declaration,
                    AccessorFunctionAllocation {
                        property,
                        kind: PropertyAccessorKind::Getter,
                        owner,
                        access: access.clone(),
                        backing,
                        modifier,
                    },
                );
                (
                    hir::PropertyAccessorImplementation::Body(function),
                    self.functions[function].attributes,
                )
            }
            _ if backing.is_some() => (
                hir::PropertyAccessorImplementation::Storage,
                hir::FunctionAttributes::default(),
            ),
            _ if computed => {
                self.error(
                    declaration.name.span,
                    format!(
                        "computed property `{}` must provide a getter body",
                        declaration.name.text
                    ),
                );
                return None;
            }
            _ => return None,
        };
        let getter_span = getter_source.map_or(declaration.name.span, |source| source.span);
        let getter = self.property_getters.alloc(hir::PropertyGetter {
            access: access.clone(),
            implementation: getter_implementation,
            attributes: getter_attributes,
            span: getter_span,
        });

        let setter_source = accessors.and_then(|accessors| accessors.setter.as_ref());
        if !declaration.mutable {
            if let Some(setter) = setter_source {
                self.error(
                    setter.span,
                    format!(
                        "immutable property `{}` cannot declare a setter",
                        declaration.name.text
                    ),
                );
                return None;
            }
            return Some(hir::PropertyCapability::ReadOnly { getter });
        }

        let setter_access = self.property_setter_access(declaration, setter_source, owner, access);
        let (setter_implementation, setter_attributes) = match setter_source {
            Some(source) if !matches!(source.body, ast::AccessorBodySyntax::Omitted) => {
                let function_declaration = self.setter_function_declaration(declaration, source);
                let function = self.allocate_accessor_function(
                    function_declaration,
                    AccessorFunctionAllocation {
                        property,
                        kind: PropertyAccessorKind::Setter,
                        owner,
                        access: setter_access.clone(),
                        backing,
                        modifier,
                    },
                );
                (
                    hir::PropertyAccessorImplementation::Body(function),
                    self.functions[function].attributes,
                )
            }
            _ if abstract_property || interface_property => {
                let function_declaration = self.implicit_setter_function_declaration(
                    declaration,
                    ast::AccessorBodySyntax::Omitted,
                );
                let function = self.allocate_accessor_function(
                    function_declaration,
                    AccessorFunctionAllocation {
                        property,
                        kind: PropertyAccessorKind::Setter,
                        owner,
                        access: setter_access.clone(),
                        backing,
                        modifier: hir::MethodModifier::Abstract,
                    },
                );
                (
                    hir::PropertyAccessorImplementation::AbstractSlot(function),
                    self.functions[function].attributes,
                )
            }
            _ if backing.is_some()
                && (runtime_storage
                    || exported_storage
                    || dispatch_storage
                    || setter_source.is_some()) =>
            {
                let parameter = setter_source.map_or_else(
                    || ast::Ident {
                        text: "value".to_string(),
                        span: declaration.name.span,
                    },
                    |setter| match &setter.parameter {
                        ast::SetterParameterSyntax::Default { span } => ast::Ident {
                            text: "value".to_string(),
                            span: *span,
                        },
                        ast::SetterParameterSyntax::Named(name) => name.clone(),
                    },
                );
                let body = ast::AccessorBodySyntax::Block(ast::Block {
                    statements: vec![ast::Statement {
                        kind: ast::StatementKind::Assign(ast::Assign {
                            target: ast::PlaceExpr::Name(ast::Ident {
                                text: "field".to_string(),
                                span: declaration.name.span,
                            }),
                            op: ast::AssignmentOp::Assign,
                            value: ast::Expr::Var(parameter),
                            span: declaration.name.span,
                        }),
                        span: declaration.name.span,
                    }],
                    span: declaration.span,
                });
                let function_declaration =
                    self.implicit_setter_function_declaration(declaration, body);
                let function = self.allocate_accessor_function(
                    function_declaration,
                    AccessorFunctionAllocation {
                        property,
                        kind: PropertyAccessorKind::Setter,
                        owner,
                        access: setter_access.clone(),
                        backing,
                        modifier,
                    },
                );
                (
                    hir::PropertyAccessorImplementation::Body(function),
                    self.functions[function].attributes,
                )
            }
            _ if backing.is_some() => (
                hir::PropertyAccessorImplementation::Storage,
                hir::FunctionAttributes::default(),
            ),
            _ if computed => {
                self.error(
                    declaration.name.span,
                    format!(
                        "computed mutable property `{}` must provide a setter body",
                        declaration.name.text
                    ),
                );
                return None;
            }
            _ => return None,
        };
        let parameter_name = setter_source.map_or_else(
            || "value".to_string(),
            |setter| match &setter.parameter {
                ast::SetterParameterSyntax::Default { .. } => "value".to_string(),
                ast::SetterParameterSyntax::Named(name) => name.text.clone(),
            },
        );
        let setter_span = setter_source.map_or(declaration.name.span, |source| source.span);
        let setter = self.property_setters.alloc(hir::PropertySetter {
            access: setter_access,
            implementation: setter_implementation,
            attributes: setter_attributes,
            parameter_name,
            span: setter_span,
        });
        Some(hir::PropertyCapability::ReadWrite { getter, setter })
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
        let field = self.class_fields.alloc(hir::ClassField {
            owner,
            property: expected_property,
            ty,
            source: hir::ClassFieldSource::PrimaryParameter(parameter_id),
            span: parameter.span,
        });
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
            override_access: Vec::new(),
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
                let field = self.class_fields.alloc(hir::ClassField {
                    owner: backing_class,
                    property: expected_property,
                    ty,
                    source: hir::ClassFieldSource::Body,
                    span: declaration.span,
                });
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
                let field = self.class_fields.alloc(hir::ClassField {
                    owner: backing_class,
                    property: expected_property,
                    ty,
                    source: hir::ClassFieldSource::Body,
                    span: declaration.span,
                });
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
            override_access: Vec::new(),
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

    pub(crate) fn allocate_value_property(
        &mut self,
        owner: Owner,
        declaration: &ast::PropertyDecl,
        ty: TypeId,
        access: hir::DeclarationAccess,
    ) {
        self.reject_logical_property_annotations("a value-type property", &declaration.annotations);
        if declaration.receiver_ty.is_some() || !declaration.type_params.is_empty() {
            self.error(
                declaration.span,
                "extension properties may only be declared at top level".to_string(),
            );
            return;
        }
        if declaration.mutable {
            self.error(
                declaration.name.span,
                format!(
                    "value-type property `{}` cannot be mutable",
                    declaration.name.text
                ),
            );
            return;
        }
        if declaration.modifier != ast::MethodModifier::Final {
            self.error(
                declaration.name.span,
                format!(
                    "value-type property `{}` cannot be open or abstract",
                    declaration.name.text
                ),
            );
            return;
        }
        if !matches!(declaration.body, ast::PropertyBodySyntax::Computed(_)) {
            self.error(
                declaration.span,
                format!(
                    "value-type property `{}` must be a computed val",
                    declaration.name.text
                ),
            );
            return;
        }
        let property_owner = match owner {
            Owner::Struct(owner) => hir::PropertyOwner::Struct(owner),
            Owner::Enum(owner) => hir::PropertyOwner::Enum(owner),
            Owner::Class(_) | Owner::Interface(_) => {
                unreachable!("value property owners are structs or enums")
            }
            Owner::Object(_) => unreachable!("an object is a reference-type property owner"),
        };
        let expected = self.next_property_id();
        let Some(capability) = self.allocate_property_accessors(
            expected,
            property_owner,
            access.clone(),
            declaration,
            None,
            hir::MethodModifier::Final,
        ) else {
            return;
        };
        let property = self.properties.alloc(hir::Property {
            owner: property_owner,
            name: declaration.name.text.clone(),
            access,
            modifier: hir::MethodModifier::Final,
            is_override: declaration.is_override,
            overrides: Vec::new(),
            override_access: Vec::new(),
            ty,
            capability,
            representation: hir::PropertyRepresentation::AccessorOnly,
            span: declaration.span,
        });
        assert_eq!(property, expected);
        match owner {
            Owner::Struct(owner) => self.structs[owner].properties.push(property),
            Owner::Enum(owner) => self.enums[owner].properties.push(property),
            Owner::Class(_) | Owner::Interface(_) => {
                unreachable!("value property owners are structs or enums")
            }
            Owner::Object(_) => unreachable!("an object is a reference-type property owner"),
        }
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

    pub(crate) fn allocate_storage_capability(
        &mut self,
        access: hir::DeclarationAccess,
        mutable: bool,
        span: ast::Span,
    ) -> hir::PropertyCapability {
        let getter = self.property_getters.alloc(hir::PropertyGetter {
            access: access.clone(),
            implementation: hir::PropertyAccessorImplementation::Storage,
            attributes: hir::FunctionAttributes::default(),
            span,
        });
        if !mutable {
            return hir::PropertyCapability::ReadOnly { getter };
        }
        let setter = self.property_setters.alloc(hir::PropertySetter {
            access,
            implementation: hir::PropertyAccessorImplementation::Storage,
            attributes: hir::FunctionAttributes::default(),
            parameter_name: "value".to_string(),
            span,
        });
        hir::PropertyCapability::ReadWrite { getter, setter }
    }

    pub(crate) fn allocate_const_capability(
        &mut self,
        access: hir::DeclarationAccess,
        span: ast::Span,
    ) -> hir::PropertyCapability {
        let getter = self.property_getters.alloc(hir::PropertyGetter {
            access,
            implementation: hir::PropertyAccessorImplementation::Constant,
            attributes: hir::FunctionAttributes::default(),
            span,
        });
        hir::PropertyCapability::ReadOnly { getter }
    }

    fn allocate_delegated_property_accessors(
        &mut self,
        property: hir::PropertyId,
        owner: hir::PropertyOwner,
        access: hir::DeclarationAccess,
        declaration: &ast::PropertyDecl,
        modifier: hir::MethodModifier,
    ) -> hir::PropertyCapability {
        let getter_declaration = self.implicit_getter_function_declaration(
            declaration,
            ast::AccessorBodySyntax::Expr(Box::new(ast::Expr::UnitLiteral {
                span: declaration.span,
            })),
        );
        let getter = self.allocate_accessor_function(
            getter_declaration,
            AccessorFunctionAllocation {
                property,
                kind: PropertyAccessorKind::Getter,
                owner,
                access: access.clone(),
                backing: None,
                modifier,
            },
        );
        self.property_accessor_sources
            .last_mut()
            .expect("the generated getter source was recorded")
            .generated_delegate = true;
        let getter = self.property_getters.alloc(hir::PropertyGetter {
            access: access.clone(),
            implementation: hir::PropertyAccessorImplementation::Body(getter),
            attributes: hir::FunctionAttributes::default(),
            span: declaration.span,
        });
        if !declaration.mutable {
            return hir::PropertyCapability::ReadOnly { getter };
        }

        let setter_access = self.property_setter_access(declaration, None, owner, access);
        let setter_declaration = self.implicit_setter_function_declaration(
            declaration,
            ast::AccessorBodySyntax::Block(ast::Block {
                statements: Vec::new(),
                span: declaration.span,
            }),
        );
        let setter_function = self.allocate_accessor_function(
            setter_declaration,
            AccessorFunctionAllocation {
                property,
                kind: PropertyAccessorKind::Setter,
                owner,
                access: setter_access.clone(),
                backing: None,
                modifier,
            },
        );
        self.property_accessor_sources
            .last_mut()
            .expect("the generated setter source was recorded")
            .generated_delegate = true;
        let setter = self.property_setters.alloc(hir::PropertySetter {
            access: setter_access,
            implementation: hir::PropertyAccessorImplementation::Body(setter_function),
            attributes: hir::FunctionAttributes::default(),
            parameter_name: "value".to_string(),
            span: declaration.span,
        });
        hir::PropertyCapability::ReadWrite { getter, setter }
    }

    fn allocate_accessor_function(
        &mut self,
        declaration: ast::FunctionDecl,
        allocation: AccessorFunctionAllocation,
    ) -> hir::FunctionId {
        let AccessorFunctionAllocation {
            property,
            kind,
            owner,
            access,
            backing,
            modifier,
        } = allocation;
        let modifier = if access.declared == hir::DeclaredVisibility::Private {
            hir::MethodModifier::Final
        } else {
            modifier
        };
        let method_owner = match owner {
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => None,
            hir::PropertyOwner::Class(owner) => Some(Owner::Class(owner)),
            hir::PropertyOwner::Struct(owner) => Some(Owner::Struct(owner)),
            hir::PropertyOwner::Enum(owner) => Some(Owner::Enum(owner)),
            hir::PropertyOwner::Interface(owner) => Some(Owner::Interface(owner)),
            hir::PropertyOwner::Object(owner) => Some(Owner::Object(owner)),
        };
        let checked = self.check_function_annotations(
            &declaration,
            method_owner.map_or(FunctionTarget::TopLevel, FunctionTarget::Member),
        );
        let method = method_owner.map(|owner| hir::Method {
            owner: self.owner_ty(owner),
            modifier,
            dispatch: hir::MethodDispatch::Direct,
        });
        let name = method_owner.map_or_else(
            || declaration.name.text.clone(),
            |owner| format!("{}.{}", owner.describe_name(self), declaration.name.text),
        );
        let function = self.functions.alloc(Function {
            name,
            access,
            override_access: Vec::new(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            modifiers: hir::CallableModifiers::default(),
            params: Vec::new(),
            return_ty: self.unit,
            attributes: checked.attributes,
            kind: FunctionKind::User(hir::Body {
                locals: la_arena::Arena::new(),
                statements: Vec::new(),
            }),
            method,
            span: declaration.span,
        });
        if let Some(owner) = method_owner {
            self.function_owner.insert(function, owner);
            match owner {
                Owner::Class(owner) => self.classes[owner].methods.push(function),
                Owner::Struct(owner) => self.structs[owner].methods.push(function),
                Owner::Enum(owner) => self.enums[owner].methods.push(function),
                Owner::Interface(owner) => {
                    self.interface_methods
                        .get_mut(&owner)
                        .expect("the interface owner map is initialized")
                        .push(function);
                    if self.functions[function].access.declared != hir::DeclaredVisibility::Private
                    {
                        let implementation = if modifier == hir::MethodModifier::Abstract {
                            hir::InterfaceMemberImplementation::AbstractSlot
                        } else {
                            hir::InterfaceMemberImplementation::Body
                        };
                        let member = self.interface_method_entities.alloc(hir::InterfaceMethod {
                            owner,
                            function,
                            role: match kind {
                                PropertyAccessorKind::Getter => {
                                    hir::InterfaceMemberRole::PropertyGetter(property)
                                }
                                PropertyAccessorKind::Setter => {
                                    hir::InterfaceMemberRole::PropertySetter(property)
                                }
                            },
                            implementation,
                            overrides: Vec::new(),
                        });
                        self.functions[function]
                            .method
                            .as_mut()
                            .expect("an interface accessor is a method")
                            .dispatch = hir::MethodDispatch::Interface(member);
                        self.interfaces[owner].methods.push(member);
                    } else {
                        self.interfaces[owner].private_methods.push(function);
                    }
                }
                Owner::Object(owner) => {
                    let backing = self.objects[owner].backing_class;
                    self.classes[backing].methods.push(function);
                }
            }
        } else {
            self.top_level.push(function);
        }
        self.function_files.insert(function, self.current_file);
        if let Some(hir::PropertyBacking::TopLevelGlobal {
            initialization: hir::TopLevelInitialization::Runtime(unit),
            ..
        }) = backing
        {
            self.runtime_accessor_units.insert(function, unit);
        }
        self.property_accessor_sources.push(PropertyAccessorSource {
            property,
            kind,
            function,
            declaration,
            owner: method_owner,
            backing: backing.and_then(|backing| match backing {
                hir::PropertyBacking::TopLevelGlobal { .. }
                | hir::PropertyBacking::ClassField { .. } => Some(backing),
                hir::PropertyBacking::StructField { .. } => None,
            }),
            generated_delegate: false,
        });
        function
    }

    pub(crate) fn resolve_property_accessor_signatures(&mut self) {
        for source in self.property_accessor_sources.clone() {
            self.current_file = self.function_files[&source.function];
            match source.owner {
                Some(owner) => {
                    self.resolve_method_signature(source.function, &source.declaration, owner)
                }
                None => self.resolve_signature(source.function, &source.declaration),
            }
        }
    }

    pub(crate) fn lower_property_accessor_bodies(&mut self) {
        for source in self.property_accessor_sources.clone() {
            if self.functions[source.function]
                .method
                .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
            {
                continue;
            }
            self.current_file = self.function_files[&source.function];
            if source.generated_delegate {
                let mut body = self.lower_generated_delegate_accessor(&source);
                self.prepend_accessor_initialization_ensure(&source, &mut body);
                self.functions[source.function].kind = FunctionKind::User(body);
                continue;
            }
            self.backing_field_context = source.backing.map(|backing| BackingFieldContext {
                backing,
                capture_depth: self.capture_contexts.len(),
            });
            let mut body = self.lower_body(source.function, &source.declaration);
            self.prepend_accessor_initialization_ensure(&source, &mut body);
            self.functions[source.function].kind = FunctionKind::User(body);
            self.backing_field_context = None;
        }
    }

    fn prepend_accessor_initialization_ensure(
        &self,
        source: &PropertyAccessorSource,
        body: &mut hir::Body,
    ) {
        let Some(&unit) = self.runtime_accessor_units.get(&source.function) else {
            return;
        };
        body.statements.insert(
            0,
            hir::Statement {
                kind: hir::StatementKind::InitializationEnsure(unit),
                span: source.declaration.span,
            },
        );
    }

    fn getter_function_declaration(
        &self,
        property: &ast::PropertyDecl,
        getter: &ast::GetterDecl,
    ) -> ast::FunctionDecl {
        ast::FunctionDecl {
            annotations: getter.annotations.clone(),
            visibility: property.visibility,
            is_suspend: false,
            is_override: property.is_override,
            operator: None,
            infix: None,
            modifier: property.modifier,
            receiver_ty: property.receiver_ty.clone(),
            name: ast::Ident {
                text: format!("$get${}", property.name.text),
                span: property.name.span,
            },
            type_params: property.type_params.clone(),
            params: Vec::new(),
            return_ty: Some(property.ty.clone()),
            where_clause: property.where_clause.clone(),
            body: accessor_function_body(&getter.body),
            span: getter.span,
        }
    }

    fn implicit_getter_function_declaration(
        &self,
        property: &ast::PropertyDecl,
        body: ast::AccessorBodySyntax,
    ) -> ast::FunctionDecl {
        self.getter_function_declaration(
            property,
            &ast::GetterDecl {
                annotations: Vec::new(),
                body,
                span: property.span,
            },
        )
    }

    fn setter_function_declaration(
        &self,
        property: &ast::PropertyDecl,
        setter: &ast::SetterDecl,
    ) -> ast::FunctionDecl {
        let name = match &setter.parameter {
            ast::SetterParameterSyntax::Default { span } => ast::Ident {
                text: "value".to_string(),
                span: *span,
            },
            ast::SetterParameterSyntax::Named(name) => name.clone(),
        };
        ast::FunctionDecl {
            annotations: setter.annotations.clone(),
            visibility: match setter.visibility {
                ast::SetterVisibilitySyntax::Explicit { visibility, span } => {
                    ast::VisibilitySyntax::Explicit { visibility, span }
                }
                ast::SetterVisibilitySyntax::Inherited => property.visibility,
            },
            is_suspend: false,
            is_override: property.is_override,
            operator: None,
            infix: None,
            modifier: property.modifier,
            receiver_ty: property.receiver_ty.clone(),
            name: ast::Ident {
                text: format!("$set${}", property.name.text),
                span: property.name.span,
            },
            type_params: property.type_params.clone(),
            params: vec![ast::Param {
                name,
                ty: property.ty.clone(),
                syntax: ast::ParameterSyntax::Required,
                span: setter.span,
            }],
            return_ty: None,
            where_clause: property.where_clause.clone(),
            body: accessor_function_body(&setter.body),
            span: setter.span,
        }
    }

    fn implicit_setter_function_declaration(
        &self,
        property: &ast::PropertyDecl,
        body: ast::AccessorBodySyntax,
    ) -> ast::FunctionDecl {
        self.setter_function_declaration(
            property,
            &ast::SetterDecl {
                annotations: Vec::new(),
                visibility: ast::SetterVisibilitySyntax::Inherited,
                parameter: ast::SetterParameterSyntax::Default {
                    span: property.name.span,
                },
                body,
                span: property.span,
            },
        )
    }
}

fn accessor_function_body(body: &ast::AccessorBodySyntax) -> ast::FunctionBody {
    match body {
        ast::AccessorBodySyntax::Block(block) => ast::FunctionBody::Block(block.clone()),
        ast::AccessorBodySyntax::Expr(expression) => ast::FunctionBody::Expr(expression.clone()),
        ast::AccessorBodySyntax::Omitted => ast::FunctionBody::None,
    }
}
