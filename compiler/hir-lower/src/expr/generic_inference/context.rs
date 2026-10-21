//! Context requirements and independent seeds for source expressions.

use super::super::*;

impl Lowerer {
    /// Classify syntax without lowering calls. An unresolved expression can
    /// be synthesized once by its consumer and the successful result reused.
    pub(crate) fn expr_context_requirement(
        &self,
        expr: &ast::Expr,
    ) -> ExpressionContextRequirement {
        use ExpressionContextRequirement::{Independent, Required};
        if crate::expr::float_literal_default_kind(expr) == Some(hir::FloatKind::F64) {
            return Required;
        }
        match expr {
            ast::Expr::IntLiteral(_) => Required,
            ast::Expr::Unary { op, operand, .. }
                if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg)
                    && matches!(&**operand, ast::Expr::IntLiteral(_)) =>
            {
                Required
            }
            ast::Expr::Var(name) => {
                ExpressionContextRequirement::from_required(self.bare_value_requires_expected(name))
            }
            ast::Expr::FieldAccess(access) => ExpressionContextRequirement::from_required(
                self.unit_variant_from_field(access).is_some()
                    || self.imported_unit_variant_requires_expected(access),
            ),
            ast::Expr::CopyUpdate { base, .. } => self.expr_context_requirement(base),
            ast::Expr::TupleLiteral { elements, .. } => {
                if elements.is_empty() {
                    Required
                } else {
                    elements
                        .iter()
                        .map(|element| self.expr_context_requirement(element))
                        .fold(Independent, ExpressionContextRequirement::or)
                }
            }
            // The surrounding type also chooses between Array and
            // MutableArray, even when every element is independently typed.
            ast::Expr::ArrayLiteral { .. } => Required,
            ast::Expr::Call(call) => self
                .constructor_context_requirement(None, &call.callee, &call.args)
                .or(self.bare_call_context_requirement(call)),
            ast::Expr::StructInit { name, args, .. } => {
                self.constructor_context_requirement(None, name, args)
            }
            ast::Expr::MethodCall {
                receiver,
                name,
                type_args,
                args,
                ..
            } => {
                if type_args.is_empty() {
                    self.qualified_variant_context_requirement(receiver, name, args)
                } else {
                    Independent
                }
            }
            // A lambda with an explicit, fully typed parameter header can
            // synthesize its own function type and therefore participate in
            // generic inference before an overload is selected. Untyped or
            // omitted parameters remain contextual and are probed against
            // each candidate transactionally.
            ast::Expr::Lambda {
                parameters: Some(parameters),
                ..
            } if parameters.iter().all(|parameter| parameter.ty.is_some()) => Independent,
            ast::Expr::Lambda { .. }
            | ast::Expr::AnonymousFunction { .. }
            | ast::Expr::CallableReference { .. } => Required,
            // Structured expressions perform their own branch-level fixed
            // point and therefore do not need to be postponed as a whole.
            ast::Expr::If(_) | ast::Expr::When(_) | ast::Expr::Try(_) => Independent,
            _ => Independent,
        }
    }

    fn bare_value_requires_expected(&self, name: &ast::Ident) -> bool {
        if self.scopes.lookup(&name.text).is_some()
            || self.available_capture(&name.text).is_some()
            || self.constructor_params_in_scope.contains_key(&name.text)
            || self.host_has_property(&name.text)
            || self
                .lexical_nested_nominal_target(&name.text)
                .is_some_and(|target| matches!(target, crate::NominalTarget::Object(_)))
        {
            return false;
        }
        match self.lookup_value_origin(&name.text) {
            crate::imports::lookup::LookupResult::Unique(origin) => {
                if let crate::imports::lookup::values::ValueOrigin::Dependency(binding) = &origin {
                    return self.imported_variant_binding_requires_expected(binding);
                }
                match self.materialized_value_target(&origin) {
                    Some(crate::imports::lookup::values::ValueTarget::Variant(target)) => {
                        self.resolved_variant_style(target) == VariantStyle::Unit
                            && !self.enums[target.enumeration()].type_params.is_empty()
                    }
                    _ => false,
                }
            }
            crate::imports::lookup::LookupResult::Missing => true,
            crate::imports::lookup::LookupResult::Ambiguous { .. }
            | crate::imports::lookup::LookupResult::Inaccessible(_) => false,
        }
    }

    fn bare_call_context_requirement(&self, call: &ast::CallExpr) -> ExpressionContextRequirement {
        use ExpressionContextRequirement::{Independent, Unresolved};
        if call.callee.text.contains('.') {
            return Independent;
        }
        if let Some(local) = self.scopes.lookup(&call.callee.text) {
            let mut probe = self.clone();
            let ty = self.locals[local].ty;
            if probe.type_exposes_invoke(ty, false) || matches!(self.types[ty], Type::FunPtr(_)) {
                return Independent;
            }
        }
        if let Some(capture) = self.available_capture(&call.callee.text) {
            let mut probe = self.clone();
            if probe.type_exposes_invoke(capture.ty, false)
                || matches!(self.types[capture.ty], Type::FunPtr(_))
            {
                return Independent;
            }
        }
        Unresolved
    }

    /// Whether a contextual expression can nevertheless synthesize a
    /// complete type when a surrounding inference fixed point has no other
    /// evidence. Numeric literals use their suffix and default format;
    /// aggregate constructors may use such literals transitively.
    ///
    /// This is deliberately separate from `expr_context_requirement`:
    /// overload candidates must still probe `Some(1)` against their own
    /// `Option<Int8>` / `Option<Int16>` parameter types before any default is
    /// committed.
    pub(crate) fn expr_may_provide_default_seed(&self, expr: &ast::Expr) -> bool {
        if crate::expr::float_literal_default_kind(expr).is_some() {
            return true;
        }
        match expr {
            ast::Expr::IntLiteral(_) => true,
            ast::Expr::Unary { op, operand, .. }
                if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg)
                    && matches!(&**operand, ast::Expr::IntLiteral(_)) =>
            {
                true
            }
            ast::Expr::TupleLiteral { elements, .. } => {
                !elements.is_empty()
                    && elements.iter().all(|element| {
                        self.expr_context_requirement(element)
                            != ExpressionContextRequirement::Required
                            || self.expr_may_provide_default_seed(element)
                    })
            }
            ast::Expr::ArrayLiteral { elements, .. } => {
                !elements.is_empty()
                    && elements.iter().all(|element| {
                        self.expr_context_requirement(element)
                            != ExpressionContextRequirement::Required
                            || self.expr_may_provide_default_seed(element)
                    })
            }
            ast::Expr::Call(call) if !call.type_args.is_empty() => false,
            ast::Expr::Call(call) => {
                self.constructor_may_provide_default_seed(None, &call.callee, &call.args)
            }
            ast::Expr::StructInit { name, args, .. } => {
                self.constructor_may_provide_default_seed(None, name, args)
            }
            ast::Expr::CopyUpdate { base, .. } => self.expr_may_provide_default_seed(base),
            ast::Expr::MethodCall {
                receiver,
                name,
                type_args,
                args,
                ..
            } if type_args.is_empty() => {
                let ast::Expr::Var(enum_name) = &**receiver else {
                    return false;
                };
                if self.scopes.lookup(&enum_name.text).is_some()
                    || self.host_has_property(&enum_name.text)
                {
                    return false;
                }
                self.constructor_may_provide_default_seed(Some(&enum_name.text), name, args)
            }
            _ => false,
        }
    }

    fn constructor_may_provide_default_seed(
        &self,
        qualifier: Option<&str>,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> bool {
        let Some((type_param_count, fields)) =
            self.constructor_inference_shape(qualifier, &name.text)
        else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if self.expr_context_requirement(&arg.expression)
                != ExpressionContextRequirement::Required
                || self.expr_may_provide_default_seed(&arg.expression)
            {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.into_iter().all(|bound| bound)
    }

    fn constructor_context_requirement(
        &self,
        qualifier: Option<&str>,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> ExpressionContextRequirement {
        use ExpressionContextRequirement::{Independent, Required, Unresolved};
        let type_name = qualifier.unwrap_or(&name.text);
        // A non-generic alias already fixes the complete target application.
        // Invalid/inaccessible aliases are also lowered immediately so their
        // focused diagnostic is not hidden behind contextual postponement.
        if self.lexical_nested_nominal_target(type_name).is_none()
            && self.source_type_alias_named(type_name).is_some()
        {
            return Independent;
        }
        if qualifier.is_none()
            && let Some((class, _)) = self.top_level_class_named(&name.text)
            && self.array_class_kind(class).is_some()
        {
            let [source] = args else {
                return Independent;
            };
            return self.expr_context_requirement(&source.expression);
        }
        let Some((type_param_count, fields)) =
            self.constructor_inference_shape(qualifier, &name.text)
        else {
            return Independent;
        };
        if type_param_count == 0 {
            return Independent;
        }
        let mut bound = vec![false; type_param_count];
        let mut possible = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            match self.expr_context_requirement(&arg.expression) {
                Independent => self.mark_type_params(field, &mut bound),
                Unresolved => self.mark_type_params(field, &mut possible),
                Required => continue,
            }
        }
        if bound
            .iter()
            .zip(possible)
            .any(|(bound, possible)| !bound && !possible)
        {
            Required
        } else if bound.iter().any(|bound| !bound) {
            Unresolved
        } else {
            Independent
        }
    }

    fn qualified_variant_context_requirement(
        &self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> ExpressionContextRequirement {
        let ast::Expr::Var(enum_name) = receiver else {
            return ExpressionContextRequirement::Independent;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return ExpressionContextRequirement::Independent;
        }
        if self
            .lexical_nested_nominal_target(&enum_name.text)
            .is_none()
            && self.source_type_alias_named(&enum_name.text).is_some()
        {
            return ExpressionContextRequirement::Independent;
        }
        self.constructor_context_requirement(Some(&enum_name.text), name, args)
    }

    /// Generic constructor parameter count and field templates, without
    /// producing diagnostics. `None` means the name is a function/class or
    /// does not denote a constructor.
    pub(in crate::expr) fn constructor_inference_shape(
        &self,
        qualifier: Option<&str>,
        name: &str,
    ) -> Option<(usize, Vec<TypeId>)> {
        let variant = if let Some(enum_name) = qualifier {
            let enum_id = match self.lexical_nested_nominal_target(enum_name) {
                Some(crate::NominalTarget::Enum(id)) => id,
                Some(_) => return None,
                None => self.top_level_enum_named(enum_name)?,
            };
            self.find_variant(enum_id, name)
                .map(|variant| (enum_id, variant))
        } else {
            match self.core_prelude_variant_refs(name) {
                [target] => Some((target.enumeration(), target.local_index())),
                _ => None,
            }
        };
        if let Some((enum_id, variant)) = variant {
            return Some((
                self.enums[enum_id].type_params.len(),
                self.enums[enum_id].variants[variant as usize]
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect(),
            ));
        }
        if qualifier.is_some() {
            return None;
        }
        self.top_level_struct_named(name)
            .map(|(struct_id, _)| {
                (
                    self.structs[struct_id].type_params.len(),
                    self.structs[struct_id]
                        .semantic_fields()
                        .iter()
                        .map(|field| field.ty)
                        .collect(),
                )
            })
            .or_else(|| {
                self.top_level_class_named(name).map(|(class_id, _)| {
                    let constructor = self.classes[class_id].constructors.first().copied();
                    (
                        self.classes[class_id].type_params.len(),
                        constructor
                            .map(|constructor| {
                                self.class_constructors[constructor]
                                    .parameters
                                    .iter()
                                    .map(|parameter| parameter.ty)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    )
                })
            })
    }

    pub(crate) fn mark_type_params(&self, ty: TypeId, bound: &mut [bool]) {
        if let Some((_, arguments)) = self.dependency_nominal_application(ty) {
            for argument in arguments {
                self.mark_type_params(*argument, bound);
            }
            return;
        }
        match &self.types[ty] {
            Type::Param(index) => bound[index.into_raw() as usize] = true,
            Type::Struct(application) => {
                for arg in &self.struct_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Class(application) => {
                for arg in &self.class_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Enum(application) => {
                for arg in &self.enum_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Interface(application) => {
                for arg in &self.interface_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Tuple(args) => {
                for arg in args {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Function(id) => {
                let function = &self.function_types[*id];
                for parameter in &function.parameter_types {
                    self.mark_type_params(*parameter, bound);
                }
                self.mark_type_params(function.return_type, bound);
            }
            Type::Ptr(pointee) => self.mark_type_params(*pointee, bound),
            Type::FunPtr(id) => {
                let function = &self.function_types[*id];
                for parameter in &function.parameter_types {
                    self.mark_type_params(*parameter, bound);
                }
                self.mark_type_params(function.return_type, bound);
            }
            _ => {}
        }
    }

    pub(in crate::expr) fn unit_variant_from_field(
        &self,
        access: &ast::FieldAccess,
    ) -> Option<(hir::EnumId, u32)> {
        let ast::Expr::Var(enum_name) = access.receiver.as_ref() else {
            return None;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return None;
        }
        // An alias target is a complete application, so even a generic enum's
        // unit variant no longer needs an expected type to infer owner args.
        if self
            .lexical_nested_nominal_target(&enum_name.text)
            .is_none()
            && self.source_type_alias_named(&enum_name.text).is_some()
        {
            return None;
        }
        let enum_id = self.top_level_enum_named(&enum_name.text)?;
        let ast::FieldSelector::Name(variant_name) = &access.selector else {
            return None;
        };
        let variant = self.find_variant(enum_id, &variant_name.text)?;
        (!self.enums[enum_id].type_params.is_empty()
            && self.enums[enum_id].variants[variant as usize]
                .fields
                .is_empty())
        .then_some((enum_id, variant))
    }
}
