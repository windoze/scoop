use super::*;

impl Lowerer {
    /// A bare identifier in expression position. The `Option` variants
    /// (`Some` / `None`) are visible without a prefix (spec 7.2 default
    /// import) and take precedence over locals (M3 behavior); every
    /// other enum's variants need the `E.V` prefix (M4 simplification,
    /// milestone4 DESIGN.md 3.2).
    ///
    /// M6: an active smart-cast narrowing retypes the reference (an
    /// immutable local narrowed by an enclosing `if (x is T)`); a
    /// narrowed value type unboxes on access. Inside a member function
    /// a name that is no local falls back to a property of the host
    /// (`x` meaning `this.x`, milestone6 DESIGN.md 1).
    pub(crate) fn lower_var(
        &mut self,
        name: &ast::Ident,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some((enum_id, variant)) = self.option_variant(&name.text) {
            if self.enums[enum_id].variants[variant as usize]
                .fields
                .is_empty()
            {
                return self.lower_unit_variant(name, enum_id, variant, expected);
            }
            let text = &name.text;
            self.error(
                name.span,
                format!("variant `{text}` of `Option` takes arguments; use `{text}(...)` to construct it"),
            );
            return None;
        }
        let Some(local) = self.scopes.lookup(&name.text) else {
            if let Some(&(parameter, ty)) = self.constructor_params_in_scope.get(&name.text) {
                return Some(hir::Expr {
                    kind: ExprKind::ConstructorParam(parameter),
                    ty,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                });
            }
            if self.available_capture(&name.text).is_some() {
                return self.lower_capture(name);
            }
            if let Some(expr) = self.bare_member_fallback(name) {
                return Some(expr);
            }
            if let Some(&global) = self.globals_by_name.get(&name.text) {
                if matches!(
                    self.globals[global].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(name.span, "reading an extern global");
                }
                return Some(hir::Expr {
                    kind: ExprKind::GlobalRead(global),
                    ty: self.globals[global].ty,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                });
            }
            if !self.local_function_scopes.lookup(&name.text).is_empty()
                || self.functions_by_name.contains_key(&name.text)
                || self.extensions_by_name.contains_key(&name.text)
            {
                self.error(
                    name.span,
                    format!(
                        "function `{}` is not a value; use `::{}` to create a callable reference",
                        name.text, name.text
                    ),
                );
                return None;
            }
            self.error(name.span, format!("unknown variable `{}`", name.text));
            return None;
        };
        let declared = self.locals[local].ty;
        if let Some(&narrowed) = self.smart_casts.get(&local) {
            if !self.types_equal(narrowed, declared) {
                let local_expr = hir::Expr {
                    kind: ExprKind::Local(local),
                    ty: declared,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                };
                if self.is_value_ty(narrowed) {
                    // A boxed value narrowed to its value type unboxes.
                    return Some(hir::Expr {
                        kind: ExprKind::Unbox(Box::new(local_expr)),
                        ty: narrowed,
                        span: name.span,
                        origin: self.expression_origin(name.span),
                    });
                }
                // A reference narrowed to a subtype: zero-cost retype.
                return Some(hir::Expr {
                    kind: ExprKind::Local(local),
                    ty: narrowed,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                });
            }
        }
        Some(hir::Expr {
            kind: ExprKind::Local(local),
            ty: declared,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    /// `x` inside a member function when `x` is no local: a property
    /// of the host type (`this.x`; class properties include the base
    /// chain). Methods are not values in M6, so only fields resolve.
    pub(super) fn bare_member_fallback(&mut self, name: &ast::Ident) -> Option<hir::Expr> {
        let receiver_ty = self.current_this_ty()?;
        let receiver = self.lower_current_this(name.span)?;
        let (field, ty) = match self.types[receiver_ty].clone() {
            Type::Class(application) => {
                let (declaring, index, ty, _) =
                    self.find_class_application_field(application, &name.text)?;
                (
                    hir::FieldRef::ClassField {
                        application: declaring,
                        index,
                    },
                    ty,
                )
            }
            Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                let struct_id = application_value.template;
                let index = self.structs[struct_id]
                    .semantic_fields()
                    .iter()
                    .position(|field| field.name == name.text)?;
                (
                    hir::FieldRef::StructField {
                        application,
                        index: index as u32,
                    },
                    self.instantiate_ty(
                        self.structs[struct_id].semantic_fields()[index].ty,
                        &application_value.arguments,
                    ),
                )
            }
            // Interfaces have no properties; enum payloads are only
            // reachable through patterns.
            _ => return None,
        };
        Some(hir::Expr {
            kind: ExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field,
            },
            ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    /// Smart-cast candidates established by `cond` evaluating to
    /// `outcome` (milestone6 DESIGN.md 5.4): `x is T` in the true
    /// branch, `x !is T` / `!(x is T)` in the false branch, and the
    /// conjuncts of `&&` in the true branch. Anything else (including
    /// `||`) establishes nothing in M6.
    pub(crate) fn resolve_smart_casts(
        &mut self,
        cond: &ast::Expr,
        outcome: bool,
    ) -> Vec<(hir::LocalId, TypeId)> {
        let mut candidates = Vec::new();
        collect_smart_cast_candidates(cond, outcome, &mut candidates);
        let mut result: Vec<(hir::LocalId, TypeId)> = Vec::new();
        for (name, ty_ref) in candidates {
            let Some(local) = self.scopes.lookup(&name.text) else {
                continue;
            };
            // Only immutable locals can be narrowed (the condition is
            // pure and the variable cannot change below it).
            if self.locals[local].mutable {
                continue;
            }
            let Some(narrowed) = self.resolve_type_ref(ty_ref) else {
                continue; // the condition's own lowering diagnoses this
            };
            let declared = self.locals[local].ty;
            // Narrowing must go strictly downward.
            if self.types_equal(narrowed, declared) || !self.is_subtype(narrowed, declared) {
                continue;
            }
            if result.iter().any(|&(l, _)| l == local) {
                continue; // first conjunct wins
            }
            result.push((local, narrowed));
        }
        result
    }

    /// Run `f` with additional smart-cast narrowings active, restoring
    /// the previous set afterwards (narrowings never escape their
    /// branch).
    pub(crate) fn with_smart_casts<T>(
        &mut self,
        narrowings: Vec<(hir::LocalId, TypeId)>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if narrowings.is_empty() {
            return f(self);
        }
        let saved = self.smart_casts.clone();
        for (local, ty) in narrowings {
            self.smart_casts.insert(local, ty);
        }
        let result = f(self);
        self.smart_casts = saved;
        result
    }
}
