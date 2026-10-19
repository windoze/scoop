use super::*;

impl Lowerer {
    pub(super) fn inspection_type(&mut self, name: &str, span: Span) -> Option<TypeId> {
        let nominal = self.core_coding_nominal(name).or_else(|| {
            self.error(span, format!("automatic inspect requires core {name}"));
            None
        })?;
        self.apply_nominal_type(nominal, Vec::new())
            .map_err(|error| {
                self.error(span, format!("cannot load core {name}: {error:?}"));
            })
            .ok()
    }

    pub(super) fn inspection_reference(
        &mut self,
        context: CodingContext,
        target: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let CodingDirection::Inspect(factory) = context.direction else {
            unreachable!("inspection has its type reference factory")
        };
        let callee = match factory {
            InspectionTypeFactory::Current(function) => {
                hir::Callable::Generic(self.record_instantiation(function, vec![target])).into()
            }
            InspectionTypeFactory::Imported(template) => {
                hir::CallableTarget::Application(self.record_imported_callable_application(
                    hir::ImportedGenericCallableApplication {
                        requirements: Vec::new(),
                        template,
                        arguments: hir::ImportedCallableArguments::Function(vec![target]),
                    },
                ))
            }
        };
        let ty = self.inspection_type("TypeRef", span)?;
        Some(self.coding_expr(
            hir::ExprKind::Call {
                callee,
                binding: None,
                args: Vec::new(),
                receiver: hir::SourceCallReceiver::NoReceiver,
            },
            ty,
            span,
        ))
    }

    pub(super) fn inspection_construct(
        &mut self,
        name: &str,
        values: Vec<hir::Expr>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let ty = self.inspection_type(name, span)?;
        let record = self.coding_primary(ty, span)?;
        self.call_coding_constructor(ty, &record, values, span, sink)
    }

    pub(super) fn inspection_variant(
        &mut self,
        name: &str,
        variant: &str,
        values: Vec<hir::Expr>,
        span: Span,
    ) -> Option<hir::Expr> {
        let ty = self.inspection_type(name, span)?;
        let variant = self
            .named_enum_variant(ty, variant)
            .expect("core observation variant is declared");
        Some(self.coding_expr(
            hir::ExprKind::VariantConstruct {
                variant,
                args: values,
            },
            ty,
            span,
        ))
    }

    pub(super) fn inspection_bind(
        &mut self,
        name: &str,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let value = self.coding_local(value, span, sink);
        let hir::ExprKind::Local(local) = value.kind else {
            unreachable!("a materialized value is local")
        };
        self.scopes.declare(name.into(), local);
        value
    }

    pub(super) fn inspection_capture(&mut self, name: &str, span: Span) -> Option<hir::Expr> {
        self.lower_expr(
            &ast::Expr::Var(ast::Ident {
                text: name.into(),
                span,
            }),
            &mut Vec::new(),
            None,
        )
    }

    pub(super) fn inspection_field(
        &mut self,
        value: hir::Expr,
        name: &str,
        span: Span,
    ) -> Option<hir::Expr> {
        if let Some(fields) = self.struct_fields(value.ty) {
            let field = fields.fields.into_iter().find(|field| field.name == name)?;
            Some(self.coding_expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(value),
                    field: field.reference,
                },
                field.ty,
                span,
            ))
        } else {
            self.coding_property(value, name, span)
        }
    }

    pub(super) fn inspection_payload(
        &mut self,
        value: hir::Expr,
        name: &str,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let variant = self.named_enum_variant(value.ty, name)?;
        let ty = self.enum_variant(variant).fields[0].1;
        let local = self.alloc_hidden("inspection_metadata", ty);
        let mut failure = Vec::new();
        let message = self.coding_string(
            &format!("unexpected inspection metadata: expected {name}"),
            span,
        );
        let message_ty = self.option_type(message.ty);
        let message =
            self.coding_expr(hir::ExprKind::SomeWrap(Box::new(message)), message_ty, span);
        let error =
            self.inspection_construct("IllegalStateException", vec![message], span, &mut failure)?;
        failure.push(hir::Statement {
            kind: hir::StatementKind::Throw(error),
            span,
        });
        sink.push(hir::Statement {
            kind: hir::StatementKind::When(hir::When::Subject {
                subject: value,
                arms: vec![hir::WhenArm {
                    condition: hir::WhenCondition::Case(hir::Pattern::Variant {
                        application: variant,
                        fields: vec![(0, hir::Pattern::Binding { local })],
                    }),
                    guard: None,
                    body: Vec::new(),
                    span,
                }],
                fallback: hir::WhenFallback::Else(failure),
            }),
            span,
        });
        Some(self.coding_expr(hir::ExprKind::Local(local), ty, span))
    }

    pub(super) fn inspection_shape_info(
        &mut self,
        reference: hir::Expr,
        shape: &str,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let description = self.coding_call(reference, "describe", Vec::new(), span, sink)?;
        let description = self.coding_local(description, span, sink);
        let shape_value = self.inspection_field(description, "shape", span)?;
        self.inspection_payload(shape_value, shape, span, sink)
    }

    pub(super) fn inspection_long(&mut self, value: usize, span: Span) -> hir::Expr {
        let ty = self.intern_type(Type::Integer(hir::IntegerKind::SIGNED_64));
        self.coding_expr(
            hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(value as u64)),
            ty,
            span,
        )
    }

    pub(super) fn inspection_index(
        &mut self,
        array: hir::Expr,
        index: hir::Expr,
        span: Span,
    ) -> hir::Expr {
        let ty = self
            .array_element_ty(array.ty)
            .expect("inspection arrays have an element type");
        self.coding_expr(
            hir::ExprKind::Index {
                access: hir::ArrayAccessKind::ImmutableGet,
                receiver: Box::new(array),
                index: Box::new(index),
            },
            ty,
            span,
        )
    }

    pub(super) fn inspection_array(
        &mut self,
        element: TypeId,
        values: Vec<hir::Expr>,
        span: Span,
    ) -> hir::Expr {
        let ty = self.array_type(crate::types::ArrayKind::Immutable, element);
        self.coding_expr(hir::ExprKind::ArrayLiteral(values), ty, span)
    }
}
