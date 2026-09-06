use super::*;

impl Lowerer {
    pub(super) fn lower_field_access(
        &mut self,
        access: &ast::FieldAccess,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let direct_alias = match self.resolve_direct_alias_qualifier(&access.receiver) {
            Ok(alias) => alias,
            Err(()) => return None,
        };
        // A qualified nested object is a value only at the final path
        // component. Resolving the owner chain itself is structural and must
        // not initialize any outer object.
        if let Some(crate::NominalTarget::Object(object)) =
            self.nominal_qualifier_target(&ast::Expr::FieldAccess(access.clone()))
        {
            return self.lower_singleton_value(object, access.span);
        }
        if let ast::FieldSelector::Name(name) = &access.selector {
            let property = self.qualified_object_const_property(&access.receiver, &name.text);
            if let Some(property) = property {
                let ty = self.properties[property].ty;
                return self.lower_property_read(property, None, None, ty, access.span);
            }
        }
        // `E.V` where `E` is an enum: a unit variant construction
        // (`Color.Red`). Variants with fields are constructors and must
        // be called (`E.V(...)`).
        let qualifier = direct_alias
            .as_ref()
            .map(|(_, target)| *target)
            .or_else(|| self.nominal_qualifier_target(&access.receiver));
        if let (Some(crate::NominalTarget::Enum(enum_id)), ast::FieldSelector::Name(name)) =
            (qualifier, &access.selector)
            && self.find_variant(enum_id, &name.text).is_some()
        {
            let expected = direct_alias
                .as_ref()
                .map_or(expected, |(alias, _)| Some(alias.target));
            return self.lower_qualified_variant(enum_id, access, expected);
        }
        if let (Some(qualifier), ast::FieldSelector::Name(name)) = (qualifier, &access.selector)
            && let Some(companion) =
                self.companion_forwarding_property_object(qualifier, &name.text)
        {
            let receiver = self.lower_singleton_value(companion, access.receiver.span())?;
            if let Some((property, owner, ty)) =
                self.find_accessible_nominal_property(receiver.ty, &name.text)
            {
                return self.lower_property_read(
                    property,
                    Some(owner),
                    Some(receiver),
                    ty,
                    access.span,
                );
            }
        }
        if let Some(crate::NominalTarget::Enum(enum_id)) = qualifier {
            let expected = direct_alias
                .as_ref()
                .map_or(expected, |(alias, _)| Some(alias.target));
            return self.lower_qualified_variant(enum_id, access, expected);
        }
        if matches!(&*access.receiver, ast::Expr::This { .. })
            && self.initialization_context.is_some()
        {
            let ast::FieldSelector::Name(name) = &access.selector else {
                self.error(
                    access.span,
                    "an initializing receiver only supports direct named field access".into(),
                );
                return None;
            };
            return self
                .initializing_field(name, access.span)
                .map(|field| field.read);
        }
        let receiver = self.lower_expr(&access.receiver, sink, None)?;
        // `array.size` (spec 10.5): the pseudo-property resolves on
        // both array kinds; any other receiver keeps the ordinary
        // field rules (so `.size` on a non-array is the usual unknown
        // field diagnostic).
        if let ast::FieldSelector::Name(field) = &access.selector {
            if field.text == "size" && self.array_element_ty(receiver.ty).is_some() {
                return Some(hir::Expr {
                    kind: ExprKind::ArrayLen(Box::new(receiver)),
                    ty: self.integer_type(hir::IntegerKind::SIGNED_64),
                    span: access.span,
                    origin: self.expression_origin(access.span),
                });
            }
            if let Some((property, owner, ty)) =
                self.find_accessible_nominal_property(receiver.ty, &field.text)
            {
                return self.lower_property_read(
                    property,
                    Some(owner),
                    Some(receiver),
                    ty,
                    access.span,
                );
            }
            match self.resolve_extension_property(receiver.clone(), field, sink, true) {
                crate::properties::ExtensionPropertyResolution::Resolved(property) => {
                    return Some(property.read);
                }
                crate::properties::ExtensionPropertyResolution::Failed => return None,
                crate::properties::ExtensionPropertyResolution::NoCandidate => {}
            }
        }
        let (field, ty) = self.resolve_field(receiver.ty, &access.selector)?;
        Some(hir::Expr {
            kind: ExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field,
            },
            ty,
            span: access.span,
            origin: self.expression_origin(access.span),
        })
    }

    /// `E.V` with `E` an enum (see `lower_field_access`).
    pub(super) fn lower_qualified_variant(
        &mut self,
        enum_id: hir::EnumId,
        access: &ast::FieldAccess,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let enum_name = self.enums[enum_id].name.clone();
        let ast::FieldSelector::Name(variant_name) = &access.selector else {
            self.error(
                access.span,
                format!("enum `{enum_name}` has no variants selected by index"),
            );
            return None;
        };
        let Some(target) = self.find_variant_ref(enum_id, &variant_name.text) else {
            self.error(
                variant_name.span,
                format!("enum `{enum_name}` has no variant `{}`", variant_name.text),
            );
            return None;
        };
        let arity = self.enums[enum_id].variants[target.local_index() as usize]
            .fields
            .len();
        if arity != 0 {
            let vname = &variant_name.text;
            self.error(
                access.span,
                format!(
                    "variant `{vname}` of `{enum_name}` takes {arity} argument(s); use `{enum_name}.{vname}(...)` to construct it"
                ),
            );
            return None;
        }
        self.lower_unit_variant(variant_name, target, expected)
    }

    /// `receiver?.field`: the receiver must be an `Option<S>`; the
    /// result is an `Option<F>` where `F` is the field type. Desugared
    /// (see the module docs): `$opt.N = receiver`, then
    /// `if isSome($opt.N) { $res.M = Some(unwrap($opt.N).field) } else { $res.M = None }`
    /// and the expression evaluates to `$res.M`.
    pub(super) fn lower_safe_field_access(
        &mut self,
        access: &ast::FieldAccess,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(&access.receiver, sink, None)?;
        let option_ty = receiver.ty;
        let Some(inner) = self.as_option(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                access.span,
                format!("`?.` requires an Option receiver, found {found}"),
            );
            return None;
        };
        let span = access.span;
        let origin = self.expression_origin(span);
        let tmp = self.alloc_hidden("opt", option_ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: tmp },
                init: receiver,
            },
            span,
        });
        let tmp_expr = hir::Expr {
            kind: ExprKind::Local(tmp),
            ty: option_ty,
            span,
            origin,
        };
        let unwrapped = hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(tmp_expr.clone()),
                trap_on_none: false,
            },
            ty: inner,
            span,
            origin,
        };
        let mut then_body = Vec::new();
        let field_access = match &access.selector {
            ast::FieldSelector::Name(name) => {
                if let Some((property, owner, ty)) =
                    self.find_accessible_nominal_property(inner, &name.text)
                {
                    self.lower_property_read(property, Some(owner), Some(unwrapped), ty, span)?
                } else {
                    match self.resolve_extension_property(
                        unwrapped.clone(),
                        name,
                        &mut then_body,
                        true,
                    ) {
                        crate::properties::ExtensionPropertyResolution::Resolved(property) => {
                            property.read
                        }
                        crate::properties::ExtensionPropertyResolution::Failed => return None,
                        crate::properties::ExtensionPropertyResolution::NoCandidate => {
                            let (field, ty) = self.resolve_field(inner, &access.selector)?;
                            hir::Expr {
                                kind: ExprKind::FieldAccess {
                                    receiver: Box::new(unwrapped),
                                    field,
                                },
                                ty,
                                span,
                                origin,
                            }
                        }
                    }
                }
            }
            ast::FieldSelector::Index(_, _) => {
                let (field, ty) = self.resolve_field(inner, &access.selector)?;
                hir::Expr {
                    kind: ExprKind::FieldAccess {
                        receiver: Box::new(unwrapped),
                        field,
                    },
                    ty,
                    span,
                    origin,
                }
            }
        };
        let result_ty = self.option_type(field_access.ty);
        let result = self.alloc_hidden("res", result_ty);
        then_body.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::SomeWrap(Box::new(field_access)),
                    ty: result_ty,
                    span,
                    origin,
                },
            },
            span,
        });
        let else_value = hir::Expr {
            kind: ExprKind::NoneLiteral,
            ty: result_ty,
            span,
            origin,
        };
        let else_body = vec![hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: else_value,
            },
            span,
        }];
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: hir::Expr {
                    kind: ExprKind::IsSome(Box::new(tmp_expr)),
                    ty: self.boolean,
                    span,
                    origin,
                },
                then_body,
                else_body: Some(else_body),
            },
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span,
            origin,
        })
    }

    /// `lhs ?: rhs`: `lhs` must be an `Option<T>` and `rhs` a `T` (the
    /// right-hand side gets `T` as its expected-type hint). Desugared:
    /// `$opt.N = lhs`, then
    /// `if isSome($opt.N) { $res.M = unwrap($opt.N) } else { $res.M = rhs }`
    /// and the expression evaluates to `$res.M`. The right-hand side is
    /// lowered into the else branch directly, so it (including its own
    /// desugaring statements) is only evaluated on the `None` path.
    pub(super) fn lower_elvis(
        &mut self,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let lhs = self.lower_expr(lhs, sink, None)?;
        let Some(inner) = self.as_option(lhs.ty) else {
            let found = self.type_name(lhs.ty);
            self.error(
                span,
                format!("`?:` requires an Option left-hand side, found {found}"),
            );
            return None;
        };
        let mut else_body = Vec::new();
        let rhs = self.lower_expr(rhs, &mut else_body, Some(inner))?;
        if !self.types_equal(inner, rhs.ty) {
            let expected = self.type_name(inner);
            let found = self.type_name(rhs.ty);
            let message = self.with_nominal_invariance_detail(
                format!("right-hand side of `?:` must be of type {expected}, found {found}"),
                rhs.ty,
                inner,
            );
            self.error(rhs.span, message);
            return None;
        }
        let origin = self.expression_origin(span);
        let then_value = move |_state: &mut Self, tmp: hir::Expr| {
            Some(hir::Expr {
                kind: ExprKind::Unwrap {
                    operand: Box::new(tmp),
                    trap_on_none: false,
                },
                ty: inner,
                span,
                origin,
            })
        };
        self.desugar_option(
            lhs,
            inner,
            span,
            sink,
            then_value,
            ElseBranch {
                statements: else_body,
                value: rhs,
            },
        )
    }

    /// `operand!!`: the operand must be an `Option<T>`; the result is
    /// `T`, trapping on `None` (M3: `scoop_rt_trap`; M8: a real
    /// `UnwrapException`, DESIGN.md 5.2).
    pub(super) fn lower_null_assert(
        &mut self,
        operand: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let Some(inner) = self.as_option(operand.ty) else {
            let found = self.type_name(operand.ty);
            self.error(
                span,
                format!("`!!` requires an Option operand, found {found}"),
            );
            return None;
        };
        Some(hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(operand),
                trap_on_none: true,
            },
            ty: inner,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// The shared `?.` / `?:` desugaring skeleton (see the module
    /// docs): push `$opt.N = receiver` and an `if isSome($opt.N)` whose
    /// branches each initialize the hidden `$res.N` result local —
    /// `then_value($opt.N)` in the then-branch, the else branch's value
    /// (preceded by its own statements) in the else-branch. Returns a
    /// reference to `$res.N`.
    pub(super) fn desugar_option(
        &mut self,
        receiver: hir::Expr,
        result_ty: TypeId,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        then_value: impl FnOnce(&mut Self, hir::Expr) -> Option<hir::Expr>,
        else_branch: ElseBranch,
    ) -> Option<hir::Expr> {
        let option_ty = receiver.ty;
        let origin = self.expression_origin(span);
        let tmp = self.alloc_hidden("opt", option_ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: tmp },
                init: receiver,
            },
            span,
        });
        let tmp_expr = |span| hir::Expr {
            kind: ExprKind::Local(tmp),
            ty: option_ty,
            span,
            origin,
        };
        let cond = hir::Expr {
            kind: ExprKind::IsSome(Box::new(tmp_expr(span))),
            ty: self.boolean,
            span,
            origin,
        };
        let result = self.alloc_hidden("res", result_ty);
        let then_body = vec![hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: then_value(self, tmp_expr(span))?,
            },
            span,
        }];
        let mut else_body = else_branch.statements;
        else_body.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: else_branch.value,
            },
            span,
        });
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond,
                then_body,
                else_body: Some(else_body),
            },
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span,
            origin,
        })
    }

    /// Resolve a field selector against a receiver type: a struct field
    /// by name, a class constructor property by name (base chain
    /// included; the index is the absolute layout index — base fields
    /// prefix, own fields consecutive), or a tuple element by (1-based)
    /// index.
    pub(super) fn resolve_field(
        &mut self,
        receiver_ty: TypeId,
        selector: &ast::FieldSelector,
    ) -> Option<(hir::FieldRef, TypeId)> {
        match self.types[receiver_ty].clone() {
            Type::Class(application) => {
                let class_id = self.class_applications[application].template;
                let class_name = self.classes[class_id].name.clone();
                match selector {
                    ast::FieldSelector::Name(field) => {
                        self.error(
                            field.span,
                            format!("class `{class_name}` has no property `{}`", field.text),
                        );
                        None
                    }
                    ast::FieldSelector::Index(index, span) => {
                        self.error(
                            *span,
                            format!("class `{class_name}` has no field `_{index}`"),
                        );
                        None
                    }
                }
            }
            Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                let struct_id = application_value.template;
                let struct_name = self.structs[struct_id].name.clone();
                match selector {
                    ast::FieldSelector::Name(field) => {
                        let fields = self.structs[struct_id].semantic_fields();
                        let Some(index) = fields.iter().position(|f| f.name == field.text) else {
                            self.error(
                                field.span,
                                format!("struct `{struct_name}` has no field `{}`", field.text),
                            );
                            return None;
                        };
                        let ty = fields[index].ty;
                        let ty = self.instantiate_ty(ty, &application_value.arguments);
                        Some((
                            hir::FieldRef::StructField {
                                application,
                                index: index as u32,
                            },
                            ty,
                        ))
                    }
                    ast::FieldSelector::Index(index, span) => {
                        self.error(
                            *span,
                            format!("struct `{struct_name}` has no field `_{index}`"),
                        );
                        None
                    }
                }
            }
            Type::Tuple(elements) => match selector {
                ast::FieldSelector::Index(index, span) => {
                    // Tuple indices are 1-based (`._1` is the first
                    // element); anything outside `1..=len` is an error.
                    let ty = (1..=elements.len() as u32)
                        .contains(index)
                        .then(|| elements[*index as usize - 1]);
                    match ty {
                        Some(ty) => Some((hir::FieldRef::TupleIndex(index - 1), ty)),
                        None => {
                            let found = self.type_name(receiver_ty);
                            self.error(
                                *span,
                                format!("tuple type `{found}` has no element `_{index}`"),
                            );
                            None
                        }
                    }
                }
                ast::FieldSelector::Name(field) => {
                    let found = self.type_name(receiver_ty);
                    self.error(
                        field.span,
                        format!("tuple type `{found}` has no field `{}`", field.text),
                    );
                    None
                }
            },
            _ => {
                let found = self.type_name(receiver_ty);
                let span = match selector {
                    ast::FieldSelector::Name(field) => field.span,
                    ast::FieldSelector::Index(_, span) => *span,
                };
                self.error(span, format!("type `{found}` has no fields"));
                None
            }
        }
    }
}
