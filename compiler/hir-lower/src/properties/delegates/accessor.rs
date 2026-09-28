//! Synthetic delegate operands share the actual accessor evaluation context.

use super::*;

impl Lowerer {
    pub(in crate::properties) fn lower_generated_delegate_accessor(
        &mut self,
        source: &PropertyAccessorSource,
    ) -> hir::Body {
        let mut declaration = source.declaration.clone();
        declaration.body = ast::FunctionBody::None;
        let body = self.lower_body(source.function, &declaration);
        let outer_source_context = self.current_source_context;
        self.set_source_context(hir::SourceContextSubject::Function(source.function));
        let body = self.generated_delegate_accessor_body(source, body);
        self.current_source_context = outer_source_context;
        body
    }

    fn generated_delegate_accessor_body(
        &mut self,
        source: &PropertyAccessorSource,
        mut body: hir::Body,
    ) -> hir::Body {
        let property = self.properties[source.property].clone();
        let parameters = self.functions[source.function].params.clone();
        let this_ref = match property.owner {
            hir::PropertyOwner::TopLevel => self.unit_delegate_argument(property.span),
            hir::PropertyOwner::Extension(_)
            | hir::PropertyOwner::Class(_)
            | hir::PropertyOwner::Object(_) => {
                let Some(this_parameter) = parameters.first() else {
                    self.error(
                        property.span,
                        "a receiver delegated-property accessor requires an instance receiver"
                            .to_string(),
                    );
                    return body;
                };
                hir::Expr {
                    kind: hir::ExprKind::Local(this_parameter.local),
                    ty: this_parameter.ty,
                    span: property.span,
                    origin: self.expression_origin(property.span),
                }
            }
            hir::PropertyOwner::Struct(_)
            | hir::PropertyOwner::Enum(_)
            | hir::PropertyOwner::Interface(_) => {
                unreachable!("value types and interfaces cannot own delegated storage")
            }
        };
        let storage_read = match property.representation {
            hir::PropertyRepresentation::GenericDelegated { template } => hir::Expr {
                kind: hir::ExprKind::GenericDelegateStorageRead(
                    self.generic_delegate_reference(template),
                ),
                ty: self.generic_delegate_templates[template].ty,
                span: property.span,
                origin: self.expression_origin(property.span),
            },
            hir::PropertyRepresentation::Delegated { storage } => {
                let delegate = self.delegate_storages[storage].clone();
                match delegate.location {
                    hir::DelegateStorageLocation::ClassField(field) => {
                        let hir::Type::Class(application) = self.types[this_ref.ty] else {
                            self.error(
                                property.span,
                                "a class delegated-property accessor requires a class receiver"
                                    .to_string(),
                            );
                            return body;
                        };
                        let application_value = self.class_applications[application].clone();
                        let storage_ty =
                            self.instantiate_ty(delegate.ty, &application_value.arguments);
                        hir::Expr {
                            kind: hir::ExprKind::FieldAccess {
                                receiver: Box::new(this_ref.clone()),
                                field: hir::FieldRef::ClassField { application, field },
                            },
                            ty: storage_ty,
                            span: property.span,
                            origin: self.expression_origin(property.span),
                        }
                    }
                    hir::DelegateStorageLocation::ManagedGlobal(global) => hir::Expr {
                        kind: hir::ExprKind::GlobalRead(global),
                        ty: delegate.ty,
                        span: property.span,
                        origin: self.expression_origin(property.span),
                    },
                }
            }
            _ => unreachable!("generated delegate accessors retain a delegate representation"),
        };

        let signature = self.signatures[&source.function].clone();
        let outer_type_params = std::mem::replace(
            &mut self.type_params_in_scope,
            signature.type_params.clone(),
        );
        let outer_return_ty = std::mem::replace(&mut self.current_return_ty, signature.return_ty);
        let accessor_name = self.functions[source.function].name.clone();
        let outer_fn_name = std::mem::replace(&mut self.current_fn_name, accessor_name);
        let outer_owner = std::mem::replace(&mut self.current_owner, source.owner);
        self.push_suspension_context(SuspensionContext::Forbidden(
            ForbiddenSuspendContext::Function,
        ));
        self.push_safety_context(self.functions[source.function].attributes.safety);

        let statements = match source.kind {
            PropertyAccessorKind::Getter => self
                .require_delegate_role_call(
                    storage_read,
                    hir::PropertyDelegateOperatorKind::GetValue,
                    vec![this_ref],
                    property.span,
                )
                .and_then(|value| {
                    self.validate_delegate_get_result(
                        &property.name,
                        value.ty,
                        signature.return_ty,
                        property.span,
                    )
                    .then(|| self.adapt_to(value, signature.return_ty))
                })
                .map_or_else(Vec::new, |value| {
                    if self.types_equal(signature.return_ty, self.unit) {
                        vec![
                            hir::Statement {
                                kind: hir::StatementKind::Expr(value),
                                span: property.span,
                            },
                            hir::Statement {
                                kind: hir::StatementKind::Return { value: None },
                                span: property.span,
                            },
                        ]
                    } else {
                        vec![hir::Statement {
                            kind: hir::StatementKind::Return { value: Some(value) },
                            span: property.span,
                        }]
                    }
                }),
            PropertyAccessorKind::Setter => {
                match if matches!(property.owner, hir::PropertyOwner::TopLevel) {
                    parameters.first()
                } else {
                    parameters.get(1)
                } {
                    Some(value_parameter) => {
                        let value = hir::Expr {
                            kind: hir::ExprKind::Local(value_parameter.local),
                            ty: value_parameter.ty,
                            span: property.span,
                            origin: self.expression_origin(property.span),
                        };
                        self.require_delegate_role_call(
                            storage_read,
                            hir::PropertyDelegateOperatorKind::SetValue,
                            vec![this_ref, value],
                            property.span,
                        )
                        .map_or_else(Vec::new, |call| {
                            vec![
                                hir::Statement {
                                    kind: hir::StatementKind::Expr(call),
                                    span: property.span,
                                },
                                hir::Statement {
                                    kind: hir::StatementKind::Return { value: None },
                                    span: property.span,
                                },
                            ]
                        })
                    }
                    None => {
                        self.error(
                            property.span,
                            "a mutable delegated-property accessor requires a value parameter"
                                .to_string(),
                        );
                        Vec::new()
                    }
                }
            }
        };

        self.pop_safety_context();
        self.pop_suspension_context();
        self.current_owner = outer_owner;
        self.current_fn_name = outer_fn_name;
        self.current_return_ty = outer_return_ty;
        self.type_params_in_scope = outer_type_params;
        body.statements = statements;
        body
    }
}
