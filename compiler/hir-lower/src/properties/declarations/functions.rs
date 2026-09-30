use super::*;

impl Lowerer {
    pub(super) fn allocate_accessor_function(
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
            body_kind: PropertyAccessorBodyKind::Source,
        });
        function
    }

    pub(crate) fn resolve_property_accessor_signatures(&mut self) {
        for source in self.property_accessor_sources.clone() {
            self.current_file = self.function_files[&source.function];
            if source.body_kind == PropertyAccessorBodyKind::Storage
                && let Some(hir::PropertyBacking::TopLevelGlobal {
                    storage,
                    initialization,
                }) = source.backing
            {
                self.resolve_top_level_storage_accessor_signature(
                    &source,
                    self.globals[storage].ty,
                    initialization,
                );
                continue;
            }
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
            if source.body_kind == PropertyAccessorBodyKind::Delegate {
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
        &mut self,
        source: &PropertyAccessorSource,
        body: &mut hir::Body,
    ) {
        let Some(&unit) = self.runtime_accessor_units.get(&source.function) else {
            return;
        };
        let kind = match self.initialization_units[unit].kind {
            hir::InitializationUnitKind::GenericDelegatedExtension { template, .. } => {
                hir::StatementKind::GenericDelegateEnsure(
                    self.generic_delegate_reference(template, source.function),
                )
            }
            hir::InitializationUnitKind::EagerTopLevel { .. }
            | hir::InitializationUnitKind::LazySingleton { .. } => {
                hir::StatementKind::InitializationEnsure(unit)
            }
        };
        body.statements.insert(
            0,
            hir::Statement {
                kind,
                span: source.declaration.span,
            },
        );
    }

    pub(super) fn getter_function_declaration(
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

    pub(super) fn implicit_getter_function_declaration(
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

    pub(super) fn setter_function_declaration(
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

    pub(super) fn implicit_setter_function_declaration(
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
