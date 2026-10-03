//! Context requirements and independent seeds for source expressions.

use super::super::*;

impl Lowerer {
    /// Expressions whose type cannot be synthesized without context. Calls
    /// to generic constructors are contextual only when their own
    /// context-independent arguments cannot bind every constructor variable;
    /// this lets nested calls perform their own fixed-point inference.
    pub(crate) fn expr_requires_expected_type(&self, expr: &ast::Expr) -> bool {
        match expr {
            ast::Expr::IntLiteral(_) => true,
            ast::Expr::Unary { op, operand, .. }
                if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg)
                    && matches!(&**operand, ast::Expr::IntLiteral(_)) =>
            {
                true
            }
            ast::Expr::Var(name) => self.bare_value_requires_expected(name),
            ast::Expr::FieldAccess(access) => {
                self.unit_variant_from_field(access).is_some()
                    || self.imported_unit_variant_requires_expected(access)
            }
            ast::Expr::CopyUpdate { base, .. } => self.expr_requires_expected_type(base),
            ast::Expr::TupleLiteral { elements, .. } => {
                elements.is_empty()
                    || elements
                        .iter()
                        .any(|element| self.expr_requires_expected_type(element))
            }
            // The surrounding type also chooses between Array and
            // MutableArray, even when every element is independently typed.
            ast::Expr::ArrayLiteral { .. } => true,
            ast::Expr::Call(call) => {
                self.constructor_requires_expected(&call.callee, &call.args)
                    || self.bare_call_requires_contextual_lookup(call)
            }
            ast::Expr::StructInit { name, args, .. } => {
                self.constructor_requires_expected(name, args)
            }
            ast::Expr::MethodCall {
                receiver,
                name,
                type_args,
                args,
                ..
            } => {
                type_args.is_empty()
                    && self.qualified_variant_requires_expected(receiver, name, args)
            }
            // A lambda with an explicit, fully typed parameter header can
            // synthesize its own function type and therefore participate in
            // generic inference before an overload is selected. Untyped or
            // omitted parameters remain contextual and are probed against
            // each candidate transactionally.
            ast::Expr::Lambda {
                parameters: Some(parameters),
                ..
            } if parameters.iter().all(|parameter| parameter.ty.is_some()) => false,
            ast::Expr::Lambda { .. }
            | ast::Expr::AnonymousFunction { .. }
            | ast::Expr::CallableReference { .. } => true,
            // Structured expressions perform their own branch-level fixed
            // point and therefore do not need to be postponed as a whole.
            ast::Expr::If(_) | ast::Expr::When(_) | ast::Expr::Try(_) => false,
            _ => false,
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

    fn bare_call_requires_contextual_lookup(&self, call: &ast::CallExpr) -> bool {
        if call.callee.text.contains('.') {
            return false;
        }
        if let Some(local) = self.scopes.lookup(&call.callee.text) {
            let mut probe = self.clone();
            let ty = self.locals[local].ty;
            if probe.type_exposes_invoke(ty, false) || matches!(self.types[ty], Type::FunPtr(_)) {
                return false;
            }
        }
        if let Some(capture) = self.available_capture(&call.callee.text) {
            let mut probe = self.clone();
            if probe.type_exposes_invoke(capture.ty, false)
                || matches!(self.types[capture.ty], Type::FunPtr(_))
            {
                return false;
            }
        }
        let mut probe = self.clone();
        let mut sink = Vec::new();
        probe.lower_call(call, &mut sink, None).is_none()
    }

    /// Whether a contextual expression can nevertheless synthesize a
    /// complete type when a surrounding inference fixed point has no other
    /// evidence. Integer literals use their suffix/default-width ladder;
    /// aggregate constructors may use such literals transitively.
    ///
    /// This is deliberately separate from `expr_requires_expected_type`:
    /// overload candidates must still probe `Some(1)` against their own
    /// `Option<Int8>` / `Option<Int16>` parameter types before any default is
    /// committed.
    pub(crate) fn expr_can_provide_default_seed(&self, expr: &ast::Expr) -> bool {
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
                        !self.expr_requires_expected_type(element)
                            || self.expr_can_provide_default_seed(element)
                    })
            }
            ast::Expr::ArrayLiteral { elements, .. } => {
                !elements.is_empty()
                    && elements.iter().all(|element| {
                        !self.expr_requires_expected_type(element)
                            || self.expr_can_provide_default_seed(element)
                    })
            }
            ast::Expr::Call(call) if !call.type_args.is_empty() => false,
            ast::Expr::Call(call) => {
                self.constructor_can_provide_default_seed(&call.callee, &call.args)
            }
            ast::Expr::StructInit { name, args, .. } => {
                self.constructor_can_provide_default_seed(name, args)
            }
            ast::Expr::CopyUpdate { base, .. } => self.expr_can_provide_default_seed(base),
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
                let qualified = ast::Ident {
                    text: format!("{}.{}", enum_name.text, name.text),
                    span: Span::new(enum_name.span.start, name.span.end),
                };
                self.constructor_can_provide_default_seed(&qualified, args)
            }
            _ => false,
        }
    }

    fn constructor_can_provide_default_seed(
        &self,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> bool {
        let Some((type_param_count, fields)) = self.constructor_inference_shape(&name.text) else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if !self.expr_requires_expected_type(&arg.expression)
                || self.expr_can_provide_default_seed(&arg.expression)
            {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.into_iter().all(|bound| bound)
    }

    pub(in crate::expr) fn constructor_requires_expected(
        &self,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> bool {
        let qualifier = name
            .text
            .split_once('.')
            .map_or(name.text.as_str(), |(qualifier, _)| qualifier);
        // A non-generic alias already fixes the complete target application.
        // Invalid/inaccessible aliases are also lowered immediately so their
        // focused diagnostic is not hidden behind contextual postponement.
        if self.lexical_nested_nominal_target(qualifier).is_none()
            && self.source_type_alias_named(qualifier).is_some()
        {
            return false;
        }
        if let Some((class, _)) = self.top_level_class_named(&name.text)
            && self.array_class_kind(class).is_some()
        {
            let [source] = args else {
                return false;
            };
            return self.expr_requires_expected_type(&source.expression);
        }
        let Some((type_param_count, fields)) = self.constructor_inference_shape(&name.text) else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if !self.expr_requires_expected_type(&arg.expression) {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.iter().any(|bound| !bound)
    }

    pub(in crate::expr) fn qualified_variant_requires_expected(
        &self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        args: &[ast::CallArgument],
    ) -> bool {
        let ast::Expr::Var(enum_name) = receiver else {
            return false;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return false;
        }
        if self
            .lexical_nested_nominal_target(&enum_name.text)
            .is_none()
            && self.source_type_alias_named(&enum_name.text).is_some()
        {
            return false;
        }
        let qualified = format!("{}.{}", enum_name.text, name.text);
        self.constructor_requires_expected(
            &ast::Ident {
                text: qualified,
                span: Span::new(enum_name.span.start, name.span.end),
            },
            args,
        )
    }

    /// Generic constructor parameter count and field templates, without
    /// producing diagnostics. `None` means the name is a function/class or
    /// does not denote a constructor.
    pub(in crate::expr) fn constructor_inference_shape(
        &self,
        name: &str,
    ) -> Option<(usize, Vec<TypeId>)> {
        let variant = if let Some((enum_name, variant_name)) = name.split_once('.') {
            let enum_id = self.top_level_enum_named(enum_name)?;
            self.find_variant(enum_id, variant_name)
                .map(|variant| (enum_id, variant))
        } else {
            let [target] = self.core_prelude_variant_refs(name) else {
                return None;
            };
            Some((target.enumeration(), target.local_index()))
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
