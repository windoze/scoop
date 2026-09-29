use super::*;
use crate::imports::lookup::values::{ResolvedValueTarget, ValueTarget};

impl Lowerer {
    /// A bare identifier in expression position. Ordinary lexical/member/
    /// top-level values are resolved first, then typed core-prelude unit
    /// variants, then the lowest-priority exact-enum contextual fallback.
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
            if let Some(crate::Owner::Object(object)) = self.current_owner {
                let property = self.classes[self.objects[object].backing_class]
                    .properties
                    .iter()
                    .copied()
                    .find(|property| {
                        self.properties[*property].name == name.text
                            && matches!(
                                self.properties[*property].representation,
                                hir::PropertyRepresentation::Const { .. }
                            )
                    });
                if let Some(property) = property {
                    let ty = self.properties[property].ty;
                    return self.lower_property_read(property, None, None, ty, name.span);
                }
            }
            if let Some(expr) = self.bare_member_fallback(name) {
                return Some(expr);
            }
            if let Some(object) =
                self.lexical_nested_nominal_target(&name.text)
                    .and_then(|target| match target {
                        crate::NominalTarget::Object(object) => Some(object),
                        _ => None,
                    })
            {
                return self.lower_singleton_value(object, name.span);
            }
            let mut implicit_failure = None;
            let selected_value = if self.initialization_context.is_none()
                && let Some(receiver) = self.lower_current_this(name.span)
            {
                match self.resolve_implicit_value_read(receiver, name, sink) {
                    crate::properties::ImplicitValueResolution::ExtensionProperty(property) => {
                        return Some(property.read);
                    }
                    crate::properties::ImplicitValueResolution::Value { target, layer } => {
                        Some((target, layer))
                    }
                    crate::properties::ImplicitValueResolution::NoApplicable(failure) => {
                        implicit_failure = Some(failure);
                        None
                    }
                    crate::properties::ImplicitValueResolution::Failed => return None,
                    crate::properties::ImplicitValueResolution::NoCandidate => {
                        self.resolve_value_name_with_layer(name).ok()?
                    }
                }
            } else {
                self.resolve_value_name_with_layer(name).ok()?
            };
            let mut prelude_failure = None;
            if let Some((target, selected_layer)) = selected_value {
                let core_variant = selected_layer == crate::imports::ImportLookupLayer::CorePrelude
                    && match &target {
                        ResolvedValueTarget::Materialized(ValueTarget::Variant(_)) => true,
                        ResolvedValueTarget::Dependency(binding) => {
                            matches!(binding.target(), hir::ImportedTarget::EnumVariant(_))
                        }
                        _ => false,
                    };
                if core_variant {
                    match self.probe_expr_layer(|state, _| {
                        state.lower_resolved_value_target(name, target, expected)
                    }) {
                        Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                        Err(failure) => prelude_failure = Some(failure),
                    }
                } else {
                    return self.lower_resolved_value_target(name, target, expected);
                }
            }
            if let Some((owner, index)) = self.contextual_imported_variant(&name.text, expected) {
                return self.lower_imported_unit_variant(owner, index, name);
            }
            if let Some(target) = self.contextual_variant_ref(&name.text, expected) {
                let enumeration = target.enumeration();
                if self.resolved_variant_style(target) != VariantStyle::Unit {
                    self.error(
                        name.span,
                        format!(
                            "variant `{}` of `{}` takes arguments; use `{}(...)` to construct it",
                            name.text, self.enums[enumeration].name, name.text
                        ),
                    );
                    return None;
                }
                return self.lower_unit_variant(name, target, expected);
            }
            if let Some(enumeration) = self.exact_expected_enum(expected) {
                self.error(
                    name.span,
                    format!(
                        "enum `{}` has no variant `{}`",
                        self.enums[enumeration].name, name.text
                    ),
                );
                return None;
            }
            if let Some(failure) = implicit_failure {
                self.commit_layer_diagnostics(*failure);
                return None;
            }
            if let Some(failure) = prelude_failure {
                self.commit_layer_diagnostics(*failure);
                return None;
            }
            if !self.local_function_scopes.lookup(&name.text).is_empty()
                || self.has_top_level_function_candidate(&name.text)
                || self.has_top_level_extension_candidate(&name.text)
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
            if let Some(crate::Owner::Object(object)) = self.current_owner
                && self.companion_host_declares_property(object, &name.text)
            {
                self.error(
                    name.span,
                    format!(
                        "companion object cannot access host instance property `{}` without a host instance",
                        name.text
                    ),
                );
                return None;
            }
            if self.lexical_nested_nominal_target(&name.text).is_none()
                && self.source_type_alias_named(&name.text).is_some()
            {
                self.resolve_type_alias_reference(name, false)?;
                self.error(
                    name.span,
                    format!("typealias `{}` is a type, not a value", name.text),
                );
                return None;
            }
            self.error(
                name.span,
                format!(
                    "unknown variable `{}`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation",
                    name.text
                ),
            );
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

