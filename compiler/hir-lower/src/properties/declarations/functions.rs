use super::*;

impl Lowerer {
    pub(super) fn property_function_target(owner: hir::PropertyOwner) -> FunctionTarget {
        match owner {
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => {
                FunctionTarget::TopLevel
            }
            hir::PropertyOwner::Class(owner) => FunctionTarget::Member(Owner::Class(owner)),
            hir::PropertyOwner::Struct(owner) => FunctionTarget::Member(Owner::Struct(owner)),
            hir::PropertyOwner::Enum(owner) => FunctionTarget::Member(Owner::Enum(owner)),
            hir::PropertyOwner::Interface(owner) => FunctionTarget::Member(Owner::Interface(owner)),
            hir::PropertyOwner::Object(owner) => FunctionTarget::Member(Owner::Object(owner)),
        }
    }

    pub(crate) fn function_accessor_kind(
        &self,
        function: hir::FunctionId,
    ) -> Option<PropertyAccessorKind> {
        if let Some(forwarder) = self.interface_forwarders.get(&function) {
            return forwarder.accessor();
        }
        self.property_accessor_sources
            .iter()
            .find(|source| source.function == function)
            .map(|source| source.kind)
    }

    pub(crate) fn function_property(&self, function: hir::FunctionId) -> Option<hir::PropertyId> {
        self.interface_forwarders
            .get(&function)
            .and_then(|forwarder| forwarder.property())
            .or_else(|| {
                self.property_accessor_sources
                    .iter()
                    .find(|source| source.function == function)
                    .map(|source| source.property)
            })
    }

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
        let target = Self::property_function_target(owner);
        let method_owner = match target {
            FunctionTarget::Member(owner) => Some(owner),
            FunctionTarget::TopLevel | FunctionTarget::Local => None,
        };
        let checked = self.check_function_annotations(&declaration, target);
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
            signature: hir::CallableSignature {
                context_parameters: Vec::new(),
                release_callability: Default::default(),
                name,
                is_suspend: false,
                modifiers: hir::CallableModifiers::default(),
                params: Vec::new(),
                return_ty: self.unit,
                attributes: checked.attributes,
                span: declaration.span,
            },

            access,
            genericity: hir::FunctionGenericity::Plain,
            kind: FunctionKind::User(hir::Body {
                locals: la_arena::Arena::new(),
                statements: Vec::new(),
            }),
            method,
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
        self.resolve_property_accessor_signatures_from(0);
    }

    pub(crate) fn resolve_property_accessor_signatures_from(&mut self, start: usize) {
        let sources = self.property_accessor_sources[start..].to_vec();
        for source in sources {
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
                self.mark_pending_property_accessor(&source);
                continue;
            }
            match source.owner {
                Some(owner) => {
                    self.resolve_method_signature(source.function, &source.declaration, owner)
                }
                None => self.resolve_signature(source.function, &source.declaration),
            }
            self.mark_pending_property_accessor(&source);
        }
    }

    pub(crate) fn lower_property_accessor_bodies(&mut self) {
        self.lower_property_accessor_bodies_from(0);
    }

    pub(crate) fn lower_property_accessor_bodies_from(&mut self, start: usize) {
        let sources = self.property_accessor_sources[start..].to_vec();
        for source in sources {
            if self.property_accessor_was_lowered(source.function) {
                continue;
            }
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

    pub(crate) fn prepend_accessor_initialization_ensure(
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
            context_parameters: property.context_parameters.clone(),
            annotations: accessor_annotations(
                property,
                &getter.annotations,
                ast::AnnotationUseSiteTarget::Get,
            ),
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
            return_ty: property.ty.clone(),
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
            context_parameters: property.context_parameters.clone(),
            annotations: accessor_annotations(
                property,
                &setter.annotations,
                ast::AnnotationUseSiteTarget::Set,
            ),
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
                annotations: Vec::new(),
                name,
                ty: property.ty.clone().unwrap_or(ast::TypeRef {
                    annotations: Vec::new(),
                    kind: ast::TypeRefKind::Unit,
                    span: property.name.span,
                }),
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

fn accessor_annotations(
    property: &ast::PropertyDecl,
    direct: &[ast::Annotation],
    site: ast::AnnotationUseSiteTarget,
) -> Vec<ast::Annotation> {
    property
        .annotations
        .iter()
        .filter(|a| a.use_site.is_some_and(|s| s.target == site))
        .cloned()
        .map(|mut a| {
            a.use_site = None;
            a
        })
        .chain(direct.iter().cloned())
        .collect()
}
