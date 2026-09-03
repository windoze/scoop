use super::*;

impl Lowerer {
    /// Resolve the field types of a struct declaration. Fields with
    /// duplicate names or unresolvable types are diagnosed and dropped;
    /// the module is rejected anyway once any diagnostic is recorded.
    pub(super) fn resolve_fields(&mut self, id: StructId, decl: &ast::StructDecl) {
        self.type_params_in_scope = self.structs[id].type_params.clone();
        if matches!(
            self.structs[id].representation,
            hir::StructRepresentation::Intrinsic(_)
        ) {
            self.type_params_in_scope.clear();
            return;
        }
        let mut seen = HashSet::new();
        let mut fields = Vec::new();
        if decl.fields.is_omitted() {
            self.error(
                decl.name.span,
                "an omitted struct representation requires a validated intrinsic type declaration"
                    .to_string(),
            );
        }
        for field in &decl.fields {
            if !seen.insert(field.name.text.clone()) {
                self.error(
                    field.name.span,
                    format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&field.ty) else {
                continue; // diagnostic already recorded
            };
            fields.push(hir::Field {
                name: field.name.text.clone(),
                ty,
            });
        }
        self.structs[id].representation = hir::StructRepresentation::Declared(fields);
        self.type_params_in_scope.clear();
    }

    /// Resolve the variants of an enum declaration (pass 2): duplicate
    /// variant and field names are diagnosed, field types resolve in
    /// the enum's type-parameter scope, and constructor-style defaults
    /// must be literals matching the field type (milestone4 DESIGN.md
    /// 5.4).
    pub(super) fn resolve_variants(&mut self, id: EnumId, decl: &ast::EnumDecl) {
        self.type_params_in_scope = self.enums[id].type_params.clone();
        let mut seen = HashSet::new();
        let mut variants = Vec::new();
        for (index, variant) in decl.variants.iter().enumerate() {
            if !seen.insert(variant.name.text.clone()) {
                self.error(
                    variant.name.span,
                    format!(
                        "duplicate variant `{}` in enum `{}`",
                        variant.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let style = match &variant.kind {
                ast::VariantDeclKind::Unit => VariantStyle::Unit,
                ast::VariantDeclKind::Positional(_) => VariantStyle::Positional,
                ast::VariantDeclKind::Named(_) => VariantStyle::Named,
                ast::VariantDeclKind::Constructor(_) => VariantStyle::Constructor,
            };
            let Some(resolved) = self.resolve_variant_fields(variant) else {
                continue; // diagnostic already recorded
            };
            self.variant_styles.insert((id, index as u32), style);
            variants.push(hir::Variant {
                name: variant.name.text.clone(),
                fields: resolved.fields,
                defaults: resolved.defaults,
            });
        }
        self.enums[id].variants = variants;
        self.type_params_in_scope.clear();
    }

    /// The fields (and constructor-style defaults) of one variant.
    /// Returns `None` after recording a diagnostic.
    fn resolve_variant_fields(&mut self, variant: &ast::VariantDecl) -> Option<ResolvedFields> {
        match &variant.kind {
            ast::VariantDeclKind::Unit => Some(ResolvedFields::default()),
            // Positional fields get `_1`-style names (hir docs).
            ast::VariantDeclKind::Positional(types) => {
                let mut resolved = ResolvedFields::default();
                for (index, ty_ref) in types.iter().enumerate() {
                    let ty = self.resolve_type_ref(ty_ref)?;
                    resolved.fields.push(hir::Field {
                        name: format!("_{}", index + 1),
                        ty,
                    });
                    resolved.defaults.push(None);
                }
                Some(resolved)
            }
            ast::VariantDeclKind::Named(fields) | ast::VariantDeclKind::Constructor(fields) => {
                let constructor = matches!(variant.kind, ast::VariantDeclKind::Constructor(_));
                let mut seen = HashSet::new();
                let mut resolved = ResolvedFields::default();
                for field in fields {
                    if !seen.insert(field.name.text.clone()) {
                        self.error(
                            field.name.span,
                            format!(
                                "duplicate field `{}` in variant `{}`",
                                field.name.text, variant.name.text
                            ),
                        );
                        return None;
                    }
                    let ty = self.resolve_type_ref(&field.ty)?;
                    let default = match &field.syntax {
                        ast::ParameterSyntax::Default {
                            expression: default,
                            ..
                        } if constructor => {
                            Some(self.resolve_variant_default(variant, field, ty, default)?)
                        }
                        // The parser only produces defaults on
                        // constructor-style variants; reject the shape
                        // here so every AST form is handled.
                        ast::ParameterSyntax::Default { .. }
                        | ast::ParameterSyntax::Vararg { .. }
                            if !constructor =>
                        {
                            self.error(
                                field.span,
                                format!(
                                    "default value of field `{}` in variant `{}` is only allowed on constructor-style variants",
                                    field.name.text, variant.name.text
                                ),
                            );
                            return None;
                        }
                        ast::ParameterSyntax::Default { .. } => {
                            unreachable!("constructor default handled by the first arm")
                        }
                        ast::ParameterSyntax::Required | ast::ParameterSyntax::Vararg { .. } => {
                            None
                        }
                    };
                    resolved.fields.push(hir::Field {
                        name: field.name.text.clone(),
                        ty,
                    });
                    resolved.defaults.push(default);
                }
                Some(resolved)
            }
        }
    }

    /// A constructor-style variant field default (M4: literals only,
    /// milestone4 DESIGN.md 5.4), checked against the field type.
    pub(super) fn resolve_variant_default(
        &mut self,
        variant: &ast::VariantDecl,
        field: &ast::VariantFieldDecl,
        field_ty: TypeId,
        default: &ast::Expr,
    ) -> Option<hir::Expr> {
        let span = default.span();
        let (kind, ty) = match default {
            ast::Expr::IntLiteral { value, .. } => (hir::ExprKind::IntLiteral(*value), self.int),
            ast::Expr::StringLiteral { value, .. } => {
                (hir::ExprKind::StringLiteral(value.clone()), self.string)
            }
            ast::Expr::BoolLiteral { value, .. } => {
                (hir::ExprKind::BoolLiteral(*value), self.boolean)
            }
            // A negative integer literal (`-1`) parses as unary minus.
            ast::Expr::Unary {
                op: ast::UnOp::Neg,
                operand,
                ..
            } => match &**operand {
                ast::Expr::IntLiteral { value, span } => {
                    let operand = Box::new(hir::Expr {
                        kind: hir::ExprKind::IntLiteral(*value),
                        ty: self.int,
                        span: *span,
                    });
                    (
                        hir::ExprKind::Unary {
                            op: hir::UnOp::Neg,
                            operand,
                        },
                        self.int,
                    )
                }
                _ => return self.invalid_variant_default(variant, field, span),
            },
            _ => return self.invalid_variant_default(variant, field, span),
        };
        if !self.types_equal(field_ty, ty) {
            let expected = self.type_name(field_ty);
            let found = self.type_name(ty);
            self.error(
                span,
                format!(
                    "default value of field `{}` in variant `{}` must be of type {expected}, found {found}",
                    field.name.text, variant.name.text
                ),
            );
            return None;
        }
        Some(hir::Expr { kind, ty, span })
    }

    pub(super) fn invalid_variant_default(
        &mut self,
        variant: &ast::VariantDecl,
        field: &ast::VariantFieldDecl,
        span: Span,
    ) -> Option<hir::Expr> {
        self.error(
            span,
            format!(
                "default value of field `{}` in variant `{}` must be a literal",
                field.name.text, variant.name.text
            ),
        );
        None
    }

    /// Resolve a function signature: typed parameters, value parameters
    /// and the return type (absent means `Unit`). Parameter
    /// locals are only allocated when the body is lowered (pass 3), so
    /// intrinsic functions — which have no body — keep an empty
    /// `params` list on the `hir::Function`; calls check against this
    /// resolved signature like any other function's (M7).
    pub(super) fn resolve_signature(&mut self, id: FunctionId, decl: &ast::FunctionDecl) {
        // Member-only flags on a top-level function (M6).
        if decl.modifier == ast::MethodModifier::Abstract {
            self.error(
                decl.name.span,
                format!(
                    "abstract function `{}` is only allowed in abstract classes",
                    decl.name.text
                ),
            );
        }
        if decl.is_override {
            self.error(
                decl.name.span,
                format!(
                    "`{}` is marked `override` but does not override any method",
                    decl.name.text
                ),
            );
        }
        if matches!(decl.body, ast::FunctionBody::None)
            && matches!(self.functions[id].kind, FunctionKind::User(_))
            && !decl
                .annotations
                .iter()
                .any(|annotation| matches!(annotation.name.text.as_str(), "Intrinsic" | "Extern"))
        {
            self.error(
                decl.name.span,
                format!("function `{}` must have a body", decl.name.text),
            );
        }
        let mut type_params = Vec::new();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            let parameter = self.fresh_type_param(type_params.len());
            type_params.push(lower_type_param_decl(param, parameter));
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            0,
            &decl.type_params,
            decl.where_clause.as_ref(),
            "function",
        );
        if !type_params.is_empty() {
            self.register_generic(id, type_params.clone());
        }
        self.type_params_in_scope = type_params.clone();

        if let Some(receiver) = &decl.receiver_ty
            && let Some(receiver_ty) = self.resolve_type_ref(receiver)
        {
            self.extension_receivers.insert(id, receiver_ty);
        }

        let mut params = Vec::with_capacity(decl.params.len());
        for param in &decl.params {
            // On failure the diagnostic is already recorded and the
            // module is rejected; the parameter is simply dropped.
            if let Some(ty) = self.resolve_type_ref(&param.ty) {
                params.push(FnParam {
                    name: param.name.clone(),
                    ty,
                });
            }
        }
        let return_ty = match &decl.return_ty {
            Some(ty_ref) => self.resolve_type_ref(ty_ref).unwrap_or(self.unit),
            None => self.unit,
        };
        self.type_params_in_scope.clear();

        self.functions[id].return_ty = return_ty;
        self.signatures.insert(
            id,
            FnSig {
                is_suspend: decl.is_suspend,
                operator: None,
                attributes: self.functions[id].attributes,
                owner_type_param_count: 0,
                type_params,
                params,
                return_ty,
            },
        );
        if let FunctionKind::Extern(extern_id) = self.functions[id].kind {
            let signature = &self.signatures[&id];
            self.extern_functions[extern_id].params =
                signature.params.iter().map(|param| param.ty).collect();
            self.extern_functions[extern_id].return_type = return_ty;
        }
    }
}

/// Intermediate result of variant field resolution.
#[derive(Default)]
struct ResolvedFields {
    fields: Vec<hir::Field>,
    defaults: Vec<Option<hir::Expr>>,
}
