//! Generic argument inference and structural type-parameter binding.

use super::*;

impl Lowerer {
    /// Lower generic-call/constructor arguments to a fixed point. An
    /// expression that intrinsically needs an expected type is postponed
    /// while its parameter still contains an unbound type variable; other
    /// arguments can then add bindings independently of their source order.
    pub(super) fn lower_inference_args(
        &mut self,
        arg_exprs: &[ast::Expr],
        param_tys: &[TypeId],
        mut bindings: Vec<Option<TypeId>>,
        type_params: &[hir::TypeParamDecl],
    ) -> Option<InferredArguments> {
        let mut args: Vec<Option<hir::Expr>> = (0..arg_exprs.len()).map(|_| None).collect();
        let mut sinks: Vec<Vec<hir::Statement>> =
            (0..arg_exprs.len()).map(|_| Vec::new()).collect();
        loop {
            let mut progress = false;
            for index in 0..arg_exprs.len() {
                if args[index].is_some() {
                    continue;
                }
                let hint = self.try_substitute(param_tys[index], &bindings);
                if hint.is_none() && self.expr_requires_expected_type(&arg_exprs[index]) {
                    continue;
                }
                let arg = self.lower_expr(&arg_exprs[index], &mut sinks[index], hint)?;
                if !self.bind_type_args(
                    param_tys[index],
                    arg.ty,
                    &mut bindings,
                    type_params,
                    arg.span,
                ) {
                    return None;
                }
                args[index] = Some(arg);
                progress = true;
            }
            if args.iter().all(Option::is_some) {
                break;
            }
            if !progress {
                // No later constraint could type the first deferred
                // expression. Lower it without a hint to retain the
                // focused diagnostic (`cannot infer the type of None`,
                // empty-array element type, and so on).
                let index = args
                    .iter()
                    .position(Option::is_none)
                    .expect("an unresolved argument remains");
                let arg = self.lower_expr(&arg_exprs[index], &mut sinks[index], None)?;
                if !self.bind_type_args(
                    param_tys[index],
                    arg.ty,
                    &mut bindings,
                    type_params,
                    arg.span,
                ) {
                    return None;
                }
                args[index] = Some(arg);
            }
        }
        Some(InferredArguments {
            args,
            bindings,
            sinks,
        })
    }

