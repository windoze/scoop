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
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if name.text == "field" && self.backing_field_context.is_some() {
            return self
                .contextual_backing_field(name.span)
                .map(|(read, _)| read);
        }
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
            if !self.capture_contexts.is_empty()
                && let Some(capture) = self.available_capture(&name.text)
            {
                let storage = self.lower_capture(name)?;
                if self.local_delegate_plans.contains_key(&capture.binding) {
                    return self.local_delegate_read(storage, capture.binding, name.span);
                }
                return Some(storage);
            }
            if let Some(&(parameter, ty, _)) = self.constructor_params_in_scope.get(&name.text) {
                return Some(hir::Expr {
                    kind: ExprKind::ConstructorParam(parameter),
                    ty,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                });
            }
            if let Some(capture) = self.available_capture(&name.text) {
                let storage = self.lower_capture(name)?;
                if self.local_delegate_plans.contains_key(&capture.binding) {
                    return self.local_delegate_read(storage, capture.binding, name.span);
                }
                return Some(storage);
            }
            if self.initialization_context.is_some()
                && self.initializing_receiver_has_field(&name.text)
            {
                return self
                    .initializing_field(name, name.span)
                    .map(|field| field.read);
            }
            if let Some(expr) = self.bare_member_fallback(name) {
                return Some(expr);
            }
            if self.initialization_context.is_none()
                && let Some(receiver) = self.lower_current_this(name.span)
            {
                match self.resolve_extension_property(receiver, name, sink, true) {
                    crate::properties::ExtensionPropertyResolution::Resolved(property) => {
                        return Some(property.read);
                    }
                    crate::properties::ExtensionPropertyResolution::Failed => return None,
                    crate::properties::ExtensionPropertyResolution::NoCandidate => {}
                }
            }
            if let Some(property) = self.visible_property(&name.text, None) {
                let ty = self.properties[property].ty;
                return self.lower_property_read(property, None, None, ty, name.span);
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
        let binding = self.locals[local].binding;
        if self.local_delegate_plans.contains_key(&binding) {
            let storage = hir::Expr {
                kind: ExprKind::Local(local),
                ty: declared,
                span: name.span,
                origin: self.expression_origin(name.span),
            };
            return self.local_delegate_read(storage, binding, name.span);
        }
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
        if self.initialization_context.is_some() && self.initializing_receiver_has_field(&name.text)
        {
            return self
                .initializing_field(name, name.span)
                .map(|field| field.read);
        }
        let receiver_ty = self.current_this_ty()?;
        if let Some((property, owner, ty)) =
            self.find_accessible_nominal_property(receiver_ty, &name.text)
        {
            let receiver = self.lower_current_this(name.span)?;
            return self.lower_property_read(property, Some(owner), Some(receiver), ty, name.span);
        }
        None
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
            if self
                .local_delegate_plans
                .contains_key(&self.locals[local].binding)
            {
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