    pub(crate) fn lower_named_value_target(
        &mut self,
        name: &ast::Ident,
        target: ValueTarget,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match target {
            ValueTarget::Property(property) => {
                let (owner, receiver) = self.named_property_receiver(property, name.span)?.parts();
                self.lower_property_read(
                    property,
                    owner,
                    receiver,
                    self.properties[property].ty,
                    name.span,
                )
            }
            ValueTarget::Object(object) => self.lower_singleton_value(object, name.span),
            ValueTarget::Variant(target) => {
                if self.resolved_variant_style(target) != VariantStyle::Unit {
                    self.error(
                        name.span,
                        format!(
                            "variant `{}` of `{}` takes arguments; use `{}(...)` to construct it",
                            name.text,
                            self.enums[target.enumeration()].name,
                            name.text
                        ),
                    );
                    return None;
                }
                self.lower_unit_variant(name, target, expected)
            }
        }
    }

    fn lower_resolved_value_target(
        &mut self,
        name: &ast::Ident,
        target: ResolvedValueTarget,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match target {
            ResolvedValueTarget::Materialized(target) => {
                self.lower_named_value_target(name, target, expected)
            }
            ResolvedValueTarget::Dependency(binding) => match binding.target() {
                hir::ImportedTarget::ObjectValue(value) => {
                    self.lower_imported_singleton(value.persistent(), name.span)
                }
                hir::ImportedTarget::EnumVariant(_) => {
                    self.lower_imported_variant_binding(&binding, name, expected)
                }
                _ => self
                    .lower_imported_dependency_property_read(&binding, None, name.span)
                    .map(|property| property.expression),
            },
        }
    }

    pub(in crate::expr) fn ambiguous_prelude_variant(
        &mut self,
        name: &ast::Ident,
        candidates: &[hir::EnumVariantRef],
    ) {
        let mut targets = candidates
            .iter()
            .map(|target| {
                format!(
                    "{}.{}",
                    self.enums[target.enumeration()].name,
                    self.enums[target.enumeration()].variants[target.local_index() as usize].name
                )
            })
            .collect::<Vec<_>>();
        targets.sort();
        self.error(
            name.span,
            format!(
                "ambiguous core-prelude variant `{}`: {}",
                name.text,
                targets.join(", ")
            ),
        );
    }

    pub(crate) fn lower_singleton_value(
        &mut self,
        object: hir::ObjectId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let declaration = self.objects[object].clone();
        if !self.access_domain_allows(&declaration.access.lookup.0) {
            let kind = match declaration.kind {
                hir::ObjectKind::Standalone => "object",
                hir::ObjectKind::Companion(_) => "companion object",
            };
            self.error(
                span,
                format!("{kind} `{}` is not accessible here", declaration.name),
            );
            return None;
        }
        let value = self.singleton_values[declaration.singleton_value];
        if let Some(current) = self.current_initialization_unit {
            let dependencies = &mut self.initialization_units[current].dependencies;
            if !dependencies
                .iter()
                .any(|dependency| dependency.unit == value.initialization)
            {
                dependencies.push(hir::InitializationDependency {
                    unit: value.initialization,
                    span,
                });
            }
        }
        Some(hir::Expr {
            kind: hir::ExprKind::SingletonValue(declaration.singleton_value),
            ty: self.object_types[declaration.object_type].canonical_type,
            span,
            origin: self.expression_origin(span),
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
        let property = self
            .resolve_imported_member_property(receiver_ty, name)
            .ok()??;
        let receiver = self.lower_current_this(name.span)?;
        self.emit_imported_member_property_read(&property, receiver, name.span)
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
