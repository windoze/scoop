use super::*;

use crate::InitializingReceiver;

pub(crate) struct InitializingField {
    pub(crate) read: hir::Expr,
    pub(crate) write: Option<hir::AssignTarget>,
}

impl Lowerer {
    pub(crate) fn lower_constructor_initialization(
        &mut self,
        classes: &[(ClassId, &ast::ClassDecl, usize)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
    ) {
        for &(class, declaration, file) in classes {
            self.current_file = file;
            self.lower_class_initialization(class, declaration);
        }
        for &(structure, declaration, file) in structs {
            self.current_file = file;
            self.lower_struct_constructor_bodies(structure, declaration);
        }
    }

    fn lower_class_initialization(&mut self, owner: ClassId, declaration: &ast::ClassDecl) {
        if !self.classes[owner].is_declared() {
            return;
        }
        let constructors = self.classes[owner].constructors.clone();
        let primary = constructors.iter().copied().find(|constructor| {
            matches!(
                self.class_constructors[*constructor].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        });
        let context_constructor = primary.or_else(|| constructors.first().copied());
        let Some(context_constructor) = context_constructor else {
            return;
        };

        let application = self.classes[owner].self_application;
        let mut initialized = self.inherited_fields(owner);
        let mut stores = Vec::new();
        if primary.is_some() {
            for &field in &self.classes[owner].fields {
                let declaration = &self.class_fields[field];
                if let hir::ClassFieldSource::PrimaryParameter(parameter) = declaration.source {
                    stores.push(hir::PrimaryFieldStore {
                        field,
                        parameter,
                        span: declaration.span,
                    });
                    initialized.insert(field);
                }
            }
        }
        self.initialization_context = Some(crate::InitializationContext {
            receiver: InitializingReceiver::Class {
                application,
                initialized,
            },
            step: format!("initialization of class `{}`", self.classes[owner].name),
            capture_depth: self.capture_contexts.len(),
        });

        let common = self.lower_common_initialization(
            owner,
            declaration,
            context_constructor,
            primary.is_some(),
        );

        if let Some(primary) = primary {
            let hir::ClassConstructorKind::Primary {
                primary_stores,
                common_initialization,
                ..
            } = &mut self.class_constructors[primary].kind
            else {
                unreachable!("the selected constructor is primary")
            };
            *primary_stores = stores;
            *common_initialization = common.clone();
        }
        for &constructor in &constructors {
            let hir::ClassConstructorKind::Secondary { delegation, .. } =
                &mut self.class_constructors[constructor].kind
            else {
                continue;
            };
            if let hir::ClassSecondaryDelegation::Terminal {
                common_initialization,
                ..
            } = delegation
            {
                *common_initialization = common.clone();
            }
        }

        let all_initialized = self.all_class_fields(owner);
        let secondary = constructors
            .iter()
            .copied()
            .filter(|constructor| {
                matches!(
                    self.class_constructors[*constructor].kind,
                    hir::ClassConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary
            .into_iter()
            .zip(declaration.secondary_constructors())
        {
            self.initialization_context = Some(crate::InitializationContext {
                receiver: InitializingReceiver::Class {
                    application,
                    initialized: all_initialized.clone(),
                },
                step: format!(
                    "secondary constructor body `{}`",
                    self.constructor_display_name(constructor)
                ),
                capture_depth: self.capture_contexts.len(),
            });
            let lowered = self.with_constructor_expression_context(
                constructor,
                "secondary constructor body",
                |this, _| Some(this.lower_block(&source.body)),
            );
            if let Some(lowered) = lowered {
                debug_assert!(lowered.statements.is_empty());
                let hir::ClassConstructorKind::Secondary { body, .. } =
                    &mut self.class_constructors[constructor].kind
                else {
                    unreachable!("the selected constructor is secondary")
                };
                *body = hir::Body {
                    locals: lowered.locals,
                    statements: lowered.value,
                };
            }
        }
        self.initialization_context = None;
    }

    fn lower_common_initialization(
        &mut self,
        owner: ClassId,
        declaration: &ast::ClassDecl,
        constructor: hir::ClassConstructorId,
        primary_parameters_visible: bool,
    ) -> Vec<hir::ClassInitializationStep> {
        let mut steps = Vec::new();
        for member in &declaration.members {
            match member {
                ast::ClassMember::StoredProperty(property) => {
                    let Some(property_id) = self.classes[owner]
                        .properties
                        .iter()
                        .copied()
                        .find(|candidate| self.properties[*candidate].name == property.name.text)
                    else {
                        continue;
                    };
                    let logical = self.properties[property_id].clone();
                    if let ast::PropertyBodySyntax::Delegated { expression, .. } = &property.body {
                        if let Some(step) = self.lower_class_delegate_initialization(
                            owner,
                            constructor,
                            primary_parameters_visible,
                            property_id,
                            property,
                            expression,
                        ) {
                            steps.push(step);
                        }
                        continue;
                    }
                    let hir::PropertyRepresentation::Stored(stored) = logical.representation else {
                        continue;
                    };
                    let hir::PropertyBacking::ClassField { field, initializer } = stored.backing
                    else {
                        continue;
                    };
                    if let Some(context) = &mut self.initialization_context {
                        context.step = format!(
                            "initializer of field `{}` in class `{}`",
                            property.name.text, self.classes[owner].name
                        );
                    }
                    let expected = logical.ty;
                    let initializer = match initializer {
                        hir::ClassPropertyInitializer::Expression => {
                            let Some(property_initializer) = property.initializer() else {
                                continue;
                            };
                            self.with_constructor_expression_context(
                                constructor,
                                "stored property initializer",
                                |this, sink| {
                                    if !primary_parameters_visible {
                                        this.constructor_params_in_scope.clear();
                                    }
                                    let value = this.lower_expr(
                                        property_initializer,
                                        sink,
                                        Some(expected),
                                    )?;
                                    if !this.is_subtype(value.ty, expected) {
                                        let message = this.with_nominal_invariance_detail(
                                            format!(
                                                "initializer of property `{}` must be of type {}, found {}",
                                                property.name.text,
                                                this.type_name(expected),
                                                this.type_name(value.ty)
                                            ),
                                            value.ty,
                                            expected,
                                        );
                                        this.error(property_initializer.span(), message);
                                        return None;
                                    }
                                    Some(this.adapt_to(value, expected))
                                },
                            )
                            .map(|lowered| hir::ConstructorExpression {
                                locals: lowered.locals,
                                statements: lowered.statements,
                                value: lowered.value,
                            })
                        }
                        hir::ClassPropertyInitializer::SyntheticNone => {
                            Some(hir::ConstructorExpression {
                                locals: la_arena::Arena::new(),
                                statements: Vec::new(),
                                value: hir::Expr {
                                    kind: hir::ExprKind::NoneLiteral,
                                    ty: expected,
                                    span: property.span,
                                    origin: self.expression_origin(property.span),
                                },
                            })
                        }
                        hir::ClassPropertyInitializer::PrimaryParameter(_) => None,
                    };
                    if let Some(initializer) = initializer {
                        steps.push(hir::ClassInitializationStep::StoredProperty {
                            field,
                            initializer,
                            span: property.span,
                        });
                        if let Some(crate::InitializationContext {
                            receiver: InitializingReceiver::Class { initialized, .. },
                            ..
                        }) = &mut self.initialization_context
                        {
                            initialized.insert(field);
                        }
                    }
                }
                ast::ClassMember::InitBlock(init) => {
                    if let Some(context) = &mut self.initialization_context {
                        context.step =
                            format!("init block in class `{}`", self.classes[owner].name);
                    }
                    let lowered = self.with_constructor_expression_context(
                        constructor,
                        "init block",
                        |this, _| {
                            if !primary_parameters_visible {
                                this.constructor_params_in_scope.clear();
                            }
                            Some(this.lower_block(&init.body))
                        },
                    );
                    if let Some(lowered) = lowered {
                        debug_assert!(lowered.statements.is_empty());
                        steps.push(hir::ClassInitializationStep::InitBlock {
                            body: hir::Body {
                                locals: lowered.locals,
                                statements: lowered.value,
                            },
                            span: init.span,
                        });
                    }
                }
                ast::ClassMember::SecondaryConstructor(_)
                | ast::ClassMember::Function(_)
                | ast::ClassMember::Nested(_)
                | ast::ClassMember::Companion(_) => {}
            }
        }
        steps
    }

    fn lower_class_delegate_initialization(
        &mut self,
        owner: ClassId,
        constructor: hir::ClassConstructorId,
        primary_parameters_visible: bool,
        property_id: hir::PropertyId,
        property: &ast::PropertyDecl,
        expression: &ast::Expr,
    ) -> Option<hir::ClassInitializationStep> {
        if let Some(context) = &mut self.initialization_context {
            context.step = format!(
                "delegate initializer of property `{}` in class `{}`",
                property.name.text, self.classes[owner].name
            );
        }
        let lowered = self.with_constructor_expression_context(
            constructor,
            "delegated property initializer",
            |this, sink| {
                if !primary_parameters_visible {
                    this.constructor_params_in_scope.clear();
                }
                let delegate = this.lower_expr(expression, sink, None)?;
                match this.resolve_delegate_role_call(
                    delegate.clone(),
                    hir::PropertyDelegateOperatorKind::ProvideDelegate,
                    Vec::new(),
                    property.span,
                ) {
                    crate::properties::DelegateRoleCall::Resolved(effective) => {
                        Some(effective.expression)
                    }
                    crate::properties::DelegateRoleCall::NoApplicable => Some(delegate),
                    crate::properties::DelegateRoleCall::Failed => None,
                }
            },
        );
        let lowered = lowered?;

        let effective_ty = lowered.value.ty;
        let field = self.class_fields.alloc(hir::ClassField {
            owner,
            property: property_id,
            ty: effective_ty,
            source: hir::ClassFieldSource::Body,
            span: property.span,
        });
        self.classes[owner].fields.push(field);
        let storage = self.delegate_storages.alloc(hir::DelegateStorage {
            property: property_id,
            ty: effective_ty,
            location: hir::DelegateStorageLocation::ClassField(field),
        });
        self.properties[property_id].representation =
            hir::PropertyRepresentation::Delegated { storage };
        let step = hir::ClassInitializationStep::DelegatedProperty {
            storage,
            field,
            initializer: hir::ConstructorExpression {
                locals: lowered.locals,
                statements: lowered.statements,
                value: lowered.value,
            },
            span: property.span,
        };
        if let Some(crate::InitializationContext {
            receiver: InitializingReceiver::Class { initialized, .. },
            ..
        }) = &mut self.initialization_context
        {
            initialized.insert(field);
        }
        Some(step)
    }

    fn lower_struct_constructor_bodies(
        &mut self,
        owner: hir::StructId,
        declaration: &ast::StructDecl,
    ) {
        if !matches!(
            self.structs[owner].representation,
            hir::StructRepresentation::Declared(_)
        ) {
            return;
        }
        let application = self.structs[owner].self_application;
        let secondary = self.structs[owner]
            .constructors
            .iter()
            .copied()
            .filter(|constructor| {
                matches!(
                    self.struct_constructors[*constructor].kind,
                    hir::StructConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary
            .into_iter()
            .zip(declaration.secondary_constructors())
        {
            self.initialization_context = Some(crate::InitializationContext {
                receiver: InitializingReceiver::Struct { application },
                step: format!(
                    "secondary constructor body of `{}`",
                    self.structs[owner].name
                ),
                capture_depth: self.capture_contexts.len(),
            });
            let lowered = self.with_constructor_expression_context(
                constructor,
                "struct secondary constructor body",
                |this, _| Some(this.lower_block(&source.body)),
            );
            if let Some(lowered) = lowered {
                debug_assert!(lowered.statements.is_empty());
                let hir::StructConstructorKind::Secondary { body, .. } =
                    &mut self.struct_constructors[constructor].kind
                else {
                    unreachable!("the selected constructor is secondary")
                };
                *body = hir::Body {
                    locals: lowered.locals,
                    statements: lowered.value,
                };
            }
        }
        self.initialization_context = None;
    }

    fn inherited_fields(&self, owner: ClassId) -> std::collections::HashSet<hir::ClassFieldId> {
        let mut fields = std::collections::HashSet::new();
        let mut current = self.direct_base_class(owner);
        let mut seen = std::collections::HashSet::new();
        while let Some(class) = current {
            if !seen.insert(class) {
                break;
            }
            fields.extend(self.classes[class].fields.iter().copied());
            current = self.direct_base_class(class);
        }
        fields
    }

    fn all_class_fields(&self, owner: ClassId) -> std::collections::HashSet<hir::ClassFieldId> {
        let mut fields = self.inherited_fields(owner);
        fields.extend(self.classes[owner].fields.iter().copied());
        fields
    }

    fn constructor_display_name(&mut self, constructor: hir::ClassConstructorId) -> String {
        let declaration = self.class_constructors[constructor].clone();
        let owner = self.classes[declaration.owner].name.clone();
        let parameters = declaration
            .parameters
            .iter()
            .map(|parameter| self.type_name(parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{owner}({parameters})")
    }

    pub(crate) fn initializing_receiver_type(&self) -> Option<TypeId> {
        match &self.initialization_context.as_ref()?.receiver {
            InitializingReceiver::Class { application, .. } => {
                Some(self.class_applications[*application].canonical_type)
            }
            InitializingReceiver::Struct { application } => {
                Some(self.struct_applications[*application].canonical_type)
            }
        }
    }

    pub(crate) fn initializing_field(
        &mut self,
        name: &ast::Ident,
        span: ast::Span,
    ) -> Option<InitializingField> {
        let context = self.initialization_context.clone()?;
        if self.capture_contexts.len() > context.capture_depth {
            self.error(
                span,
                "initializing receiver cannot escape before construction completes".into(),
            );
            return None;
        }
        match context.receiver {
            InitializingReceiver::Class {
                application,
                initialized,
            } => {
                let class = self.class_applications[application].template;
                let Some((declaring, field, ty, mutable)) =
                    self.find_class_application_field(application, &name.text)
                else {
                    self.error(
                        name.span,
                        format!(
                            "class `{}` has no field `{}`",
                            self.classes[class].name, name.text
                        ),
                    );
                    return None;
                };
                if !initialized.contains(&field) {
                    self.error(
                        name.span,
                        format!(
                            "field `{}` is not initialized during {}; initializing receiver cannot observe a field before its store completes",
                            name.text, context.step
                        ),
                    );
                    return None;
                }
                let read = hir::Expr {
                    kind: hir::ExprKind::InitializingClassFieldAccess {
                        application: declaring,
                        field,
                    },
                    ty,
                    span,
                    origin: self.expression_origin(span),
                };
                let write = mutable.then_some(hir::AssignTarget::InitializingClassField {
                    application: declaring,
                    field,
                    origin: self.expression_origin(span),
                });
                Some(InitializingField { read, write })
            }
            InitializingReceiver::Struct { application } => {
                let application_value = self.struct_applications[application].clone();
                let structure = application_value.template;
                let fields = self.structs[structure].semantic_fields();
                let Some(index) = fields.iter().position(|field| field.name == name.text) else {
                    self.error(
                        name.span,
                        format!(
                            "struct `{}` has no field `{}`",
                            self.structs[structure].name, name.text
                        ),
                    );
                    return None;
                };
                let ty = self.instantiate_ty(fields[index].ty, &application_value.arguments);
                Some(InitializingField {
                    read: hir::Expr {
                        kind: hir::ExprKind::InitializingStructFieldAccess {
                            application,
                            index: index as u32,
                        },
                        ty,
                        span,
                        origin: self.expression_origin(span),
                    },
                    write: None,
                })
            }
        }
    }

    pub(crate) fn initializing_receiver_has_field(&mut self, name: &str) -> bool {
        let Some(context) = self.initialization_context.clone() else {
            return false;
        };
        match context.receiver {
            InitializingReceiver::Class { application, .. } => self
                .find_class_application_field(application, name)
                .is_some(),
            InitializingReceiver::Struct { application } => {
                let structure = self.struct_applications[application].template;
                self.structs[structure]
                    .semantic_fields()
                    .iter()
                    .any(|field| field.name == name)
            }
        }
    }

    pub(crate) fn reject_initializing_this(&mut self, span: ast::Span) -> bool {
        if self.initialization_context.is_none() {
            return false;
        }
        self.error(
            span,
            "initializing receiver cannot escape before construction completes".into(),
        );
        true
    }
}
