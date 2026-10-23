use super::*;

impl Lowerer {
    pub(super) fn lower_common_initialization(
        &mut self,
        owner: ClassId,
        members: &[ast::ClassMember],
        constructor: hir::ClassConstructorId,
        primary_parameters_visible: bool,
        host: &str,
    ) -> Vec<hir::ClassInitializationStep> {
        let mut steps = Vec::new();
        for (member_index, member) in members.iter().enumerate() {
            if self.body_cancelled() {
                break;
            }
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
                    if matches!(
                        property.body,
                        ast::PropertyBodySyntax::Initializer { .. }
                            | ast::PropertyBodySyntax::Delegated { .. }
                    ) && self.infer_property_type(property_id).is_none()
                    {
                        continue;
                    }
                    if let Some(step) = self
                        .declaration_inference
                        .field_initializers
                        .remove(&property_id)
                    {
                        if let hir::ClassInitializationStep::Field { .. } = &step
                            && let Some(crate::InitializationContext {
                                receiver: InitializingReceiver::Class { initialized, .. },
                                ..
                            }) = &mut self.initialization_context
                        {
                            initialized.extend(self.classes[owner].fields.iter().copied().filter(
                                |field| {
                                    self.class_fields[*field].source.property() == Some(property_id)
                                },
                            ));
                        }
                        steps.push(step);
                        continue;
                    }
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
                        context.step =
                            format!("initializer of property `{}` in {host}", property.name.text);
                    }
                    let expected = logical.ty;
                    let initializer = match initializer {
                        hir::ClassPropertyInitializer::Expression => {
                            let Some(property_initializer) = property.initializer() else {
                                continue;
                            };
                            self.with_constructor_source_body(
                                constructor,
                                hir::EditingConstructionBody::Property(property_id),
                                "stored property initializer",
                                hir::Safety::Safe,
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
                        steps.push(hir::ClassInitializationStep::Field {
                            field: self.initializing_class_field_reference(
                                self.classes[owner].self_application,
                                field,
                            ),
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
                        context.step = format!("init block in {host}");
                    }
                    let lowered = self.with_constructor_source_body(
                        constructor,
                        hir::EditingConstructionBody::InitBlock {
                            owner,
                            member: member_index
                                .try_into()
                                .expect("source member index fits u32"),
                        },
                        "init block",
                        hir::Safety::Safe,
                        |this, _| {
                            if !primary_parameters_visible {
                                this.constructor_params_in_scope.clear();
                            }
                            Some(this.lower_block(&init.body))
                        },
                    );
                    if let Some(lowered) = lowered {
                        steps.push(hir::ClassInitializationStep::InitBlock {
                            body: hir::Body {
                                locals: lowered.locals,
                                statements: lowered
                                    .statements
                                    .into_iter()
                                    .chain(lowered.value)
                                    .collect(),
                            },
                            span: init.span,
                        });
                    }
                }
                ast::ClassMember::SecondaryConstructor(_)
                | ast::ClassMember::InterfaceImplementation(_)
                | ast::ClassMember::Function(_)
                | ast::ClassMember::Nested(_)
                | ast::ClassMember::Companion(_)
                | ast::ClassMember::ReleaseBlock(_) => {}
            }
        }
        steps
    }

    pub(crate) fn lower_class_delegate_initialization(
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
        let lower = |this: &mut Self, sink: &mut Vec<hir::Statement>| {
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
        };
        let lowered = if property.ty.is_none() {
            self.with_property_initializer_context(
                constructor,
                property_id,
                "delegated property initializer",
                lower,
            )
        } else {
            self.with_constructor_source_body(
                constructor,
                hir::EditingConstructionBody::Property(property_id),
                "delegated property initializer",
                hir::Safety::Safe,
                lower,
            )
        };
        let lowered = lowered?;

        let effective_ty = lowered.value.ty;
        let field = self.allocate_class_field(
            owner,
            hir::Field {
                name: property.name.text.clone(),
                ty: effective_ty,
            },
            hir::ClassFieldSource::BodyProperty(property_id),
            property.span,
        );
        self.classes[owner].fields.push(field);
        let storage = self.delegate_storages.alloc(hir::DelegateStorage {
            property: property_id,
            ty: effective_ty,
            location: hir::DelegateStorageLocation::ClassField(field),
        });
        self.properties[property_id].representation =
            hir::PropertyRepresentation::Delegated { storage };
        let step = hir::ClassInitializationStep::Field {
            field: self
                .initializing_class_field_reference(self.classes[owner].self_application, field),
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
}