    /// Expressions whose type cannot be synthesized without context. Calls
    /// to generic constructors are contextual only when their own
    /// context-independent arguments cannot bind every constructor variable;
    /// this lets nested calls perform their own fixed-point inference.
    pub(crate) fn expr_requires_expected_type(&self, expr: &ast::Expr) -> bool {
        match expr {
            ast::Expr::Var(name) => {
                name.text == "None"
                    && self.scopes.lookup(&name.text).is_none()
                    && !self.host_has_property(&name.text)
            }
            ast::Expr::FieldAccess(access) => self.unit_variant_from_field(access).is_some(),
            ast::Expr::TupleLiteral { elements, .. } | ast::Expr::ArrayLiteral { elements, .. } => {
                elements.is_empty()
                    || elements
                        .iter()
                        .any(|element| self.expr_requires_expected_type(element))
            }
            ast::Expr::Call(call) if !call.type_args.is_empty() => false,
            ast::Expr::Call(call) => self.constructor_requires_expected(&call.callee, &call.args),
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

    /// Type one context-dependent expression in a cloned semantic state. This is
    /// the transactional probe used by overload applicability: generated
    /// function/closure entities, inferred types, captures, and diagnostics are
    /// all discarded with the clone. The selected candidate is lowered once in
    /// the original state afterwards.
    pub(crate) fn probe_contextual_expr(
        &self,
        expr: &ast::Expr,
        expected: TypeId,
    ) -> Result<(), String> {
        let mut probe = self.clone();
        let diagnostics_before = probe.diagnostics.len();
        let mut sink = Vec::new();
        let value = probe.lower_expr(expr, &mut sink, Some(expected));
        let diagnostics: Vec<_> = probe.diagnostics[diagnostics_before..]
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let Some(value) = value else {
            return Err(if diagnostics.is_empty() {
                "contextual expression could not be typed".to_string()
            } else {
                diagnostics.join(", ")
            });
        };
        if !diagnostics.is_empty() {
            return Err(diagnostics.join(", "));
        }
        if !probe.is_subtype(value.ty, expected) {
            return Err(format!(
                "expression has type {}, expected {}",
                probe.type_name(value.ty),
                probe.type_name(expected)
            ));
        }
        Ok(())
    }

    pub(super) fn constructor_requires_expected(
        &self,
        name: &ast::Ident,
        args: &[ast::Expr],
    ) -> bool {
        let Some((type_param_count, fields)) = self.constructor_inference_shape(&name.text) else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if !self.expr_requires_expected_type(arg) {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.iter().any(|bound| !bound)
    }

    pub(super) fn qualified_variant_requires_expected(
        &self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        args: &[ast::Expr],
    ) -> bool {
        let ast::Expr::Var(enum_name) = receiver else {
            return false;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
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
    pub(super) fn constructor_inference_shape(&self, name: &str) -> Option<(usize, Vec<TypeId>)> {
        let variant = if let Some((enum_name, variant_name)) = name.split_once('.') {
            let enum_id = self.enums_by_name.get(enum_name).copied()?;
            self.find_variant(enum_id, variant_name)
                .map(|variant| (enum_id, variant))
        } else {
            self.option_variant(name)
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
        self.structs_by_name
            .get(name)
            .map(|(struct_id, _)| {
                (
                    self.structs[*struct_id].type_params.len(),
                    self.structs[*struct_id]
                        .semantic_fields()
                        .iter()
                        .map(|field| field.ty)
                        .collect(),
                )
            })
            .or_else(|| {
                self.classes_by_name.get(name).map(|(class_id, _)| {
                    (
                        self.classes[*class_id].type_params.len(),
                        self.classes[*class_id]
                            .semantic_constructor()
                            .iter()
                            .map(|field| field.ty)
                            .collect(),
                    )
                })
            })
    }

    pub(super) fn mark_type_params(&self, ty: TypeId, bound: &mut [bool]) {
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

    pub(super) fn unit_variant_from_field(
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
        let enum_id = self.enums_by_name.get(&enum_name.text).copied()?;
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

    /// Bind type arguments by matching a parameter (or variant field)
    /// type against the argument type: `T` binds to the argument type,
    /// `Option<T>` vs `Option<Int>` recurses (so `T = Int`) — as do
    /// other enum applications — generic struct applications
    /// (`PinnedPtr<T>`, M12) match by struct and recurse into their
    /// argument lists, and tuples match elementwise. Anything
    /// else is left to the argument type check. Returns `false` after
    /// recording a conflict diagnostic.
    pub(crate) fn bind_type_args(
        &mut self,
        param_ty: TypeId,
        arg_ty: TypeId,
        bindings: &mut [Option<TypeId>],
        type_params: &[hir::TypeParamDecl],
        span: Span,
    ) -> bool {
        match (self.types[param_ty].clone(), self.types[arg_ty].clone()) {
            (Type::Param(index), _) => {
                let index = index.into_raw() as usize;
                match bindings[index] {
                    Some(existing) => {
                        if self.types_equal(existing, arg_ty) {
                            true
                        } else {
                            let first = self.type_name(existing);
                            let second = self.type_name(arg_ty);
                            self.error(
                                span,
                                format!(
                                    "conflicting types for `{}`: {first} and {second}",
                                    type_params[index].name
                                ),
                            );
                            false
                        }
                    }
                    None => {
                        bindings[index] = Some(arg_ty);
                        true
                    }
                }
            }
            (Type::Enum(param), Type::Enum(arg)) => {
                let param = self.enum_applications[param].clone();
                let arg = self.enum_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Struct(param), Type::Struct(arg)) => {
                let param = self.struct_applications[param].clone();
                let arg = self.struct_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Class(param), Type::Class(arg)) => {
                let param = self.class_applications[param].clone();
                let arg = self.class_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Interface(param), Type::Interface(arg)) => {
                let param = self.interface_applications[param].clone();
                let arg = self.interface_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Interface(param), _) => {
                let param = self.interface_applications[param].clone();
                let Some(arg_args) = self.implemented_interface_application(arg_ty, param.template)
                else {
                    return true;
                };
                if param.arguments.len() != arg_args.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg_args) {
                    ok &= self.bind_type_args(*param, arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Ptr(param), Type::Ptr(arg)) => {
                self.bind_type_args(param, arg, bindings, type_params, span)
            }
            (Type::Tuple(param_elements), Type::Tuple(arg_elements))
                if param_elements.len() == arg_elements.len() =>
            {
                let mut ok = true;
                for (param, arg) in param_elements.iter().zip(arg_elements.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Function(param_id), Type::Function(arg_id)) => {
                let param = self.function_types[param_id].clone();
                let arg = self.function_types[arg_id].clone();
                if param.is_suspend != arg.is_suspend
                    || param.parameter_types.len() != arg.parameter_types.len()
                {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.parameter_types.iter().zip(arg.parameter_types.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok &= self.bind_type_args(
                    param.return_type,
                    arg.return_type,
                    bindings,
                    type_params,
                    span,
                );
                ok
            }
            (Type::FunPtr(param_id), Type::FunPtr(arg_id)) => {
                let param = self.function_types[param_id].clone();
                let arg = self.function_types[arg_id].clone();
                if param.is_suspend != arg.is_suspend
                    || param.parameter_types.len() != arg.parameter_types.len()
                {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.parameter_types.iter().zip(arg.parameter_types.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok &= self.bind_type_args(
                    param.return_type,
                    arg.return_type,
                    bindings,
                    type_params,
                    span,
                );
                ok
            }
            _ => true,
        }
    }
}
