use super::*;

impl Lowerer {
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
        // Templates retain the original accessor target for every top-level
        // property, including hidden storage with an encoded static value.
        let top_level_storage =
            matches!(backing, Some(hir::PropertyBacking::TopLevelGlobal { .. }));
        let exported_storage = backing.is_some() && Self::declaration_is_exported(&access);
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
                && (top_level_storage
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
                self.property_accessor_sources
                    .last_mut()
                    .expect("the implicit getter source was recorded")
                    .body_kind = PropertyAccessorBodyKind::Storage;
                (
                    hir::PropertyAccessorImplementation::StorageBody(function),
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
                && (top_level_storage
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
                self.property_accessor_sources
                    .last_mut()
                    .expect("the implicit setter source was recorded")
                    .body_kind = PropertyAccessorBodyKind::Storage;
                (
                    hir::PropertyAccessorImplementation::StorageBody(function),
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
            .body_kind = PropertyAccessorBodyKind::Delegate;
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
            .body_kind = PropertyAccessorBodyKind::Delegate;
        let setter = self.property_setters.alloc(hir::PropertySetter {
            access: setter_access,
            implementation: hir::PropertyAccessorImplementation::Body(setter_function),
            attributes: hir::FunctionAttributes::default(),
            parameter_name: "value".to_string(),
            span: declaration.span,
        });
        hir::PropertyCapability::ReadWrite { getter, setter }
    }
}
