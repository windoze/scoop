//! Calls, callable references, anonymous functions, lambdas and callbacks.

use super::*;

impl Lowerer {
    /// `Name(args...)` in call position: a variant or struct
    /// construction when the name resolves as one, a direct function
    /// call otherwise.
    pub(super) fn lower_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some(local) = self.scopes.lookup(&call.callee.text) {
            let ty = self
                .smart_casts
                .get(&local)
                .copied()
                .unwrap_or(self.locals[local].ty);
            if matches!(self.types[ty], Type::Function(_)) {
                if !call.type_args.is_empty() {
                    self.error(
                        call.callee.span,
                        "function values do not accept explicit type arguments".to_string(),
                    );
                    return None;
                }
                let callee = hir::Expr {
                    kind: ExprKind::Local(local),
                    ty,
                    span: call.callee.span,
                };
                return self.lower_callable_call(callee, &call.args, call.span, sink);
            }
        }
        if let Some(capture) = self.available_capture(&call.callee.text)
            && matches!(self.types[capture.ty], Type::Function(_))
        {
            if !call.type_args.is_empty() {
                self.error(
                    call.callee.span,
                    "function values do not accept explicit type arguments".to_string(),
                );
                return None;
            }
            let callee = self.lower_capture(&call.callee)?;
            return self.lower_callable_call(callee, &call.args, call.span, sink);
        }
        // An intrinsic array class in constructor position denotes the
        // opposite-family snapshot conversion. The class namespace resolves
        // the source name; the typed declaration kind selects the operation.
        if let Some(&(class, _)) = self.classes_by_name.get(&call.callee.text) {
            let target_kind = match self.classes[class].representation {
                hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeDeclaration {
                    kind: hir::IntrinsicTypeKind::Array,
                    ..
                }) => Some(ArrayKind::Immutable),
                hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeDeclaration {
                    kind: hir::IntrinsicTypeKind::MutableArray,
                    ..
                }) => Some(ArrayKind::Mutable),
                hir::ClassRepresentation::Declared(_) | hir::ClassRepresentation::Intrinsic(_) => {
                    None
                }
            };
            if let Some(target_kind) = target_kind {
                return self.lower_array_conversion(call, sink, target_kind);
            }
        }
        if let Some(core) = self.foreign_callback_core
            && !self
                .functions_by_name
                .get(&call.callee.text)
                .is_some_and(|functions| {
                    functions
                        .iter()
                        .any(|function| self.function_files[function] == self.user_file_index)
                })
        {
            if call.callee.text == "foreignCallback" {
                return self.lower_foreign_callback_registration(core, call, sink);
            }
            if let Some(operation) = self.foreign_callback_operation(core, &call.callee.text) {
                return self.lower_foreign_callback_call(core, operation, call, sink);
            }
        }
        match self.classify_constructor(&call.callee)? {
            Constructor::Variant { enum_id, variant } => self.lower_variant_construct(
                enum_id,
                variant,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
                expected,
            ),
            Constructor::Struct { struct_id, ty } => {
                if Some(struct_id) == self.ffi_foreign_callback {
                    self.error(
                        call.span,
                        "`ForeignCallback` values can only be produced by `foreignCallback`"
                            .to_string(),
                    );
                    return None;
                }
                let site = CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                };
                if Some(struct_id) == self.ffi_ptr || Some(struct_id) == self.ffi_fun_ptr {
                    self.lower_ffi_struct_init(struct_id, site, sink, expected)
                } else {
                    self.lower_struct_init(struct_id, ty, site, sink, expected)
                }
            }
            Constructor::Class { class_id } => self.lower_class_construct(
                class_id,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
                expected,
            ),
            Constructor::Unmatched => self.lower_function_call(call, sink),
        }
    }

    pub(super) fn foreign_callback_operation(
        &self,
        core: hir::ForeignCallbackCore,
        name: &str,
    ) -> Option<hir::ForeignCallbackOperation> {
        match name {
            "foreignCallback" => None,
            "retainForeignCallback" => Some(hir::ForeignCallbackOperation::Retain),
            "releaseForeignCallback" => Some(hir::ForeignCallbackOperation::Release),
            "foreignCallbackState" => Some(hir::ForeignCallbackOperation::State),
            "foreignCallbackFailure" => Some(hir::ForeignCallbackOperation::Failure),
            _ => return None,
        }
        .filter(|operation| {
            let function = match operation {
                hir::ForeignCallbackOperation::Retain => core.retain,
                hir::ForeignCallbackOperation::Release => core.release,
                hir::ForeignCallbackOperation::State => core.query_state,
                hir::ForeignCallbackOperation::Failure => core.failure,
            };
            self.functions[function].name == name
        })
    }

    pub(super) fn lower_foreign_callback_registration(
        &mut self,
        core: hir::ForeignCallbackCore,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if call.type_args.len() != 1 || call.args.len() != 3 {
            self.error(
                call.span,
                "`foreignCallback` requires one explicit function type and exactly three arguments"
                    .to_string(),
            );
            return None;
        }
        let native_ty = self.resolve_type_ref(&call.type_args[0])?;
        let hir::Type::Function(native_function_type) = self.types[native_ty] else {
            self.error(
                call.type_args[0].span,
                "`foreignCallback` type argument must be an ordinary concrete function type"
                    .to_string(),
            );
            return None;
        };
        if !self.validate_foreign_callback_signature(native_function_type, call.span) {
            return None;
        }
        let native_signature = self.function_types[native_function_type].clone();
        let ast::Expr::IntLiteral {
            value: context_index,
            span: context_span,
        } = &call.args[1]
        else {
            self.error(
                call.args[1].span(),
                "foreign callback `contextIndex` must be a compile-time integer literal"
                    .to_string(),
            );
            return None;
        };
        if *context_index < 0 || *context_index as usize >= native_signature.parameter_types.len() {
            self.error(
                *context_span,
                "foreign callback `contextIndex` is outside the native signature".to_string(),
            );
            return None;
        }
        let context_type = native_signature.parameter_types[*context_index as usize];
        if !matches!(self.types[context_type], hir::Type::Ptr(pointee) if pointee == self.unit) {
            self.error(
                *context_span,
                "foreign callback context parameter must be exactly `Ptr<Unit>`".to_string(),
            );
            return None;
        }

        let managed_parameters = native_signature
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != *context_index as usize).then_some(*ty))
            .collect();
        let managed_ty =
            self.intern_function_type(false, managed_parameters, native_signature.return_type);
        let hir::Type::Function(managed_function_type) = self.types[managed_ty] else {
            unreachable!("interned managed callback signature is a function type")
        };
        let closure = self.lower_expr(&call.args[0], sink, Some(managed_ty))?;
        if !self.types_equal(closure.ty, managed_ty) {
            self.error(
                closure.span,
                format!(
                    "foreign callback closure must have type {}, found {}",
                    self.type_name(managed_ty),
                    self.type_name(closure.ty)
                ),
            );
            return None;
        }

        let mode_ty = self.interned_enum_type(core.mode);
        let mode = self.lower_expr(&call.args[2], sink, Some(mode_ty))?;
        let ExprKind::VariantConstruct {
            application,
            variant,
            args,
        } = mode.kind
        else {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".to_string(),
            );
            return None;
        };
        if self.enum_applications[application].template != core.mode
            || !args.is_empty()
            || variant > 1
        {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".to_string(),
            );
            return None;
        }
        let mode = if variant == 0 {
            hir::ForeignCallbackMode::Reusable
        } else {
            hir::ForeignCallbackMode::OneShot
        };
        self.check_call_effects(hir::Callable::Function(core.register), call.span);
        let registration =
            self.foreign_callback_registrations
                .alloc(hir::ForeignCallbackRegistration {
                    native_function_type,
                    managed_function_type,
                    context_index: *context_index as u32,
                    mode,
                });
        let ty = self.struct_application(core.callback, vec![native_ty]);
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackRegister {
                registration,
                closure: Box::new(closure),
            },
            ty,
            span: call.span,
        })
    }

    pub(super) fn lower_foreign_callback_call(
        &mut self,
        core: hir::ForeignCallbackCore,
        operation: hir::ForeignCallbackOperation,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = match operation {
            hir::ForeignCallbackOperation::Retain => core.retain,
            hir::ForeignCallbackOperation::Release => core.release,
            hir::ForeignCallbackOperation::State => core.query_state,
            hir::ForeignCallbackOperation::Failure => core.failure,
        };
        if call.args.len() != 1 || call.type_args.len() > 1 {
            self.error(
                call.span,
                format!("`{}` expects one callback value", call.callee.text),
            );
            return None;
        }
        let explicit = if let Some(ty) = call.type_args.first() {
            Some(self.resolve_type_ref(ty)?)
        } else {
            None
        };
        let expected_callback =
            explicit.map(|function| self.struct_application(core.callback, vec![function]));
        let callback = self.lower_expr(&call.args[0], sink, expected_callback)?;
        let hir::Type::Struct(application) = self.types[callback.ty] else {
            self.error(
                callback.span,
                "managed callback token operation requires `ForeignCallback<F>`".to_string(),
            );
            return None;
        };
        let application = self.struct_applications[application].clone();
        if application.template != core.callback || application.arguments.len() != 1 {
            self.error(
                callback.span,
                "managed callback token operation requires `ForeignCallback<F>`".to_string(),
            );
            return None;
        }
        let function_ty = application.arguments[0];
        if !matches!(self.types[function_ty], hir::Type::Function(_))
            || explicit.is_some_and(|explicit| !self.types_equal(explicit, function_ty))
        {
            self.error(
                callback.span,
                "`ForeignCallback` type argument must be one concrete function type".to_string(),
            );
            return None;
        }
        self.check_call_effects(hir::Callable::Function(function), call.span);
        let return_ty = self.instantiate_ty(self.signatures[&function].return_ty, &[function_ty]);
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackOperation {
                operation,
                callback: Box::new(callback),
            },
            ty: return_ty,
            span: call.span,
        })
    }

    pub(super) fn lower_callable_call(
        &mut self,
        callee: hir::Expr,
        args: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let Type::Function(function_type) = self.types[callee.ty] else {
            let found = self.type_name(callee.ty);
            self.error(
                callee.span,
                format!("value of type {found} is not callable"),
            );
            return None;
        };
        let signature = self.function_types[function_type].clone();
        if signature.parameter_types.len() != args.len() {
            self.error(
                span,
                format!(
                    "function value takes exactly {} argument(s), but {} were supplied",
                    signature.parameter_types.len(),
                    args.len()
                ),
            );
            return None;
        }
        if signature.is_suspend {
            let context = *self
                .suspension_contexts
                .last()
                .expect("the suspension context stack is initialized");
            if let SuspensionContext::Forbidden(reason) = context {
                let location = match reason {
                    ForbiddenSuspendContext::TopLevel => "a non-suspend declaration".to_string(),
                    ForbiddenSuspendContext::Function => {
                        format!("non-suspend function `{}`", self.current_fn_name)
                    }
                    ForbiddenSuspendContext::ConstructorDelegation => {
                        "constructor delegation".to_string()
                    }
                };
                self.error(
                    span,
                    format!("suspend function value cannot be called from {location}"),
                );
                return None;
            }
        }
        let mut lowered = Vec::with_capacity(args.len());
        for (arg, &parameter_ty) in args.iter().zip(&signature.parameter_types) {
            let value = self.lower_expr(arg, sink, Some(parameter_ty))?;
            if !self.is_subtype(value.ty, parameter_ty) {
                let expected = self.type_name(parameter_ty);
                let found = self.type_name(value.ty);
                self.error(
                    arg.span(),
                    format!("function argument must be of type {expected}, found {found}"),
                );
                return None;
            }
            lowered.push(self.adapt_to(value, parameter_ty));
        }
        Some(hir::Expr {
            kind: ExprKind::CallableCall {
                callee: Box::new(callee),
                function_type,
                args: lowered,
            },
            ty: signature.return_type,
            span,
        })
    }

    pub(super) fn lower_callable_reference(
        &mut self,
        receiver: Option<&ast::Expr>,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if let Some(expected) = expected
            && let Type::FunPtr(signature) = self.types[expected]
        {
            if receiver.is_some() {
                self.error(
                    span,
                    "a native `FunPtr` address must reference an unbound top-level function"
                        .to_string(),
                );
                return None;
            }
            return self.lower_native_function_reference(name, span, expected, signature);
        }
        if let Some(receiver) = receiver {
            return self.lower_bound_callable_reference(receiver, name, span, expected, sink);
        }
        if let Some(local) = self.scopes.lookup(&name.text)
            && matches!(self.types[self.locals[local].ty], Type::Function(_))
        {
            self.error(
                span,
                format!(
                    "`::{}` cannot reference an existing function value; use `{}` directly",
                    name.text, name.text
                ),
            );
            return None;
        }
        let local_candidates = self.local_function_scopes.lookup(&name.text);
        if !local_candidates.is_empty() {
            return self.lower_local_callable_reference(local_candidates, name, span, expected);
        }
        let candidates = self.named_reference_candidate_layer(&name.text);
        if candidates.is_empty() {
            if self.is_declared_type_name(&name.text) {
                self.error(
                    span,
                    format!(
                        "constructor reference `::{}` is not supported; construct the value in a lambda",
                        name.text
                    ),
                );
                return None;
            }
            self.error(name.span, format!("unknown function `{}`", name.text));
            return None;
        }
        let expected_signature = self.expected_function_signature(expected);
        let display = format!("callable reference `::{}`", name.text);
        let resolved = self.resolve_reference_candidates(
            &candidates,
            &[],
            expected_signature.as_ref(),
            &display,
            span,
            ReferenceExtensionMode::IncludeUnbound,
        )?;
        let callee = resolved.callable;
        let ty = resolved.ty;
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let id = self.callable_references.alloc(hir::CallableReference {
            target: hir::CallableReferenceTarget::Named(callee),
            function_type,
            owner_type_param_count: self.type_params_in_scope.len(),
            captures: Vec::new(),
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
        })
    }

    pub(super) fn lower_native_function_reference(
        &mut self,
        name: &ast::Ident,
        span: Span,
        expected: TypeId,
        signature: hir::FunctionTypeId,
    ) -> Option<hir::Expr> {
        if self.scopes.lookup(&name.text).is_some()
            || !self.local_function_scopes.lookup(&name.text).is_empty()
        {
            self.error(
                span,
                "a native `FunPtr` address cannot target a local function or function value"
                    .to_string(),
            );
            return None;
        }
        let expected_signature = self.function_types[signature].clone();
        let candidates = self.named_reference_candidate_layer(&name.text);
        let mut matching = Vec::new();
        for function in candidates {
            let declaration = &self.functions[function];
            let sig = &self.signatures[&function];
            if declaration.method.is_some()
                || self.extension_receivers.contains_key(&function)
                || !sig.type_params.is_empty()
                || sig.is_suspend
                || declaration.attributes.gc_effect != hir::GcEffect::NoGc
                || !matches!(declaration.kind, hir::FunctionKind::User(_))
                || sig.params.len() != expected_signature.parameter_types.len()
            {
                continue;
            }
            let params_match = sig
                .params
                .iter()
                .zip(&expected_signature.parameter_types)
                .all(|(parameter, expected)| self.types_equal(parameter.ty, *expected));
            if params_match && self.types_equal(sig.return_ty, expected_signature.return_type) {
                matching.push(function);
            }
        }
        let function = match matching.as_slice() {
            [function] => *function,
            [] => {
                self.error(
                    span,
                    format!(
                        "no eligible `@NoGC` top-level function `::{}` exactly matches the expected FunPtr signature",
                        name.text
                    ),
                );
                return None;
            }
            _ => {
                self.error(
                    span,
                    format!(
                        "native function reference `::{}` is ambiguous for the expected FunPtr signature",
                        name.text
                    ),
                );
                return None;
            }
        };
        if self.functions[function].attributes.safety == hir::Safety::Unsafe {
            self.require_unsafe_operation(span, "taking the address of an unsafe callback");
        }
        Some(hir::Expr {
            kind: ExprKind::FunctionAddress(function),
            ty: expected,
            span,
        })
    }

    pub(super) fn lower_bound_callable_reference(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if let ast::Expr::Var(type_name) = receiver
            && self.scopes.lookup(&type_name.text).is_none()
            && !self.host_has_property(&type_name.text)
            && (self.is_declared_type_name(&type_name.text)
                || self
                    .type_params_in_scope
                    .iter()
                    .any(|param| param.name == type_name.text))
        {
            self.error(
                span,
                format!(
                    "unbound member reference `{}::{}` is not supported; bind an expression receiver first",
                    type_name.text, name.text
                ),
            );
            return None;
        }
        // The source expression is retained on the reference entity and becomes
        // the first closure field initializer in MIR. It is therefore evaluated
        // once at reference creation, including when it reads a mutable local.
        let receiver = self.lower_expr(receiver, sink, None)?;
        let mut member_candidates = self.methods_by_name(receiver.ty, &name.text);
        let is_extension = member_candidates.is_empty();
        let extension_candidates = if is_extension {
            let candidates = self.extension_candidate_layer(&name.text);
            if candidates.is_empty() {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!("type `{found}` has no method `{}`", name.text),
                );
                return None;
            }
            candidates
        } else {
            Vec::new()
        };
        if !is_extension && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = member_candidates.len();
            member_candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if member_candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be referenced through interface type `{found}`",
                        name.text
                    ),
                );
                return None;
            }
        }
        let expected_signature = self.expected_function_signature(expected);
        let display = format!("bound callable reference `receiver::{}`", name.text);
        let resolved = if is_extension {
            self.resolve_reference_candidates(
                &extension_candidates,
                &[],
                expected_signature.as_ref(),
                &display,
                span,
                ReferenceExtensionMode::Bound(receiver.ty),
            )?
        } else {
            self.resolve_member_reference_candidates(
                &member_candidates,
                expected_signature.as_ref(),
                &display,
                span,
            )?
        };
        let callee = resolved.callable;
        let ty = resolved.ty;
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let target = if is_extension {
            hir::CallableReferenceTarget::BoundExtension {
                receiver: Box::new(receiver),
                callee,
            }
        } else {
            hir::CallableReferenceTarget::BoundMember {
                receiver: Box::new(receiver),
                callee: self.materialize_method_callee(
                    resolved.source,
                    callee,
                    &resolved.type_args,
                ),
            }
        };
        let id = self.callable_references.alloc(hir::CallableReference {
            target,
            function_type,
            owner_type_param_count: self.type_params_in_scope.len(),
            captures: Vec::new(),
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
        })
    }

    pub(super) fn is_declared_type_name(&self, name: &str) -> bool {
        self.classes_by_name.contains_key(name)
            || self.interfaces_by_name.contains_key(name)
            || self.structs_by_name.contains_key(name)
            || self.enums_by_name.contains_key(name)
    }

    pub(super) fn expected_function_signature(
        &self,
        expected: Option<TypeId>,
    ) -> Option<(TypeId, hir::FunctionType)> {
        expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        })
    }

    /// Resolve one top-level or member callable-reference candidate layer.
    /// Expected function types bind generic parameters in both parameter and
    /// return positions. Without one, only candidates with no declaration-owned
    /// type parameters can produce a concrete function value.
    pub(super) fn resolve_reference_candidates(
        &mut self,
        candidates: &[hir::FunctionId],
        owner_type_args: &[TypeId],
        expected: Option<&(TypeId, hir::FunctionType)>,
        display: &str,
        span: Span,
        extension_mode: ReferenceExtensionMode,
    ) -> Option<ResolvedReference> {
        let candidates = candidates
            .iter()
            .copied()
            .map(|function| crate::CallableCandidate::function(function, owner_type_args.to_vec()))
            .collect::<Vec<_>>();
        self.resolve_reference_candidate_set(&candidates, expected, display, span, extension_mode)
    }

    pub(super) fn resolve_member_reference_candidates(
        &mut self,
        candidates: &[crate::CallableCandidate],
        expected: Option<&(TypeId, hir::FunctionType)>,
        display: &str,
        span: Span,
    ) -> Option<ResolvedReference> {
        self.resolve_reference_candidate_set(
            candidates,
            expected,
            display,
            span,
            ReferenceExtensionMode::Exclude,
        )
    }

    pub(super) fn resolve_reference_candidate_set(
        &mut self,
        candidates: &[crate::CallableCandidate],
        expected: Option<&(TypeId, hir::FunctionType)>,
        display: &str,
        span: Span,
        extension_mode: ReferenceExtensionMode,
    ) -> Option<ResolvedReference> {
        let mut applicable = Vec::new();
        for candidate in candidates {
            let function = candidate.function;
            let owner_type_args = self.callable_candidate_owner_arguments(candidate);
            let sig = self.signatures[&function].clone();
            if sig.owner_type_param_count != owner_type_args.len() {
                continue;
            }
            let mut bindings = vec![None; sig.type_params.len()];
            for (binding, &ty) in bindings.iter_mut().zip(&owner_type_args) {
                *binding = Some(ty);
            }
            let extension_receiver = self.extension_receivers.get(&function).copied();
            let bound_extension_receiver = match extension_mode {
                ReferenceExtensionMode::Exclude if extension_receiver.is_some() => continue,
                ReferenceExtensionMode::Bound(_) if extension_receiver.is_none() => continue,
                ReferenceExtensionMode::Bound(receiver) => Some(receiver),
                ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
            };
            if let (Some(declared), Some(actual)) = (extension_receiver, bound_extension_receiver)
                && !self.try_bind(declared, actual, &mut bindings)
            {
                continue;
            }
            let mut reference_params: Vec<_> =
                sig.params.iter().map(|parameter| parameter.ty).collect();
            if matches!(extension_mode, ReferenceExtensionMode::IncludeUnbound)
                && let Some(receiver) = extension_receiver
            {
                reference_params.insert(0, receiver);
            }
            match expected {
                Some((_, expected)) => {
                    if sig.is_suspend != expected.is_suspend
                        || reference_params.len() != expected.parameter_types.len()
                    {
                        continue;
                    }
                    let parameters_match = reference_params
                        .iter()
                        .zip(&expected.parameter_types)
                        .all(|(&parameter, &expected)| {
                            self.try_bind(parameter, expected, &mut bindings)
                        });
                    if !parameters_match
                        || !self.try_bind(sig.return_ty, expected.return_type, &mut bindings)
                        || bindings.iter().any(Option::is_none)
                    {
                        continue;
                    }
                }
                None => {
                    if sig.type_params.len() != sig.owner_type_param_count {
                        continue;
                    }
                }
            }
            let type_args: Vec<_> = bindings.into_iter().flatten().collect();
            if !self.type_arguments_satisfy_kinds(&sig.type_params, &type_args) {
                continue;
            }
            let parameter_types: Vec<_> = reference_params
                .iter()
                .map(|&parameter| self.instantiate_ty(parameter, &type_args))
                .collect();
            let return_type = self.instantiate_ty(sig.return_ty, &type_args);
            if let (Some(declared), Some(actual)) = (extension_receiver, bound_extension_receiver) {
                let declared = self.instantiate_ty(declared, &type_args);
                if !self.is_subtype(actual, declared) {
                    continue;
                }
            }
            if let Some((_, expected)) = expected {
                let exact = parameter_types
                    .iter()
                    .zip(&expected.parameter_types)
                    .all(|(&parameter, &expected)| self.types_equal(parameter, expected))
                    && self.types_equal(return_type, expected.return_type);
                if !exact {
                    continue;
                }
            }
            let own_type_param_count = sig.type_params.len() - sig.owner_type_param_count;
            applicable.push((
                function,
                candidate.source,
                candidate.owner.clone(),
                type_args,
                parameter_types,
                return_type,
                sig.is_suspend,
                own_type_param_count,
            ));
        }

        let selected = match applicable.len() {
            0 => {
                let message = if expected.is_some() {
                    format!("no overload of {display} matches the expected function type")
                } else {
                    format!(
                        "cannot determine {display} without an expected function type; no unique non-generic candidate exists"
                    )
                };
                self.error(span, message);
                return None;
            }
            1 => 0,
            _ if expected.is_some() => {
                let concrete: Vec<_> = applicable
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| (candidate.7 == 0).then_some(index))
                    .collect();
                if let [index] = concrete.as_slice() {
                    *index
                } else {
                    self.error(
                        span,
                        format!("{display} is ambiguous for the expected function type"),
                    );
                    return None;
                }
            }
            _ => {
                self.error(
                    span,
                    format!("{display} is ambiguous; provide an expected function type"),
                );
                return None;
            }
        };
        let (function, source, owner, type_args, parameter_types, return_type, is_suspend, _) =
            applicable.swap_remove(selected);
        let ty = match expected {
            Some((ty, _)) => *ty,
            None => self.intern_function_type(is_suspend, parameter_types, return_type),
        };
        let selected_candidate = crate::CallableCandidate {
            function,
            owner,
            source,
        };
        let callee = self.materialize_candidate_callable(&selected_candidate, &type_args);
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
        Some(ResolvedReference {
            callable: callee,
            source,
            type_args,
            ty,
        })
    }

    pub(super) fn lower_local_callable_reference(
        &mut self,
        candidates: Vec<hir::LocalFunctionId>,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        });
        let mut applicable = Vec::new();
        for local_function in candidates {
            let function = self.local_functions[local_function].function;
            let sig = self.signatures[&function].clone();
            let mut bindings = vec![None; sig.type_params.len()];
            for (binding, ty) in bindings
                .iter_mut()
                .zip(self.ambient_type_args(sig.owner_type_param_count))
            {
                *binding = Some(ty);
            }
            match &expected_signature {
                Some((_, expected)) => {
                    if sig.is_suspend != expected.is_suspend
                        || sig.params.len() != expected.parameter_types.len()
                    {
                        continue;
                    }
                    let mut matches = true;
                    for (parameter, expected) in sig.params.iter().zip(&expected.parameter_types) {
                        matches &= self.try_bind(parameter.ty, *expected, &mut bindings);
                    }
                    matches &= self.try_bind(sig.return_ty, expected.return_type, &mut bindings);
                    if !matches || bindings.iter().any(Option::is_none) {
                        continue;
                    }
                    let type_args: Vec<_> = bindings.into_iter().flatten().collect();
                    if !self.type_arguments_satisfy_kinds(&sig.type_params, &type_args) {
                        continue;
                    }
                    let instantiated_params: Vec<_> = sig
                        .params
                        .iter()
                        .map(|parameter| self.instantiate_ty(parameter.ty, &type_args))
                        .collect();
                    let instantiated_return = self.instantiate_ty(sig.return_ty, &type_args);
                    let exact = instantiated_params
                        .iter()
                        .zip(&expected.parameter_types)
                        .all(|(parameter, expected)| self.types_equal(*parameter, *expected))
                        && self.types_equal(instantiated_return, expected.return_type);
                    if exact {
                        let own_type_param_count =
                            sig.type_params.len() - sig.owner_type_param_count;
                        applicable.push((local_function, type_args, own_type_param_count));
                    }
                }
                None => {
                    if sig.type_params.len() != sig.owner_type_param_count {
                        continue;
                    }
                    applicable.push((local_function, bindings.into_iter().flatten().collect(), 0));
                }
            }
        }
        let selected = match applicable.len() {
            0 => {
                self.error(
                    span,
                    format!(
                        "no local overload of `::{}` matches the expected function type",
                        name.text
                    ),
                );
                return None;
            }
            1 => 0,
            _ if expected_signature.is_some() => {
                let concrete: Vec<_> = applicable
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| (candidate.2 == 0).then_some(index))
                    .collect();
                if let [index] = concrete.as_slice() {
                    *index
                } else {
                    self.error(
                        span,
                        format!(
                            "local callable reference `::{}` is ambiguous for the expected function type",
                            name.text
                        ),
                    );
                    return None;
                }
            }
            _ => {
                self.error(
                    span,
                    format!(
                        "local callable reference `::{}` is ambiguous; provide an expected function type",
                        name.text
                    ),
                );
                return None;
            }
        };
        let (local_function, type_args, _) = applicable.swap_remove(selected);
        let local = &self.local_functions[local_function];
        let function = local.function;
        let ty = expected_signature.map_or_else(
            || {
                let signature = self.signatures[&function].clone();
                let parameters = signature
                    .params
                    .iter()
                    .map(|parameter| self.instantiate_ty(parameter.ty, &type_args))
                    .collect();
                let return_ty = self.instantiate_ty(signature.return_ty, &type_args);
                self.intern_function_type(signature.is_suspend, parameters, return_ty)
            },
            |(ty, _)| ty,
        );
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let capture_sources = self.local_call_capture_args(local_function, span)?;
        let capture_specs: Vec<_> = self.local_functions[local_function]
            .captures
            .iter()
            .map(|capture| {
                (
                    capture.binding,
                    capture.name.clone(),
                    capture.ty,
                    capture.first_use_span,
                )
            })
            .collect();
        let captures = capture_specs
            .into_iter()
            .zip(capture_sources)
            .map(
                |((binding, name, ty, first_use_span), source)| hir::Capture {
                    binding,
                    name,
                    ty: self.instantiate_ty(ty, &type_args),
                    first_use_span,
                    source,
                },
            )
            .collect();
        let callee = if type_args.is_empty() {
            hir::Callable::Function(function)
        } else {
            hir::Callable::Generic(self.record_instantiation(function, type_args))
        };
        let id = self.callable_references.alloc(hir::CallableReference {
            target: hir::CallableReferenceTarget::Local {
                local_function,
                callee,
            },
            function_type,
            owner_type_param_count: self.type_params_in_scope.len(),
            captures,
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
        })
    }

    pub(super) fn top_level_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        self.candidate_layer(self.functions_by_name.get(name))
    }

    pub(super) fn managed_reference_target_is_safe(
        &mut self,
        callee: hir::Callable,
        span: Span,
    ) -> bool {
        let function = self.callable_function_id(callee);
        if self.functions[function].attributes.safety == hir::Safety::Safe {
            return true;
        }
        self.error(
            span,
            format!(
                "unsafe function `{}` cannot be stored in a managed function type because safety is not part of function-type identity",
                self.functions[function].name
            ),
        );
        false
    }

    pub(super) fn extension_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        self.candidate_layer(self.extensions_by_name.get(name))
    }

    pub(super) fn candidate_layer(
        &self,
        ids: Option<&Vec<hir::FunctionId>>,
    ) -> Vec<hir::FunctionId> {
        let Some(ids) = ids else {
            return Vec::new();
        };
        let call_site_is_core = self.current_file < self.user_file_index;
        let same_side: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| (self.function_files[id] < self.user_file_index) == call_site_is_core)
            .collect();
        if same_side.is_empty() {
            ids.iter()
                .copied()
                .filter(|id| (self.function_files[id] < self.user_file_index) != call_site_is_core)
                .collect()
        } else {
            same_side
        }
    }

    /// The top-level callable-reference layer includes ordinary and extension
    /// declarations. Package/core precedence is applied to the combined set,
    /// then declaration order is restored by the globally unique function id.
    pub(super) fn named_reference_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        let mut ids = Vec::new();
        ids.extend(
            self.functions_by_name
                .get(name)
                .into_iter()
                .flatten()
                .copied(),
        );
        ids.extend(
            self.extensions_by_name
                .get(name)
                .into_iter()
                .flatten()
                .copied(),
        );
        let call_site_is_core = self.current_file < self.user_file_index;
        let same_side: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| (self.function_files[id] < self.user_file_index) == call_site_is_core)
            .collect();
        let mut selected = if same_side.is_empty() {
            ids.into_iter()
                .filter(|id| (self.function_files[id] < self.user_file_index) != call_site_is_core)
                .collect::<Vec<_>>()
        } else {
            same_side
        };
        selected.sort_by_key(|id| id.into_raw().into_u32());
        selected
    }

    pub(super) fn lower_lambda(
        &mut self,
        is_suspend: bool,
        parameters: Option<&[ast::LambdaParam]>,
        body: &ast::Block,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if block_contains_return(body) {
            self.error(
                span,
                "a lambda cannot use `return`; use an anonymous function for local returns"
                    .to_string(),
            );
            return None;
        }
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        });
        if let Some((_, signature)) = &expected_signature
            && signature.is_suspend != is_suspend
        {
            self.error(
                span,
                "ordinary and suspend function types are incompatible".to_string(),
            );
            return None;
        }
        let source_parameters: Vec<Option<&ast::LambdaParam>> = match parameters {
            Some(parameters) => parameters.iter().map(Some).collect(),
            None => match &expected_signature {
                Some((_, signature)) if signature.parameter_types.len() == 1 => vec![None],
                Some((_, signature)) if signature.parameter_types.is_empty() => Vec::new(),
                Some((_, signature)) => {
                    self.error(
                        span,
                        format!(
                            "lambda omits its parameter list, but the expected type has {} parameters",
                            signature.parameter_types.len()
                        ),
                    );
                    return None;
                }
                None => Vec::new(),
            },
        };
        if let Some((_, signature)) = &expected_signature
            && source_parameters.len() != signature.parameter_types.len()
        {
            self.error(
                span,
                format!(
                    "lambda has {} parameter(s), but the expected function type has {}",
                    source_parameters.len(),
                    signature.parameter_types.len()
                ),
            );
            return None;
        }

        let capture_environment = self.capture_environment();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let outer_return_ty = self.current_return_ty;
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_owner = self.current_owner;
        let outer_this = self.current_this.take();
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        let function_number = self.next_lambda_function;
        self.next_lambda_function += 1;
        self.current_fn_name = format!("$lambda.{function_number}");
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();

        let lowered = (|| {
            let mut abi_params = Vec::with_capacity(source_parameters.len());
            let mut parameter_types = Vec::with_capacity(source_parameters.len());
            let mut prefix = Vec::new();
            for (index, parameter) in source_parameters.iter().enumerate() {
                let expected_ty = expected_signature
                    .as_ref()
                    .map(|(_, signature)| signature.parameter_types[index]);
                let explicit_ty = match parameter.and_then(|parameter| parameter.ty.as_ref()) {
                    Some(ty) => Some(self.resolve_type_ref(ty)?),
                    None => None,
                };
                let parameter_ty = match (explicit_ty, expected_ty) {
                    (Some(explicit), Some(expected)) => {
                        if !self.types_equal(explicit, expected) {
                            let found = self.type_name(explicit);
                            let expected = self.type_name(expected);
                            let at = parameter
                                .expect("an explicit type belongs to a parameter")
                                .span;
                            self.error(
                                at,
                                format!(
                                    "lambda parameter type is {found}, but the expected type is {expected}"
                                ),
                            );
                            return None;
                        }
                        explicit
                    }
                    (Some(explicit), None) => explicit,
                    (None, Some(expected)) => expected,
                    (None, None) => {
                        let at = parameter.map_or(span, |parameter| parameter.span);
                        self.error(
                            at,
                            "lambda parameter requires a type when there is no expected function type"
                                .to_string(),
                        );
                        return None;
                    }
                };
                parameter_types.push(parameter_ty);
                let target = parameter.map(|parameter| &parameter.target);
                let binding_name = match target {
                    Some(ast::Pattern::Binding(name)) => Some(name.clone()),
                    None => Some(ast::Ident {
                        text: "it".to_string(),
                        span,
                    }),
                    _ => None,
                };
                if let Some(name) = binding_name {
                    let pattern = ast::Pattern::Binding(name.clone());
                    let hir::Pattern::Binding { local } = self.lower_pattern(
                        &pattern,
                        parameter_ty,
                        PatternCtx {
                            mutable: false,
                            in_when: false,
                        },
                    )?
                    else {
                        unreachable!("a binding parameter lowers to a binding")
                    };
                    abi_params.push(hir::Param {
                        name: name.text,
                        ty: parameter_ty,
                        local,
                    });
                } else {
                    let local = self.alloc_local(format!("$arg.{index}"), parameter_ty, false);
                    let pattern = self.lower_pattern(
                        target.expect("non-binding source parameter has a pattern"),
                        parameter_ty,
                        PatternCtx {
                            mutable: false,
                            in_when: false,
                        },
                    )?;
                    prefix.push(hir::Statement {
                        kind: hir::StatementKind::ValDecl {
                            pattern,
                            init: hir::Expr {
                                kind: ExprKind::Local(local),
                                ty: parameter_ty,
                                span,
                            },
                        },
                        span,
                    });
                    abi_params.push(hir::Param {
                        name: format!("$arg.{index}"),
                        ty: parameter_ty,
                        local,
                    });
                }
            }

            let expected_return = expected_signature
                .as_ref()
                .map(|(_, signature)| signature.return_type);
            let mut value_block = self.lower_value_block(body, expected_return)?;
            let return_ty = value_block
                .value
                .as_ref()
                .map_or(expected_return.unwrap_or(self.unit), |value| value.ty);
            if let Some(expected_return) = expected_return
                && !self.types_equal(expected_return, self.unit)
                && !self.types_equal(return_ty, expected_return)
            {
                let expected = self.type_name(expected_return);
                let found = self.type_name(return_ty);
                self.error(
                    body.span,
                    format!("lambda result must be of type {expected}, found {found}"),
                );
                return None;
            }
            let return_ty = expected_return.unwrap_or(return_ty);
            self.current_return_ty = return_ty;
            prefix.append(&mut value_block.statements);
            if let Some(value) = value_block.value.take() {
                if self.types_equal(return_ty, self.unit) {
                    if !matches!(value.kind, ExprKind::UnitLiteral) {
                        prefix.push(hir::Statement {
                            span: value.span,
                            kind: hir::StatementKind::Expr(value),
                        });
                    }
                    prefix.push(hir::Statement {
                        span: body.span,
                        kind: hir::StatementKind::Return { value: None },
                    });
                } else {
                    let value = self.adapt_to(value, return_ty);
                    prefix.push(hir::Statement {
                        span: value.span,
                        kind: hir::StatementKind::Return { value: Some(value) },
                    });
                }
            }
            let function_ty = self.intern_function_type(is_suspend, parameter_types, return_ty);
            let Type::Function(function_type) = self.types[function_ty] else {
                unreachable!()
            };
            let closure_local = self.alloc_local("$closure".to_string(), function_ty, false);
            let mut params = Vec::with_capacity(abi_params.len() + 1);
            params.push(hir::Param {
                name: "$closure".to_string(),
                ty: function_ty,
                local: closure_local,
            });
            params.extend(abi_params);
            let type_params = self.type_params_in_scope.clone();
            let function = self.functions.alloc(hir::Function {
                name: self.current_fn_name.clone(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend,
                params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: std::mem::take(&mut self.locals),
                    statements: prefix,
                }),
                method: None,
                span,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            let captures = self.finish_current_captures();
            let id = self.lambdas.alloc(hir::Lambda {
                function,
                function_type,
                owner_type_param_count: type_params.len(),
                captures,
                span,
            });
            Some(hir::Expr {
                kind: ExprKind::Lambda(id),
                ty: function_ty,
                span,
            })
        })();

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.current_fn_name = outer_fn_name;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        self.smart_casts = outer_smart_casts;
        lowered
    }

    pub(super) fn lower_anonymous_function(
        &mut self,
        is_suspend: bool,
        source_params: &[ast::Param],
        source_return_ty: Option<&ast::TypeRef>,
        body: &ast::Block,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        });
        if let Some((_, signature)) = &expected_signature {
            if signature.is_suspend != is_suspend {
                self.error(
                    span,
                    "ordinary and suspend function types are incompatible".to_string(),
                );
                return None;
            }
            if source_params.len() != signature.parameter_types.len() {
                self.error(
                    span,
                    format!(
                        "anonymous function has {} parameter(s), but the expected function type has {}",
                        source_params.len(),
                        signature.parameter_types.len()
                    ),
                );
                return None;
            }
        }

        let mut parameter_types = Vec::with_capacity(source_params.len());
        for (index, parameter) in source_params.iter().enumerate() {
            let ty = self.resolve_type_ref(&parameter.ty)?;
            if let Some((_, signature)) = &expected_signature {
                let expected = signature.parameter_types[index];
                if !self.types_equal(ty, expected) {
                    let found = self.type_name(ty);
                    let expected = self.type_name(expected);
                    self.error(
                        parameter.ty.span,
                        format!(
                            "anonymous-function parameter type is {found}, but the expected type is {expected}"
                        ),
                    );
                    return None;
                }
            }
            parameter_types.push(ty);
        }
        let explicit_return = match source_return_ty {
            Some(return_ty) => Some(self.resolve_type_ref(return_ty)?),
            None => None,
        };
        let expected_return = expected_signature
            .as_ref()
            .map(|(_, signature)| signature.return_type);
        if let (Some(explicit), Some(expected)) = (explicit_return, expected_return)
            && !self.types_equal(explicit, expected)
        {
            let found = self.type_name(explicit);
            let expected = self.type_name(expected);
            self.error(
                source_return_ty.expect("explicit return type").span,
                format!(
                    "anonymous-function return type is {found}, but the expected type is {expected}"
                ),
            );
            return None;
        }
        let known_return = explicit_return.or(expected_return);

        let capture_environment = self.capture_environment();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_owner = self.current_owner;
        let outer_this = self.current_this.take();
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        let function_number = self.next_anonymous_function;
        self.next_anonymous_function += 1;
        self.current_fn_name = format!("$anonymous.{function_number}");
        self.current_return_ty = known_return.unwrap_or(self.unit);
        self.return_inference = known_return.is_none().then(ReturnInference::default);
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();

        let lowered = (|| {
            let mut params = Vec::with_capacity(source_params.len());
            for (parameter, ty) in source_params.iter().zip(&parameter_types) {
                if self.scopes.is_declared_here(&parameter.name.text) {
                    self.error(
                        parameter.name.span,
                        format!("duplicate parameter `{}`", parameter.name.text),
                    );
                    return None;
                }
                let local = self.alloc_local(parameter.name.text.clone(), *ty, false);
                self.scopes.declare(parameter.name.text.clone(), local);
                params.push(hir::Param {
                    name: parameter.name.text.clone(),
                    ty: *ty,
                    local,
                });
            }

            let diagnostics_before = self.diagnostics.len();
            let mut statements = self.lower_block(body);
            let return_ty = if let Some(known) = known_return {
                known
            } else {
                let inference = self
                    .return_inference
                    .take()
                    .expect("return inference is active");
                if inference.saw_bare && !inference.value_types.is_empty() {
                    self.error(
                        body.span,
                        "anonymous function mixes bare and value returns".to_string(),
                    );
                    return None;
                }
                if inference.value_types.is_empty() {
                    self.unit
                } else {
                    self.least_upper_bound(&inference.value_types)
                }
            };
            self.current_return_ty = return_ty;
            if self.diagnostics.len() == diagnostics_before
                && !self.types_equal(return_ty, self.unit)
                && statements_can_fall_through(&statements)
            {
                let found = self.type_name(return_ty);
                self.error(
                    body.span,
                    format!(
                        "anonymous function returning {found} may complete without returning a value"
                    ),
                );
                return None;
            }
            if known_return.is_none() {
                statements = self.adapt_inferred_returns(statements, return_ty);
            }
            if self.types_equal(return_ty, self.unit) && statements_can_fall_through(&statements) {
                statements.push(hir::Statement {
                    kind: hir::StatementKind::Return { value: None },
                    span: body.span,
                });
            }

            let function_ty =
                self.intern_function_type(is_suspend, parameter_types.clone(), return_ty);
            let Type::Function(function_type) = self.types[function_ty] else {
                unreachable!()
            };
            let closure_local = self.alloc_local("$closure".to_string(), function_ty, false);
            let mut abi_params = Vec::with_capacity(params.len() + 1);
            abi_params.push(hir::Param {
                name: "$closure".to_string(),
                ty: function_ty,
                local: closure_local,
            });
            abi_params.extend(params);
            let type_params = self.type_params_in_scope.clone();
            let function = self.functions.alloc(hir::Function {
                name: self.current_fn_name.clone(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend,
                params: abi_params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: std::mem::take(&mut self.locals),
                    statements,
                }),
                method: None,
                span,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            let captures = self.finish_current_captures();
            let id = self.anonymous_functions.alloc(hir::AnonymousFunction {
                function,
                function_type,
                owner_type_param_count: type_params.len(),
                captures,
                span,
            });
            Some(hir::Expr {
                kind: ExprKind::AnonymousFunction(id),
                ty: function_ty,
                span,
            })
        })();

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.return_inference = outer_return_inference;
        self.current_fn_name = outer_fn_name;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        self.smart_casts = outer_smart_casts;
        lowered
    }
}
