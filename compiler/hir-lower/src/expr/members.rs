use super::*;

impl Lowerer {
    /// `this` (M6): only inside member functions, where it is
    /// parameter 0 (`lower_body` registers it as a local).
    pub(super) fn lower_this(&mut self, span: Span) -> Option<hir::Expr> {
        let Some(this) = self.lower_current_this(span) else {
            self.error(
                span,
                "`this` is only allowed inside member functions".to_string(),
            );
            return None;
        };
        Some(this)
    }

    /// `receiver.name(args)` (M6/M7): the method overloads are
    /// collected from the receiver's static type — class members (base
    /// chain included), interface methods, or struct / enum methods —
    /// and resolved by the M7 overload algorithm (`resolve_overload`;
    /// an explicit-receiver call has only this member layer). A single
    /// candidate keeps the pre-M7 path so its diagnostics stay intact.
    /// The dispatch kind (direct / virtual / interface) is decided at
    /// MIR from the receiver's static type (hir docs).
    ///
    /// One receiver shape is not a method call: the M6 parser folds a
    /// qualified enum variant construction `E.V(args)` into this
    /// syntax (`MethodCall { receiver: Var("E"), ... }`). When the
    /// receiver is a bare name that is no in-scope variable and no
    /// property of the current host — but names an enum — it is a
    /// variant path and goes through variant construction (M4 rules:
    /// variant existence, per-field argument checks, type-argument
    /// inference, constructor-style defaults). Variables and host
    /// properties shadow enum names. The two compiler-built-in array
    /// conversion methods are recognized after lowering the receiver
    /// and produce the same `ArrayClone` node as their constructor
    /// forms (spec 10.4).
    pub(super) fn lower_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let ast::Expr::Var(enum_name) = receiver {
            if self.scopes.lookup(&enum_name.text).is_none()
                && !self.host_has_property(&enum_name.text)
                && self.enums_by_name.contains_key(&enum_name.text)
            {
                let enum_id = self.enums_by_name[&enum_name.text];
                let Some(variant) = self.find_variant(enum_id, &name.text) else {
                    self.error(
                        name.span,
                        format!("enum `{}` has no variant `{}`", enum_name.text, name.text),
                    );
                    return None;
                };
                return self.lower_variant_construct(enum_id, variant, call, sink, expected);
            }
        }
        let receiver = self.lower_expr(receiver, sink, None)?;
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            if !call.type_args.is_empty() {
                self.error(
                    name.span,
                    "function values do not accept explicit type arguments".to_string(),
                );
                return None;
            }
            return self.lower_callable_call(receiver, call.args, call.span, sink);
        }
        let array_conversion = match (self.array_type_info(receiver.ty), name.text.as_str()) {
            (
                Some(ArrayType {
                    kind: ArrayKind::Mutable,
                    element,
                }),
                "toArray",
            ) => Some((ArrayKind::Immutable, element)),
            (
                Some(ArrayType {
                    kind: ArrayKind::Immutable,
                    element,
                }),
                "toMutableArray",
            ) => Some((ArrayKind::Mutable, element)),
            _ => None,
        };
        if let Some((target_kind, element)) = array_conversion {
            if !call.type_args.is_empty() {
                self.error(name.span, format!("method `{}` is not generic", name.text));
                return None;
            }
            return self.lower_array_method_conversion(
                receiver,
                name,
                call.args,
                call.span,
                target_kind,
                element,
            );
        }
        let mut candidates = self.methods_by_name(receiver.ty, &name.text);
        if candidates.is_empty() {
            let extensions = self.extension_candidate_layer(&name.text);
            if extensions.is_empty() {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!("type `{found}` has no method `{}`", name.text),
                );
                return None;
            }
            return self.finish_extension_call(&extensions, &name.text, receiver, call, sink);
        }
        if matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = candidates.len();
            candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                return None;
            }
        }
        if candidates.len() == 1 {
            return self.finish_method_call(candidates.remove(0), receiver, call, sink);
        }
        self.finish_overloaded_method_call(candidates, &name.text, receiver, call, sink)
    }

    pub(super) fn finish_extension_call(
        &mut self,
        candidates: &[hir::FunctionId],
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        let resolved = self.resolve_extension_overload(
            name,
            candidates,
            receiver,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
            },
            sink,
        )?;
        self.check_call_effects(resolved.callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::Call {
                callee: resolved.callee,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span: call.span,
        })
    }

    /// `m.toArray()` / `a.toMutableArray()` (spec 10.4). The receiver and
    /// result use exact intrinsic class applications; only the clone operation
    /// itself remains compiler-lowered.
    pub(super) fn lower_array_method_conversion(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        args: &[ast::Expr],
        span: Span,
        target_kind: ArrayKind,
        element: TypeId,
    ) -> Option<hir::Expr> {
        if !args.is_empty() {
            self.error(
                span,
                format!(
                    "method `{}` takes exactly 0 arguments, but {} were supplied",
                    name.text,
                    args.len()
                ),
            );
            return None;
        }
        let ty = self.array_type(target_kind, element);
        Some(hir::Expr {
            kind: ExprKind::ArrayClone(Box::new(receiver)),
            ty,
            span,
        })
    }

    /// The multi-candidate path of a method call (explicit receiver or
    /// bare `m(...)`): `resolve_overload` picks the winner among the
    /// receiver type's methods and the call becomes a resolved
    /// `MethodCall`.
    pub(super) fn finish_overloaded_method_call(
        &mut self,
        candidates: Vec<crate::CallableCandidate>,
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        let resolved = self.resolve_member_overload(
            name,
            &candidates,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
            },
            sink,
        )?;
        let ty = resolved.return_ty;
        self.check_call_effects(resolved.callee, call.span);
        if let Some(expr) = self.normalize_pointer_method_call(
            resolved.callee,
            receiver.clone(),
            resolved.args.clone(),
            ty,
            call.span,
        ) {
            return Some(expr);
        }
        let method_callee =
            self.materialize_method_callee(resolved.source, resolved.callee, &resolved.type_args);
        Some(hir::Expr {
            kind: ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: method_callee,
                args: resolved.args,
            },
            ty,
            span: call.span,
        })
    }

    /// Whether the current host type has a property named `name`
    /// (the quiet probe behind the enum-path fallback in
    /// `lower_method_call`: a bare receiver name that would resolve
    /// to `this.name` is a property access, not an enum path).
    pub(super) fn host_has_property(&self, name: &str) -> bool {
        match self.current_this_ty().map(|ty| self.types[ty].clone()) {
            Some(Type::Class(application)) => self
                .find_class_field(self.class_applications[application].template, name)
                .is_some(),
            Some(Type::Struct(application)) => self.structs
                [self.struct_applications[application].template]
                .semantic_fields()
                .iter()
                .any(|field| field.name == name),
            _ => false,
        }
    }

    /// Check and build a resolved method call: arity and argument
    /// types against the method's declared parameters. The receiver binds
    /// the owner prefix and the complete argument group infers the method
    /// suffix; subtype adaptation (boxing) happens afterwards.
    pub(super) fn finish_method_call(
        &mut self,
        candidate: crate::CallableCandidate,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = candidate.function;
        let name = self.functions[function].name.clone();
        let owner_type_args = self.callable_candidate_owner_arguments(&candidate);
        let sig = self.signatures[&function].clone();
        if sig.params.len() != call.args.len() {
            let expected = sig.params.len();
            let supplied = call.args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                call.span,
                format!(
                    "method `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        debug_assert_eq!(sig.owner_type_param_count, owner_type_args.len());
        let mut bindings = vec![None; sig.type_params.len()];
        for (binding, &ty) in bindings.iter_mut().zip(&owner_type_args) {
            *binding = Some(ty);
        }
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        if !self.bind_explicit_type_args(
            &mut bindings,
            sig.owner_type_param_count,
            &explicit_type_args,
            call.span,
            &format!("method `{name}`"),
        ) {
            return None;
        }
        let param_tys: Vec<_> = sig.params.iter().map(|param| param.ty).collect();
        let inferred =
            self.lower_inference_args(call.args, &param_tys, bindings, &sig.type_params)?;
        let mut type_args = Vec::with_capacity(sig.type_params.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&sig.type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        call.span,
                        format!("cannot infer type argument `{}` for `{name}`", param.name),
                    );
                    return None;
                }
            }
        }
        if !self.check_type_argument_kinds(
            &sig.type_params,
            &type_args,
            call.span,
            &format!("function `{}`", self.functions[function].name),
        ) {
            return None;
        }
        let lowered = inferred.finish(sink);
        let mut adapted = Vec::with_capacity(lowered.len());
        for (param, arg) in sig.params.iter().zip(lowered) {
            let param_ty = self.instantiate_ty(param.ty, &type_args);
            if !self.is_subtype(arg.ty, param_ty) {
                let param_name = param.name.text.clone();
                let expected = self.type_name(param_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for parameter `{param_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, param_ty));
        }
        let callee = self.materialize_candidate_callable(&candidate, &type_args);
        let ty = self.instantiate_ty(sig.return_ty, &type_args);
        self.check_call_effects(callee, call.span);
        if let Some(expr) = self.normalize_pointer_method_call(
            callee,
            receiver.clone(),
            adapted.clone(),
            ty,
            call.span,
        ) {
            return Some(expr);
        }
        let method_callee = self.materialize_method_callee(candidate.source, callee, &type_args);
        Some(hir::Expr {
            kind: ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: method_callee,
                args: adapted,
            },
            ty,
            span: call.span,
        })
    }

    pub(crate) fn materialize_method_callee(
        &mut self,
        source: crate::CallableCandidateSource,
        callable: hir::Callable,
        type_args: &[TypeId],
    ) -> hir::MethodCallee {
        let crate::CallableCandidateSource::Bound {
            receiver_parameter,
            bound,
            member,
        } = source
        else {
            return hir::MethodCallee::Callable(callable);
        };
        let function = self.interface_method_entities[member].function;
        let signature = self.signatures[&function].clone();
        let parameter_types = signature
            .params
            .iter()
            .map(|parameter| self.instantiate_ty(parameter.ty, type_args))
            .collect();
        let return_type = self.instantiate_ty(signature.return_ty, type_args);
        let signature_type =
            self.intern_function_type(signature.is_suspend, parameter_types, return_type);
        let Type::Function(instantiated_signature) = self.types[signature_type] else {
            unreachable!("interned function signatures have function type identity")
        };
        let value = hir::BoundCallableRef {
            receiver_parameter,
            bound,
            member,
            instantiated_signature,
        };
        let existing = self
            .bound_callable_refs
            .iter()
            .find_map(|(id, existing)| (existing == &value).then_some(id));
        let id = match existing {
            Some(id) => id,
            None => self.bound_callable_refs.alloc(value),
        };
        hir::MethodCallee::Bound(id)
    }

    pub(super) fn normalize_pointer_method_call(
        &self,
        callee: hir::Callable,
        receiver: hir::Expr,
        args: Vec<hir::Expr>,
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let core = self.ffi_core?;
        let function = self.callable_function_id(callee);
        let kind = if function == core.ptr_to_uint {
            hir::PointerIntrinsic::ToUInt
        } else if function == core.ptr_cast {
            hir::PointerIntrinsic::Cast
        } else if function == core.ptr_load {
            hir::PointerIntrinsic::Load
        } else if function == core.ptr_load_offset {
            hir::PointerIntrinsic::LoadOffset
        } else if function == core.ptr_store {
            hir::PointerIntrinsic::Store
        } else if function == core.ptr_store_offset {
            hir::PointerIntrinsic::StoreOffset
        } else if function == core.ptr_plus {
            hir::PointerIntrinsic::Plus
        } else if function == core.ptr_minus {
            hir::PointerIntrinsic::Minus
        } else {
            return None;
        };
        let mut args = args.into_iter();
        let pointer = Box::new(receiver);
        let expr = match kind {
            hir::PointerIntrinsic::ToUInt => ExprKind::PtrToUInt(pointer),
            hir::PointerIntrinsic::Cast => ExprKind::PtrCast(pointer),
            hir::PointerIntrinsic::Load => ExprKind::PtrLoad {
                pointer,
                offset: None,
            },
            hir::PointerIntrinsic::LoadOffset => ExprKind::PtrLoad {
                pointer,
                offset: Some(Box::new(args.next().expect("validated offset argument"))),
            },
            hir::PointerIntrinsic::Store => ExprKind::PtrStore {
                pointer,
                offset: None,
                value: Box::new(args.next().expect("validated store value")),
            },
            hir::PointerIntrinsic::StoreOffset => ExprKind::PtrStore {
                pointer,
                offset: Some(Box::new(args.next().expect("validated offset argument"))),
                value: Box::new(args.next().expect("validated store value")),
            },
            hir::PointerIntrinsic::Plus | hir::PointerIntrinsic::Minus => ExprKind::PtrOffset {
                pointer,
                offset: Box::new(args.next().expect("validated pointer offset")),
                subtract: kind == hir::PointerIntrinsic::Minus,
            },
            _ => unreachable!("top-level pointer intrinsic is not a method"),
        };
        Some(hir::Expr {
            kind: expr,
            ty,
            span,
        })
    }
}
