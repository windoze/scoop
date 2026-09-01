//! Expression lowering and type checking (milestone3 DESIGN.md 2.2,
//! milestone4 DESIGN.md 3.2).
//!
//! Every expression that survives this stage carries its type
//! (`hir::Expr::ty`); calls resolve to a `FunctionId` (plus inferred
//! type arguments for generic callees), struct constructions to a
//! `StructId`, variant constructions to a `(EnumId, variant index)`
//! pair, field accesses to a `FieldRef`.
//!
//! Two cross-cutting mechanisms:
//!
//! **Desugaring sink.** `?.` and `?:` are expressions in the source
//! but lower to statement-level control flow: HIR has no if-expression
//! and `hir::StatementKind::ValDecl` always has an initializer, so the
//! desugaring stores the receiver in a hidden `$opt.N` local (evaluated
//! exactly once) and initializes a hidden `$res.N` result local *once
//! per branch* of an `if`/`else` — this keeps the else operand lazily
//! evaluated without needing an uninitialized declaration. An
//! expression whose lowering needs such statements pushes them into
//! `sink`, and the expression itself becomes a reference to `$res.N`.
//! The caller (a statement lowering) drains `sink` into the enclosing
//! statement list right before the statement that owns the expression,
//! so sink statements execute exactly where the owning statement does.
//! (`while` conditions are the one place where this would change
//! semantics — they re-evaluate per iteration — and are rejected by
//! the while-statement lowering; `when` guards reject them too.)
//!
//! **Expected-type hint.** `lower_expr` receives the type the context
//! expects, when known: `val` annotations, assignment targets, function
//! argument positions, the other side of `==` / `!=`, the right side of
//! `?:`, and the function return type at `return`. The consumers are
//! the variant constructors whose type arguments cannot be inferred
//! from arguments: a unit variant (`None`, `Color.Red` on a generic
//! enum) takes its type arguments from the hint, and `Some(x)` seeds
//! its inference from an expected `Option<T>` (this is what types
//! `Some(None)` under an `Int??` annotation). M5 adds a second
//! consumer: array literals take their kind (`Array` vs `MutableArray`)
//! and element type from an expected array type. Every other expression
//! ignores the hint and mismatches are reported by the context's own
//! type check.
//!
//! M9 (milestone9 DESIGN.md section 1): arithmetic and comparison
//! operators accept `UInt` operands under the same rules as `Int`
//! (both sides must share one type — no `Int`/`UInt` mixing). M12
//! generalizes the former GC-specific generic check into typed
//! `value` / `ref` kind bounds on every generic declaration.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{ExprKind, Type, TypeId};

use crate::patterns::PatternCtx;
use crate::scope::Scopes;
use crate::stmt::statements_can_fall_through;
use crate::types::{ArrayKind, ArrayType};
use crate::{
    AvailableCapture, CaptureContext, CaptureSource, ForbiddenSuspendContext, Lowerer,
    PendingCapture, ReturnInference, SuspensionContext,
};

struct InferredArguments {
    args: Vec<Option<hir::Expr>>,
    bindings: Vec<Option<TypeId>>,
    sinks: Vec<Vec<hir::Statement>>,
}

#[derive(Clone, Copy)]
struct CallSite<'a> {
    type_args: &'a [ast::TypeRef],
    args: &'a [ast::Expr],
    span: Span,
}

#[derive(Clone, Copy)]
enum ReferenceExtensionMode {
    Exclude,
    IncludeUnbound,
    Bound(TypeId),
}

struct ResolvedReference {
    callable: hir::Callable,
    source: crate::CallableCandidateSource,
    type_args: Vec<TypeId>,
    ty: TypeId,
}

impl InferredArguments {
    fn finish(self, sink: &mut Vec<hir::Statement>) -> Vec<hir::Expr> {
        let mut args = Vec::with_capacity(self.args.len());
        for (arg, mut arg_sink) in self.args.into_iter().zip(self.sinks) {
            sink.append(&mut arg_sink);
            args.push(arg.expect("complete type bindings type every deferred argument"));
        }
        args
    }
}

impl Lowerer {
    /// Snapshot the bindings visible at a nested callable creation point.
    /// Bindings inherited from the enclosing callable are represented as
    /// transitive capture reads; locals declared by that callable override
    /// them according to ordinary lexical shadowing.
    pub(crate) fn capture_environment(
        &self,
    ) -> std::collections::HashMap<String, AvailableCapture> {
        let mut available: std::collections::HashMap<String, AvailableCapture> = self
            .capture_contexts
            .last()
            .map(|context| {
                context
                    .available
                    .iter()
                    .map(|(name, capture)| {
                        let mut capture = capture.clone();
                        capture.source = CaptureSource::Capture(capture.binding);
                        (name.clone(), capture)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let declaration_depth = self.capture_contexts.len();
        for (name, local) in self.scopes.visible() {
            let local_def = &self.locals[local];
            available.insert(
                name,
                AvailableCapture {
                    binding: local_def.binding,
                    ty: local_def.ty,
                    mutable: local_def.mutable,
                    source: CaptureSource::Local(local),
                    declaration_depth,
                },
            );
        }
        available
    }

    pub(crate) fn register_capture_at(
        &mut self,
        context_index: usize,
        name: &str,
        available: AvailableCapture,
        first_use_span: Span,
    ) {
        if matches!(available.source, CaptureSource::Capture(_)) {
            assert!(
                context_index > 0,
                "a transitive capture always has an enclosing closure"
            );
            let parent_index = context_index - 1;
            let parent_available = self.capture_contexts[parent_index]
                .available
                .values()
                .find(|candidate| candidate.binding == available.binding)
                .cloned()
                .expect("the enclosing closure can provide a transitive capture");
            self.register_capture_at(parent_index, name, parent_available, first_use_span);
        }
        let context = &mut self.capture_contexts[context_index];
        if context.by_binding.contains_key(&available.binding) {
            return;
        }
        let index = context.captures.len();
        context.by_binding.insert(available.binding, index);
        context.captures.push(PendingCapture {
            binding: available.binding,
            name: name.to_string(),
            ty: available.ty,
            first_use_span,
            source: available.source,
            declaration_depth: available.declaration_depth,
        });
    }

    fn lower_capture(&mut self, name: &ast::Ident) -> Option<hir::Expr> {
        let context_index = self.capture_contexts.len().checked_sub(1)?;
        let available = self.capture_contexts[context_index]
            .available
            .get(&name.text)?
            .clone();
        if available.mutable {
            self.error(
                name.span,
                format!(
                    "cannot capture mutable local `{}`; bind its current value to a `val` snapshot or capture explicit reference state",
                    name.text
                ),
            );
            return None;
        }
        self.register_capture_at(context_index, &name.text, available.clone(), name.span);
        Some(hir::Expr {
            kind: ExprKind::Capture(available.binding),
            ty: available.ty,
            span: name.span,
        })
    }

    fn lower_capture_binding(
        &mut self,
        binding: hir::BindingId,
        fallback_name: &str,
        span: Span,
    ) -> Option<hir::Expr> {
        let context_index = self.capture_contexts.len().checked_sub(1)?;
        let available = self.capture_contexts[context_index]
            .available
            .iter()
            .find_map(|(name, candidate)| {
                (candidate.binding == binding).then(|| (name.clone(), candidate.clone()))
            })?;
        let (name, available) = available;
        if available.mutable {
            self.error(
                span,
                format!(
                    "cannot capture mutable local `{fallback_name}`; bind its current value to a `val` snapshot or capture explicit reference state"
                ),
            );
            return None;
        }
        self.register_capture_at(context_index, &name, available.clone(), span);
        Some(hir::Expr {
            kind: ExprKind::Capture(available.binding),
            ty: available.ty,
            span,
        })
    }

    pub(crate) fn available_capture(&self, name: &str) -> Option<AvailableCapture> {
        self.capture_contexts
            .last()
            .and_then(|context| context.available.get(name))
            .cloned()
    }

    pub(crate) fn finish_current_captures(&mut self) -> Vec<hir::Capture> {
        let context = self
            .capture_contexts
            .last_mut()
            .expect("a lambda capture context is active");
        context.captures.sort_by_key(|capture| {
            (
                capture.declaration_depth,
                capture.first_use_span.start,
                capture.binding,
            )
        });
        context
            .captures
            .drain(..)
            .map(|capture| {
                let source = match capture.source {
                    CaptureSource::Local(local) => hir::Expr {
                        kind: ExprKind::Local(local),
                        ty: capture.ty,
                        span: capture.first_use_span,
                    },
                    CaptureSource::Capture(binding) => hir::Expr {
                        kind: ExprKind::Capture(binding),
                        ty: capture.ty,
                        span: capture.first_use_span,
                    },
                };
                hir::Capture {
                    binding: capture.binding,
                    name: capture.name,
                    ty: capture.ty,
                    first_use_span: capture.first_use_span,
                    source,
                }
            })
            .collect()
    }

    pub(crate) fn lower_current_this(&mut self, span: Span) -> Option<hir::Expr> {
        if let Some((local, ty)) = self.current_this {
            return Some(hir::Expr {
                kind: ExprKind::Local(local),
                ty,
                span,
            });
        }
        if self.available_capture("this").is_some() {
            return self.lower_capture(&ast::Ident {
                text: "this".to_string(),
                span,
            });
        }
        None
    }

    pub(crate) fn current_this_ty(&self) -> Option<TypeId> {
        self.current_this
            .map(|(_, ty)| ty)
            .or_else(|| self.available_capture("this").map(|capture| capture.ty))
    }

    /// Lower an expression, recording a diagnostic and returning `None`
    /// on error. See the module docs for the `sink` / `expected`
    /// mechanisms.
    pub(crate) fn lower_expr(
        &mut self,
        expr: &ast::Expr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let lowered = match expr {
            ast::Expr::StringLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::StringLiteral(value.clone()),
                ty: self.string,
                span: *span,
            }),
            ast::Expr::IntLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::IntLiteral(*value),
                ty: self.int,
                span: *span,
            }),
            ast::Expr::BoolLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::BoolLiteral(*value),
                ty: self.boolean,
                span: *span,
            }),
            ast::Expr::UnitLiteral { span } => Some(hir::Expr {
                kind: ExprKind::UnitLiteral,
                ty: self.unit,
                span: *span,
            }),
            ast::Expr::TupleLiteral { elements, span } => {
                self.lower_tuple_literal(elements, *span, sink, expected)
            }
            // `Name(args...)` where the parser already knows `Name` is
            // a type (struct or enum variant path).
            ast::Expr::StructInit { name, args, span } => match self.classify_constructor(name)? {
                Constructor::Struct { struct_id, ty } => {
                    let call = CallSite {
                        type_args: &[],
                        args,
                        span: *span,
                    };
                    if Some(struct_id) == self.ffi_ptr || Some(struct_id) == self.ffi_fun_ptr {
                        self.lower_ffi_struct_init(struct_id, call, sink, expected)
                    } else {
                        self.lower_struct_init(struct_id, ty, call, sink, expected)
                    }
                }
                Constructor::Variant { enum_id, variant } => self.lower_variant_construct(
                    enum_id,
                    variant,
                    CallSite {
                        type_args: &[],
                        args,
                        span: *span,
                    },
                    sink,
                    expected,
                ),
                Constructor::Class { class_id } => self.lower_class_construct(
                    class_id,
                    CallSite {
                        type_args: &[],
                        args,
                        span: *span,
                    },
                    sink,
                    expected,
                ),
                Constructor::Unmatched => {
                    self.error(name.span, format!("unknown struct `{}`", name.text));
                    None
                }
            },
            ast::Expr::Var(name) => self.lower_var(name, expected),
            ast::Expr::Lambda {
                is_suspend,
                parameters,
                body,
                span,
                ..
            } => self.lower_lambda(*is_suspend, parameters.as_deref(), body, *span, expected),
            ast::Expr::AnonymousFunction {
                is_suspend,
                params,
                return_ty,
                body,
                span,
                ..
            } => self.lower_anonymous_function(
                *is_suspend,
                params,
                return_ty.as_ref(),
                body,
                *span,
                expected,
            ),
            ast::Expr::CallableReference {
                receiver,
                name,
                span,
                ..
            } => self.lower_callable_reference(receiver.as_deref(), name, *span, expected, sink),
            ast::Expr::FieldAccess(access) if access.safe => {
                self.lower_safe_field_access(access, sink)
            }
            ast::Expr::FieldAccess(access) => self.lower_field_access(access, sink, expected),
            ast::Expr::Call(call) => self.lower_call(call, sink, expected),
            ast::Expr::Invoke { callee, args, span } => {
                let callee = self.lower_expr(callee, sink, None)?;
                self.lower_callable_call(callee, args, *span, sink)
            }
            ast::Expr::Binary { op, lhs, rhs, span } => {
                self.lower_binary(*op, lhs, rhs, *span, sink)
            }
            ast::Expr::Unary { op, operand, span } => self.lower_unary(*op, operand, *span, sink),
            ast::Expr::NullAssert { operand, span } => self.lower_null_assert(operand, *span, sink),
            ast::Expr::Elvis { lhs, rhs, span } => self.lower_elvis(lhs, rhs, *span, sink),
            ast::Expr::This { span } => self.lower_this(*span),
            ast::Expr::MethodCall {
                receiver,
                name,
                type_args,
                args,
                span,
            } => self.lower_method_call(
                receiver,
                name,
                CallSite {
                    type_args,
                    args,
                    span: *span,
                },
                sink,
                expected,
            ),
            ast::Expr::Is {
                operand,
                ty,
                negated,
                span,
            } => self.lower_is(operand, ty, *negated, *span, sink),
            ast::Expr::Cast {
                operand,
                ty,
                optional,
                span,
            } => self.lower_cast(operand, ty, *optional, *span, sink),
            ast::Expr::ArrayLiteral { elements, span } => {
                self.lower_array_literal(elements, *span, sink, expected)
            }
            ast::Expr::Index {
                receiver,
                index,
                span,
            } => self.lower_index_read(receiver, index, *span, sink),
            ast::Expr::If(if_) => self.lower_if_expression(if_, sink, expected),
            ast::Expr::When(when) => self.lower_when_expression(when, sink, expected),
            ast::Expr::Try(try_) => self.lower_try_expression(try_, sink, expected),
        };
        if let Some(value) = &lowered
            && !self.require_unsafe_type_use(value.ty, value.span)
        {
            return None;
        }
        lowered
    }

    /// `this` (M6): only inside member functions, where it is
    /// parameter 0 (`lower_body` registers it as a local).
    fn lower_this(&mut self, span: Span) -> Option<hir::Expr> {
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
    fn lower_method_call(
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

    fn finish_extension_call(
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
    fn lower_array_method_conversion(
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
    fn finish_overloaded_method_call(
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
    fn host_has_property(&self, name: &str) -> bool {
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
    fn finish_method_call(
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

    fn materialize_method_callee(
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

    fn normalize_pointer_method_call(
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

    /// `expr is T` / `expr !is T` (M6): the static premise is that the
    /// operand could ever hold a `T` (`could_hold`); a check between
    /// unrelated types is useless and diagnosed.
    fn lower_is(
        &mut self,
        operand: &ast::Expr,
        ty_ref: &ast::TypeRef,
        negated: bool,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let check_ty = self.resolve_type_ref(ty_ref)?;
        if !self.could_hold(operand.ty, check_ty) {
            let found = self.type_name(operand.ty);
            let check = self.type_name(check_ty);
            self.error(
                span,
                format!("useless type check: `{found}` can never be `{check}`"),
            );
            return None;
        }
        let is_expr = hir::Expr {
            kind: ExprKind::IsInstance {
                operand: Box::new(operand),
                check_ty,
            },
            ty: self.boolean,
            span,
        };
        if negated {
            Some(hir::Expr {
                kind: ExprKind::Unary {
                    op: hir::UnOp::Not,
                    operand: Box::new(is_expr),
                },
                ty: self.boolean,
                span,
            })
        } else {
            Some(is_expr)
        }
    }

    /// `expr as T` / `expr as? T` (M6). An upcast is free (`adapt_to`:
    /// boxing for value types, a retype for references — `as?` wraps
    /// the result in `Some`). A downcast lowers to `ExprKind::Cast`;
    /// for a value-type target the non-optional form is followed by
    /// `Unbox` (the check keeps the reference, the unbox extracts the
    /// payload), while `as?` yields `Option<T>` directly (the payload
    /// unboxing is part of the optional-cast semantics — `Cast::ty`
    /// is `Option<T>`, so no separate `Unbox` node can be attached).
    fn lower_cast(
        &mut self,
        operand: &ast::Expr,
        ty_ref: &ast::TypeRef,
        optional: bool,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let target = self.resolve_type_ref(ty_ref)?;
        if !self.could_hold(operand.ty, target) {
            let found = self.type_name(operand.ty);
            let check = self.type_name(target);
            self.error(
                span,
                format!("cast from `{found}` to `{check}` can never succeed"),
            );
            return None;
        }
        if optional && self.option_enum.is_none() {
            // The missing core `Option` was already diagnosed.
            return None;
        }
        if self.is_subtype(operand.ty, target) {
            let adapted = self.adapt_to(operand, target);
            if optional {
                let ty = self.option_type(target);
                return Some(hir::Expr {
                    kind: ExprKind::SomeWrap(Box::new(adapted)),
                    ty,
                    span,
                });
            }
            return Some(adapted);
        }
        if optional {
            let ty = self.option_type(target);
            return Some(hir::Expr {
                kind: ExprKind::Cast {
                    operand: Box::new(operand),
                    optional: true,
                },
                ty,
                span,
            });
        }
        let cast = hir::Expr {
            kind: ExprKind::Cast {
                operand: Box::new(operand),
                optional: false,
            },
            ty: target,
            span,
        };
        if self.is_value_ty(target) {
            Some(hir::Expr {
                kind: ExprKind::Unbox(Box::new(cast)),
                ty: target,
                span,
            })
        } else {
            Some(cast)
        }
    }

    fn lower_tuple_literal(
        &mut self,
        elements: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        // The parser never produces an empty tuple literal (`()` is a
        // `UnitLiteral`); reject it here so every AST shape is handled.
        if elements.is_empty() {
            self.error(
                span,
                "tuple literal must contain at least one element".to_string(),
            );
            return None;
        }
        let expected_elements = expected.and_then(|ty| match &self.types[ty] {
            Type::Tuple(expected) if expected.len() == elements.len() => Some(expected.clone()),
            _ => None,
        });
        let mut lowered = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            let hint = expected_elements.as_ref().map(|expected| expected[index]);
            lowered.push(self.lower_expr(element, sink, hint)?);
        }
        let ty = self.intern_type(Type::Tuple(
            lowered.iter().map(|element| element.ty).collect(),
        ));
        Some(hir::Expr {
            kind: ExprKind::TupleLiteral(lowered),
            ty,
            span,
        })
    }

    /// `[e1, ...]` (spec 10.2/10.3, milestone5 DESIGN.md 2.2). With an
    /// expected `Array<U>` / `MutableArray<U>` the literal takes that
    /// kind and every element must be a subtype of `U`, except that a
    /// value element may not cross into a reference type by implicit
    /// boxing (an empty literal is only legal in this case). Without an
    /// array expectation, value elements require exact equality while
    /// an all-reference literal infers their least upper bound. (An
    /// expected type of any other shape is ignored: the context's own
    /// check reports the mismatch.)
    fn lower_array_literal(
        &mut self,
        elements: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let expected_array =
            expected.and_then(|ty| self.array_type_info(ty).map(|array| (ty, array.element)));
        if let Some((array_ty, element_ty)) = expected_array {
            let mut lowered = Vec::with_capacity(elements.len());
            for element in elements {
                let element = self.lower_expr(element, sink, Some(element_ty))?;
                let would_auto_box = self.is_value_ty(element.ty)
                    && self.is_ref_ty(element_ty)
                    && !self.types_equal(element.ty, element_ty);
                if !self.is_subtype(element.ty, element_ty) || would_auto_box {
                    let expected = self.type_name(element_ty);
                    let found = self.type_name(element.ty);
                    self.error(
                        element.span,
                        format!("array literal element must be of type {expected}, found {found}"),
                    );
                    return None;
                }
                lowered.push(self.adapt_to(element, element_ty));
            }
            return Some(hir::Expr {
                kind: ExprKind::ArrayLiteral(lowered),
                ty: array_ty,
                span,
            });
        }
        if elements.is_empty() {
            self.error(
                span,
                "cannot infer the element type of an empty array literal".to_string(),
            );
            return None;
        }
        let mut lowered = Vec::with_capacity(elements.len());
        for element in elements {
            lowered.push(self.lower_expr(element, sink, None)?);
        }
        let first_ty = lowered[0].ty;
        if lowered.iter().any(|element| self.is_value_ty(element.ty)) {
            for element in &lowered[1..] {
                if self.types_equal(first_ty, element.ty) {
                    continue;
                }
                let first = self.type_name(first_ty);
                let found = self.type_name(element.ty);
                self.error(
                    element.span,
                    format!(
                        "array literal elements must have the same type, found {first} and {found}"
                    ),
                );
                return None;
            }
        }
        let element_ty = if self.is_ref_ty(first_ty) {
            let element_types: Vec<TypeId> = lowered.iter().map(|element| element.ty).collect();
            self.reference_lob(&element_types)
        } else {
            first_ty
        };
        let lowered = lowered
            .into_iter()
            .map(|element| self.adapt_to(element, element_ty))
            .collect();
        let ty = self.array_type(ArrayKind::Immutable, element_ty);
        Some(hir::Expr {
            kind: ExprKind::ArrayLiteral(lowered),
            ty,
            span,
        })
    }

    /// `receiver[index]` (spec 10.5): the receiver must be an
    /// `Array<T>` / `MutableArray<T>` and the index an `Int`; the
    /// result is the element type `T`.
    fn lower_index_read(
        &mut self,
        receiver: &ast::Expr,
        index: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(receiver, sink, None)?;
        let Some(element_ty) = self.array_element_ty(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                receiver.span,
                format!("subscript is only supported on arrays, found {found}"),
            );
            return None;
        };
        let index = self.lower_expr(index, sink, Some(self.int))?;
        if index.ty != self.int {
            let found = self.type_name(index.ty);
            self.error(
                index.span,
                format!("array index must be Int, found {found}"),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::Index {
                receiver: Box::new(receiver),
                index: Box::new(index),
            },
            ty: element_ty,
            span,
        })
    }

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
    fn lower_var(&mut self, name: &ast::Ident, expected: Option<TypeId>) -> Option<hir::Expr> {
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
                };
                if self.is_value_ty(narrowed) {
                    // A boxed value narrowed to its value type unboxes.
                    return Some(hir::Expr {
                        kind: ExprKind::Unbox(Box::new(local_expr)),
                        ty: narrowed,
                        span: name.span,
                    });
                }
                // A reference narrowed to a subtype: zero-cost retype.
                return Some(hir::Expr {
                    kind: ExprKind::Local(local),
                    ty: narrowed,
                    span: name.span,
                });
            }
        }
        Some(hir::Expr {
            kind: ExprKind::Local(local),
            ty: declared,
            span: name.span,
        })
    }

    /// `x` inside a member function when `x` is no local: a property
    /// of the host type (`this.x`; class properties include the base
    /// chain). Methods are not values in M6, so only fields resolve.
    fn bare_member_fallback(&mut self, name: &ast::Ident) -> Option<hir::Expr> {
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

    /// A unit variant construction (`None`, `Color.Red`): the variant
    /// carries no fields, so the enum's type arguments (if any) must
    /// come from the expected-type hint — the M3 `None` inference
    /// rule, generalized.
    fn lower_unit_variant(
        &mut self,
        name: &ast::Ident,
        enum_id: hir::EnumId,
        variant: u32,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let arity = self.enums[enum_id].type_params.len();
        let type_args = if arity == 0 {
            Vec::new()
        } else {
            let inferred = expected.and_then(|ty| match self.types[ty].clone() {
                Type::Enum(application) => {
                    let application = &self.enum_applications[application];
                    (application.template == enum_id && application.arguments.len() == arity)
                        .then(|| application.arguments.clone())
                }
                _ => None,
            });
            match inferred {
                Some(args) => args,
                None => {
                    self.error(
                        name.span,
                        format!("cannot infer the type of `{}`", name.text),
                    );
                    return None;
                }
            }
        };
        let application = self.enum_application_id(enum_id, type_args);
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                application,
                variant,
                args: Vec::new(),
            },
            ty,
            span: name.span,
        })
    }

    /// What a `Name` / `Name(...)` construction site resolves to. A
    /// dotted path `E.V` is always an enum variant; a bare name is a
    /// globally visible `Option` variant (`Some` / `None`), then a
    /// struct, then a class, then — for `Call` nodes only — a
    /// function.
    fn classify_constructor(&mut self, name: &ast::Ident) -> Option<Constructor> {
        if let Some((enum_name, variant_name)) = name.text.split_once('.') {
            let Some(&enum_id) = self.enums_by_name.get(enum_name) else {
                self.error(name.span, format!("unknown enum `{enum_name}`"));
                return None;
            };
            let Some(variant) = self.find_variant(enum_id, variant_name) else {
                self.error(
                    name.span,
                    format!("enum `{enum_name}` has no variant `{variant_name}`"),
                );
                return None;
            };
            return Some(Constructor::Variant { enum_id, variant });
        }
        if let Some((enum_id, variant)) = self.option_variant(&name.text) {
            return Some(Constructor::Variant { enum_id, variant });
        }
        if let Some(&(struct_id, ty)) = self.structs_by_name.get(&name.text) {
            return Some(Constructor::Struct { struct_id, ty });
        }
        if let Some(&(class_id, _)) = self.classes_by_name.get(&name.text) {
            return Some(Constructor::Class { class_id });
        }
        Some(Constructor::Unmatched)
    }

    /// `Name(args...)` where `Name` is a class (M6): object
    /// construction with the class's own constructor properties
    /// (`hir::ExprKind::ClassInit`; base-class delegation is part of
    /// the generated constructor, mir-lower's job). Abstract classes
    /// cannot be instantiated. Argument count and types are checked
    /// against the constructor properties one by one (subtype
    /// adaptation included, mirroring struct construction).
    fn lower_class_construct(
        &mut self,
        class_id: hir::ClassId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let name = self.classes[class_id].name.clone();
        if matches!(
            self.classes[class_id].representation,
            hir::ClassRepresentation::Intrinsic(_)
        ) {
            self.error(
                span,
                format!("intrinsic class `{name}` has no source constructor"),
            );
            return None;
        }
        if self.classes[class_id].modifier == hir::ClassModifier::Abstract {
            self.error(
                span,
                format!("abstract class `{name}` cannot be instantiated"),
            );
            return None;
        }
        let props: Vec<(String, TypeId)> = self.classes[class_id]
            .semantic_constructor()
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        if args.len() != props.len() {
            let expected = props.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "class `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        let type_params = self.classes[class_id].type_params.clone();
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        if type_params.is_empty() && !explicit_type_args.is_empty() {
            self.error(span, format!("class `{name}` is not generic"));
            return None;
        }
        let mut bindings = vec![None; type_params.len()];
        if !self.bind_explicit_type_args(
            &mut bindings,
            0,
            &explicit_type_args,
            span,
            &format!("class `{name}`"),
        ) {
            return None;
        }
        if let Some(expected) = expected
            && let Type::Class(application) = self.types[expected]
            && self.class_applications[application].template == class_id
            && self.class_applications[application].arguments.len() == type_params.len()
        {
            let expected_args = self.class_applications[application].arguments.clone();
            for (binding, argument) in bindings.iter_mut().zip(expected_args) {
                if binding.is_none() {
                    *binding = Some(argument);
                }
            }
        }
        let property_types = props.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
        let inferred = self.lower_inference_args(args, &property_types, bindings, &type_params)?;
        let mut type_args = Vec::with_capacity(type_params.len());
        for (binding, parameter) in inferred.bindings.iter().copied().zip(&type_params) {
            let Some(argument) = binding else {
                self.error(
                    span,
                    format!(
                        "cannot infer type argument `{}` for class `{name}`",
                        parameter.name
                    ),
                );
                return None;
            };
            type_args.push(argument);
        }
        if !self.check_type_argument_kinds(
            &type_params,
            &type_args,
            span,
            &format!("class `{name}`"),
        ) {
            return None;
        }
        let lowered = inferred.finish(sink);
        let mut adapted = Vec::with_capacity(lowered.len());
        for ((prop_name, prop_ty), arg) in props.iter().zip(lowered) {
            let prop_ty = self.instantiate_ty(*prop_ty, &type_args);
            if !self.is_subtype(arg.ty, prop_ty) {
                let expected = self.type_name(prop_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{prop_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, prop_ty));
        }
        let application = self.class_application_id(class_id, type_args);
        let ty = self.class_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::ClassInit {
                application,
                args: adapted,
            },
            ty,
            span,
        })
    }

    /// `Name(args...)` in call position: a variant or struct
    /// construction when the name resolves as one, a direct function
    /// call otherwise.
    fn lower_call(
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

    fn foreign_callback_operation(
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

    fn lower_foreign_callback_registration(
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

    fn lower_foreign_callback_call(
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

    fn lower_callable_call(
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

    fn lower_callable_reference(
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

    fn lower_native_function_reference(
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

    fn lower_bound_callable_reference(
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

    fn is_declared_type_name(&self, name: &str) -> bool {
        self.classes_by_name.contains_key(name)
            || self.interfaces_by_name.contains_key(name)
            || self.structs_by_name.contains_key(name)
            || self.enums_by_name.contains_key(name)
    }

    fn expected_function_signature(
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
    fn resolve_reference_candidates(
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

    fn resolve_member_reference_candidates(
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

    fn resolve_reference_candidate_set(
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

    fn lower_local_callable_reference(
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

    fn top_level_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        self.candidate_layer(self.functions_by_name.get(name))
    }

    fn managed_reference_target_is_safe(&mut self, callee: hir::Callable, span: Span) -> bool {
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

    fn extension_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        self.candidate_layer(self.extensions_by_name.get(name))
    }

    fn candidate_layer(&self, ids: Option<&Vec<hir::FunctionId>>) -> Vec<hir::FunctionId> {
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
    fn named_reference_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
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

    fn lower_lambda(
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

    fn lower_anonymous_function(
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

    /// `Array(m)` / `MutableArray(a)`: the conversion between the two
    /// array kinds (spec 10.4). The argument must be exactly one array
    /// of the *other* kind with the same element type; the result is a
    /// memcpy snapshot (`ArrayClone`). A same-kind argument is
    /// rejected: conversion is never the identity.
    fn lower_array_conversion(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        target_kind: ArrayKind,
    ) -> Option<hir::Expr> {
        let name = call.callee.text.clone();
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if explicit_type_args.len() > 1 {
            self.error(
                call.callee.span,
                format!(
                    "`{name}` takes exactly 1 type argument, but {} were supplied",
                    explicit_type_args.len()
                ),
            );
            return None;
        }
        if call.args.len() != 1 {
            let supplied = call.args.len();
            self.error(
                call.span,
                format!("`{name}` takes exactly 1 argument, but {supplied} were supplied"),
            );
            return None;
        }
        let arg = self.lower_expr(&call.args[0], sink, None)?;
        let element_ty = match (target_kind, self.array_type_info(arg.ty)) {
            (
                ArrayKind::Immutable,
                Some(ArrayType {
                    kind: ArrayKind::Mutable,
                    element,
                }),
            )
            | (
                ArrayKind::Mutable,
                Some(ArrayType {
                    kind: ArrayKind::Immutable,
                    element,
                }),
            ) => element,
            (_, Some(_)) => {
                self.error(
                    arg.span,
                    "use the value directly; conversion is only between Array and MutableArray"
                        .to_string(),
                );
                return None;
            }
            _ => {
                let expected = if target_kind == ArrayKind::Immutable {
                    "a MutableArray"
                } else {
                    "an Array"
                };
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!("argument of `{name}` conversion must be {expected}, found {found}"),
                );
                return None;
            }
        };
        if let Some(&explicit) = explicit_type_args.first()
            && !self.types_equal(explicit, element_ty)
        {
            let expected = self.type_name(explicit);
            let found = self.type_name(element_ty);
            self.error(
                call.callee.span,
                format!("explicit element type is {expected}, but the argument contains {found}"),
            );
            return None;
        }
        let ty = self.array_type(target_kind, element_ty);
        Some(hir::Expr {
            kind: ExprKind::ArrayClone(Box::new(arg)),
            ty,
            span: call.span,
        })
    }

    /// Variant construction (`Some(x)`, `Shape.Circle(1)`,
    /// `E.WithDefault(1)` with a trailing default filled in). The
    /// variant behaves like a generic constructor function: type
    /// arguments are seeded from an expected `E<...>` hint and then
    /// inferred from the arguments (the same binding mechanism as
    /// generic calls), and each argument is checked against the
    /// instantiated field type.
    fn lower_variant_construct(
        &mut self,
        enum_id: hir::EnumId,
        variant: u32,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let enum_name = self.enums[enum_id].name.clone();
        let type_params = self.enums[enum_id].type_params.clone();
        let variant_name = self.enums[enum_id].variants[variant as usize].name.clone();
        let fields: Vec<(String, TypeId)> = self.enums[enum_id].variants[variant as usize]
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        let total = fields.len();
        let supplied = args.len();
        if supplied > total {
            self.error(
                span,
                format!(
                    "variant `{variant_name}` of `{enum_name}` takes exactly {total} {}, but {supplied} were supplied",
                    if total == 1 { "argument" } else { "arguments" }
                ),
            );
            return None;
        }
        // Missing trailing fields must have constructor-style defaults.
        for index in supplied..total {
            if self.enums[enum_id].variants[variant as usize].defaults[index].is_none() {
                self.error(
                    span,
                    format!(
                        "variant `{variant_name}` of `{enum_name}` takes exactly {total} {}, but {supplied} were supplied",
                        if total == 1 { "argument" } else { "arguments" }
                    ),
                );
                return None;
            }
        }

        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let mut bindings = vec![None; type_params.len()];
        if !self.bind_explicit_type_args(
            &mut bindings,
            0,
            &explicit_type_args,
            span,
            &format!("enum `{enum_name}`"),
        ) {
            return None;
        }
        if let Some(expected) = expected {
            if let Type::Enum(application) = self.types[expected] {
                let application = self.enum_applications[application].clone();
                if application.template == enum_id
                    && application.arguments.len() == type_params.len()
                {
                    for (binding, arg) in bindings.iter_mut().zip(application.arguments) {
                        if binding.is_none() {
                            *binding = Some(arg);
                        }
                    }
                }
            }
        }
        let field_tys: Vec<TypeId> = fields.iter().map(|(_, ty)| *ty).collect();
        let inferred =
            self.lower_inference_args(args, &field_tys[..supplied], bindings, &type_params)?;

        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        span,
                        format!(
                            "cannot infer type argument `{}` for `{enum_name}.{variant_name}`",
                            param.name
                        ),
                    );
                    return None;
                }
            }
        }
        if !self.check_type_argument_kinds(
            &type_params,
            &type_args,
            span,
            &format!("enum `{enum_name}`"),
        ) {
            return None;
        }
        let mut lowered = inferred.finish(sink);

        // Argument types must match the instantiated field types.
        for ((field_name, field_ty), arg) in fields.iter().zip(&lowered) {
            let expected = self.instantiate_ty(*field_ty, &type_args);
            if !self.types_equal(expected, arg.ty) {
                let expected_name = self.type_name(expected);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{field_name}` of `{enum_name}.{variant_name}` must be of type {expected_name}, found {found}"
                    ),
                );
                return None;
            }
        }

        // Fill the trailing defaults (already lowered and type-checked
        // at the declaration site).
        for index in supplied..total {
            let default = self.enums[enum_id].variants[variant as usize].defaults[index]
                .as_ref()
                .expect("missing defaults were rejected above");
            lowered.push(clone_literal(default));
        }

        let application = self.enum_application_id(enum_id, type_args);
        let ty = self.enum_applications[application].canonical_type;
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                application,
                variant,
                args: lowered,
            },
            ty,
            span,
        })
    }

    /// A bare call `f(args)` (M7): the candidate layers are, in order,
    /// the current host's methods (inside a member function, where
    /// `f(...)` means `this.f(...)`), the top-level functions declared
    /// on the call site's own side of the core/user boundary (its
    /// "same package" layer), and the other side (the implicitly
    /// imported layer) — the first layer containing any candidate wins
    /// whole (milestone7 DESIGN.md 1.2). The layering is relative to
    /// the call site's file: for a user-file call that is user
    /// top-level → core, for a core-file call core → user, so a user
    /// declaration shadows core overloads for user code without
    /// breaking the core library's own internal calls. (M13's
    /// multi-Cone package system will redefine these layers per
    /// package/Cone.) A single candidate keeps the pre-M7 path
    /// (`finish_single_function_call` / `finish_method_call`) so its
    /// diagnostics stay intact; several candidates go through
    /// `resolve_overload`.
    fn lower_function_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let name = call.callee.text.clone();

        // Layer 1: the nearest lexical block containing local functions of
        // this name. The whole overload set shadows members and top-level
        // functions, and declarations enter it only as they are encountered.
        let local_candidates = self.local_function_scopes.lookup(&name);
        if !local_candidates.is_empty() {
            if local_candidates.len() == 1 {
                return self.finish_local_function_call(local_candidates[0], call, sink);
            }
            let functions: Vec<_> = local_candidates
                .iter()
                .map(|id| self.local_functions[*id].function)
                .collect();
            let owner_count = self.local_functions[local_candidates[0]].owner_type_param_count;
            let owner_type_args = self.ambient_type_args(owner_count);
            let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
            let resolved = self.resolve_overload(
                &name,
                &functions,
                &owner_type_args,
                crate::overload::OverloadCall {
                    explicit_type_args: &explicit_type_args,
                    arg_exprs: &call.args,
                    span: call.span,
                },
                sink,
            )?;
            let function = self.callable_function_id(resolved.callee);
            let local_function = self.local_function_by_function[&function];
            let captures = self.local_call_capture_args(local_function, call.span)?;
            self.check_call_effects(resolved.callee, call.span);
            return Some(hir::Expr {
                kind: ExprKind::LocalFunctionCall {
                    local_function,
                    callee: resolved.callee,
                    captures,
                    args: resolved.args,
                },
                ty: resolved.return_ty,
                span: call.span,
            });
        }

        // Layer 2: members of the current host.
        let mut members = self
            .current_this_ty()
            .map(|host_ty| self.methods_by_name(host_ty, &name))
            .unwrap_or_default();
        if !members.is_empty() {
            let receiver = self
                .lower_current_this(call.callee.span)
                .expect("a member callable body always has a lexical `this`");
            if members.len() == 1 {
                return self.finish_method_call(
                    members.remove(0),
                    receiver,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                );
            }
            return self.finish_overloaded_method_call(
                members,
                &name,
                receiver,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
            );
        }

        // An extension body has a lexical `this` just like a member body.
        // If no real member wins, another visible extension may use it as
        // the implicit receiver before ordinary top-level functions.
        if self.current_this_ty().is_some() {
            let extensions = self.extension_candidate_layer(&name);
            if !extensions.is_empty() {
                let receiver = self
                    .lower_current_this(call.callee.span)
                    .expect("a lexical receiver has a `this` value");
                return self.finish_extension_call(
                    &extensions,
                    &name,
                    receiver,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                );
            }
        }

        // Layers 2 and 3, relative to the call site's file: the
        // declarations on the call site's own side of the core/user
        // boundary come first, the other side is the implicitly
        // imported layer.
        let candidates = self.top_level_candidate_layer(&name);
        if candidates.is_empty() {
            self.error(
                call.callee.span,
                format!("unknown function `{}`", call.callee.text),
            );
            return None;
        }
        if candidates.len() == 1 {
            return self.finish_single_function_call(candidates[0], call, sink);
        }
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        let resolved = self.resolve_overload(
            &name,
            &candidates,
            &[],
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: &call.args,
                span: call.span,
            },
            sink,
        )?;
        let ty = resolved.return_ty;
        self.check_call_effects(resolved.callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::Call {
                callee: resolved.callee,
                args: resolved.args,
            },
            ty,
            span: call.span,
        })
    }

    fn ambient_type_args(&mut self, count: usize) -> Vec<TypeId> {
        self.type_params_in_scope[..count]
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect()
    }

    fn resolve_call_type_args(&mut self, refs: &[ast::TypeRef]) -> Option<Vec<TypeId>> {
        refs.iter()
            .map(|type_ref| self.resolve_type_ref(type_ref))
            .collect()
    }

    /// Seed the call's own generic suffix. Receiver/lexical-owner type
    /// parameters occupy the prefix and are never repeated at the call site.
    /// Scoop requires either no explicit arguments (infer the whole suffix)
    /// or the complete suffix; partial explicit lists are intentionally not
    /// ambiguous with inference.
    fn bind_explicit_type_args(
        &mut self,
        bindings: &mut [Option<TypeId>],
        owner_type_param_count: usize,
        explicit: &[TypeId],
        span: Span,
        target: &str,
    ) -> bool {
        if explicit.is_empty() {
            return true;
        }
        let expected = bindings.len() - owner_type_param_count;
        if explicit.len() != expected {
            self.error(
                span,
                format!(
                    "{target} takes exactly {expected} type argument(s), but {} were supplied",
                    explicit.len()
                ),
            );
            return false;
        }
        for (binding, &ty) in bindings[owner_type_param_count..].iter_mut().zip(explicit) {
            *binding = Some(ty);
        }
        true
    }

    fn local_call_capture_args(
        &mut self,
        local_function: hir::LocalFunctionId,
        span: Span,
    ) -> Option<Vec<hir::Expr>> {
        let captures: Vec<_> = self.local_functions[local_function]
            .captures
            .iter()
            .map(|capture| (capture.binding, capture.name.clone(), capture.ty))
            .collect();
        let mut args = Vec::with_capacity(captures.len());
        for (binding, name, ty) in captures {
            if let Some((local, _)) = self
                .locals
                .iter()
                .find(|(_, candidate)| candidate.binding == binding)
            {
                args.push(hir::Expr {
                    kind: ExprKind::Local(local),
                    ty,
                    span,
                });
                continue;
            }
            args.push(self.lower_capture_binding(binding, &name, span)?);
        }
        Some(args)
    }

    fn finish_local_function_call(
        &mut self,
        local_function: hir::LocalFunctionId,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = self.local_functions[local_function].function;
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
                    "local function `{}` takes exactly {expected} {noun}, but {supplied} were supplied",
                    call.callee.text
                ),
            );
            return None;
        }
        let owner_type_args = self.ambient_type_args(sig.owner_type_param_count);
        let mut bindings = vec![None; sig.type_params.len()];
        for (binding, ty) in bindings.iter_mut().zip(owner_type_args) {
            *binding = Some(ty);
        }
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if !self.bind_explicit_type_args(
            &mut bindings,
            sig.owner_type_param_count,
            &explicit_type_args,
            call.span,
            &format!("local function `{}`", call.callee.text),
        ) {
            return None;
        }
        let param_tys: Vec<_> = sig.params.iter().map(|param| param.ty).collect();
        let inferred =
            self.lower_inference_args(&call.args, &param_tys, bindings, &sig.type_params)?;
        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&sig.type_params) {
            let Some(ty) = binding else {
                self.error(
                    call.span,
                    format!(
                        "cannot infer type argument `{}` for local function `{}`",
                        param.name, call.callee.text
                    ),
                );
                return None;
            };
            type_args.push(ty);
        }
        if !self.check_type_argument_kinds(
            &sig.type_params,
            &type_args,
            call.span,
            &format!("local function `{}`", call.callee.text),
        ) {
            return None;
        }
        let args = inferred.finish(sink);
        let mut adapted = Vec::with_capacity(args.len());
        for (param, arg) in sig.params.iter().zip(args) {
            let expected = self.instantiate_ty(param.ty, &type_args);
            if !self.is_subtype(arg.ty, expected) {
                let expected_name = self.type_name(expected);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for parameter `{}` of local function `{}` must be of type {expected_name}, found {found}",
                        param.name.text, call.callee.text
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, expected));
        }
        let return_ty = self.instantiate_ty(sig.return_ty, &type_args);
        let callee = if type_args.is_empty() {
            hir::Callable::Function(function)
        } else {
            hir::Callable::Generic(self.record_instantiation(function, type_args))
        };
        let captures = self.local_call_capture_args(local_function, call.span)?;
        self.check_call_effects(callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args: adapted,
            },
            ty: return_ty,
            span: call.span,
        })
    }

    /// A call to the single candidate of its layer: arity and
    /// argument-type diagnostics name the function directly, and the
    /// parameter types serve as expected-type hints for the arguments
    /// (this is what types `None` in argument position). Type-argument
    /// inference for generic callees follows the M3 rules.
    fn finish_single_function_call(
        &mut self,
        function: hir::FunctionId,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if self.ffi_core.is_some_and(|core| {
            function == core.address_of || function == core.size_of || function == core.align_of
        }) {
            return self.lower_pointer_top_level_intrinsic(function, call);
        }
        let name = self.functions[function].name.clone();

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
                    "function `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }

        // Infer to a fixed point: context-independent arguments may bind
        // parameters needed to type earlier `None` / empty-array arguments.
        // Each argument owns a temporary desugaring sink; the sinks are
        // concatenated in source order after inference, preserving runtime
        // evaluation order even when typing happens in a different order.
        let param_tys: Vec<TypeId> = sig.params.iter().map(|param| param.ty).collect();
        let mut bindings = vec![None; sig.type_params.len()];
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if !self.bind_explicit_type_args(
            &mut bindings,
            sig.owner_type_param_count,
            &explicit_type_args,
            call.span,
            &format!("function `{name}`"),
        ) {
            return None;
        }
        let inferred =
            self.lower_inference_args(&call.args, &param_tys, bindings, &sig.type_params)?;
        let mut type_args = Vec::with_capacity(inferred.bindings.len());
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
            &format!("function `{name}`"),
        ) {
            return None;
        }
        let args = inferred.finish(sink);

        // Argument types must be subtypes of the (instantiated)
        // parameter types; the adaptation boxes value types crossing
        // into `Any` / an interface (M6).
        let mut adapted_args = Vec::with_capacity(args.len());
        for (param, arg) in sig.params.iter().zip(args) {
            let expected = self.instantiate_ty(param.ty, &type_args);
            if !self.is_subtype(arg.ty, expected) {
                let param_name = param.name.text.clone();
                let expected_name = self.type_name(expected);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for parameter `{param_name}` of `{name}` must be of type {expected_name}, found {found}"
                    ),
                );
                return None;
            }
            adapted_args.push(self.adapt_to(arg, expected));
        }
        let ty = self.instantiate_ty(sig.return_ty, &type_args);

        // Every successful generic call (including calls inside generic
        // function bodies, whose type arguments may still mention
        // `Type::Param`) requests an instantiation; mir-lower
        // materializes them.
        let callee = if type_args.is_empty() {
            hir::Callable::Function(function)
        } else {
            hir::Callable::Generic(self.record_instantiation(function, type_args))
        };

        self.check_call_effects(callee, call.span);

        Some(hir::Expr {
            kind: ExprKind::Call {
                callee,
                args: adapted_args,
            },
            ty,
            span: call.span,
        })
    }

    fn lower_pointer_top_level_intrinsic(
        &mut self,
        function: hir::FunctionId,
        call: &ast::CallExpr,
    ) -> Option<hir::Expr> {
        let core = self
            .ffi_core
            .expect("pointer core is validated before bodies are lowered");
        if function == core.address_of {
            if call.args.len() != 1 {
                self.error(
                    call.span,
                    format!(
                        "function `addressOf` takes exactly 1 argument, but {} were supplied",
                        call.args.len()
                    ),
                );
                return None;
            }
            let place = match &call.args[0] {
                ast::Expr::Var(name) => self
                    .scopes
                    .lookup(&name.text)
                    .map(|local| (hir::Place::Local(local), self.locals[local].ty, name.span))
                    .or_else(|| {
                        self.globals_by_name.get(&name.text).copied().map(|global| {
                            (
                                hir::Place::Global(global),
                                self.globals[global].ty,
                                name.span,
                            )
                        })
                    }),
                ast::Expr::This { span } => self
                    .current_this
                    .map(|(local, ty)| (hir::Place::Local(local), ty, *span)),
                expression => {
                    self.error(
                        expression.span(),
                        "`addressOf` argument must be an addressable local, parameter, global, or value-type `this`"
                            .to_string(),
                    );
                    return None;
                }
            };
            let Some((place, place_ty, place_span)) = place else {
                self.error(
                    call.args[0].span(),
                    "`addressOf` argument must be an addressable local, parameter, global, or value-type `this`"
                        .to_string(),
                );
                return None;
            };
            if let hir::Place::Global(global) = place
                && matches!(
                    self.globals[global].storage,
                    hir::GlobalStorage::Extern { .. }
                )
            {
                self.require_unsafe_operation(place_span, "taking the address of an extern global");
            }
            let explicit = self.resolve_call_type_args(&call.type_args)?;
            if explicit.len() > 1 {
                self.error(
                    call.span,
                    format!(
                        "function `addressOf` takes exactly 1 type argument, but {} were supplied",
                        explicit.len()
                    ),
                );
                return None;
            }
            if explicit
                .first()
                .is_some_and(|explicit| !self.types_equal(*explicit, place_ty))
            {
                self.error(
                    place_span,
                    format!(
                        "`addressOf` type argument must match the place type {}, found {}",
                        self.type_name(place_ty),
                        self.type_name(explicit[0])
                    ),
                );
                return None;
            }
            if !self.is_value_ty(place_ty)
                || self.type_contains_param(place_ty)
                || !self.is_gc_free(place_ty)
            {
                self.error(
                    place_span,
                    format!(
                        "`addressOf` requires a concrete GC-free value type, found {}",
                        self.type_name(place_ty)
                    ),
                );
                return None;
            }
            self.check_call_effects(hir::Callable::Function(function), call.span);
            let ty = self.intern_type(Type::Ptr(place_ty));
            return Some(hir::Expr {
                kind: ExprKind::AddressOf(place),
                ty,
                span: call.span,
            });
        }

        if !call.args.is_empty() {
            let name = if function == core.size_of {
                "sizeOf"
            } else {
                "alignOf"
            };
            self.error(
                call.span,
                format!(
                    "function `{name}` takes exactly 0 arguments, but {} were supplied",
                    call.args.len()
                ),
            );
            return None;
        }
        let explicit = self.resolve_call_type_args(&call.type_args)?;
        if explicit.len() != 1 {
            let name = if function == core.size_of {
                "sizeOf"
            } else {
                "alignOf"
            };
            self.error(
                call.span,
                format!(
                    "function `{name}` requires exactly 1 explicit type argument, but {} were supplied",
                    explicit.len()
                ),
            );
            return None;
        }
        let target = explicit[0];
        if !self.is_value_ty(target) || self.type_contains_param(target) || !self.is_gc_free(target)
        {
            self.error(
                call.span,
                format!(
                    "memory layout requires a concrete GC-free value type, found {}",
                    self.type_name(target)
                ),
            );
            return None;
        }
        let kind = if function == core.size_of {
            ExprKind::SizeOf(target)
        } else {
            ExprKind::AlignOf(target)
        };
        Some(hir::Expr {
            kind,
            ty: self.uint,
            span: call.span,
        })
    }

    /// Lower generic-call/constructor arguments to a fixed point. An
    /// expression that intrinsically needs an expected type is postponed
    /// while its parameter still contains an unbound type variable; other
    /// arguments can then add bindings independently of their source order.
    fn lower_inference_args(
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

    fn constructor_requires_expected(&self, name: &ast::Ident, args: &[ast::Expr]) -> bool {
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

    fn qualified_variant_requires_expected(
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
    fn constructor_inference_shape(&self, name: &str) -> Option<(usize, Vec<TypeId>)> {
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

    fn mark_type_params(&self, ty: TypeId, bound: &mut [bool]) {
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

    fn unit_variant_from_field(&self, access: &ast::FieldAccess) -> Option<(hir::EnumId, u32)> {
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
    fn bind_type_args(
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

    /// Normalize the two compiler-known FFI value constructors. Their source
    /// structs exist to make the surface API explicit, but no aggregate value
    /// or source field survives in typed HIR.
    fn lower_ffi_struct_init(
        &mut self,
        struct_id: hir::StructId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if Some(struct_id) == self.ffi_ptr {
            if call.args.len() != 1 {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` takes exactly 1 argument, but {} were supplied",
                        call.args.len()
                    ),
                );
                return None;
            }
            let explicit = self.resolve_call_type_args(call.type_args)?;
            if explicit.len() > 1 {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` takes exactly 1 type argument, but {} were supplied",
                        explicit.len()
                    ),
                );
                return None;
            }
            let expected_pointee = expected.and_then(|ty| match self.types[ty] {
                Type::Ptr(pointee) => Some(pointee),
                _ => None,
            });
            let pointee = explicit.first().copied().or(expected_pointee);
            let Some(pointee) = pointee else {
                self.error(
                    call.span,
                    "cannot infer `Ptr` pointee type; provide `Ptr<T>` or an expected `Ptr<T>` type"
                        .to_string(),
                );
                return None;
            };
            if explicit.first().is_some_and(|explicit| {
                expected_pointee.is_some_and(|expected| !self.types_equal(*explicit, expected))
            }) {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` constructor produces Ptr<{}>, which does not match the expected type",
                        self.type_name(pointee)
                    ),
                );
                return None;
            }
            if !self.is_value_ty(pointee)
                || self.type_contains_param(pointee)
                || !self.is_gc_free(pointee)
            {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` pointee must be a concrete GC-free value type, found {}",
                        self.type_name(pointee)
                    ),
                );
                return None;
            }
            self.require_unsafe_operation(call.span, "constructing `Ptr` from a raw integer");
            let raw = self.lower_expr(&call.args[0], sink, Some(self.uint))?;
            if raw.ty != self.uint {
                self.error(
                    raw.span,
                    format!(
                        "`Ptr` raw value must be of type UInt, found {}",
                        self.type_name(raw.ty)
                    ),
                );
                return None;
            }
            let ty = self.intern_type(Type::Ptr(pointee));
            return Some(hir::Expr {
                kind: ExprKind::PtrFromUInt(Box::new(raw)),
                ty,
                span: call.span,
            });
        }

        debug_assert_eq!(Some(struct_id), self.ffi_fun_ptr);
        if !call.args.is_empty() {
            self.error(
                call.span,
                "`FunPtr` only supports the zero-argument null constructor".to_string(),
            );
            return None;
        }
        let explicit = self.resolve_call_type_args(call.type_args)?;
        if explicit.len() > 1 {
            self.error(
                call.span,
                format!(
                    "`FunPtr` takes exactly 1 type argument, but {} were supplied",
                    explicit.len()
                ),
            );
            return None;
        }
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::FunPtr(signature) => Some(signature),
            _ => None,
        });
        let explicit_signature = explicit.first().and_then(|ty| match self.types[*ty] {
            Type::Function(signature) => Some(signature),
            _ => None,
        });
        if !explicit.is_empty() && explicit_signature.is_none() {
            self.error(
                call.span,
                "`FunPtr` type argument must be an ordinary concrete function type".to_string(),
            );
            return None;
        }
        let signature = explicit_signature.or(expected_signature);
        let Some(signature) = signature else {
            self.error(
                call.span,
                "cannot infer `FunPtr` signature; provide `FunPtr<F>` or an expected `FunPtr<F>` type"
                    .to_string(),
            );
            return None;
        };
        if explicit_signature.is_some()
            && expected_signature.is_some()
            && explicit_signature != expected_signature
        {
            self.error(
                call.span,
                "explicit `FunPtr` signature does not match the expected type".to_string(),
            );
            return None;
        }
        if self.function_types[signature].is_suspend || self.function_type_contains_param(signature)
        {
            self.error(
                call.span,
                "`FunPtr` type argument must be an ordinary concrete function type".to_string(),
            );
            return None;
        }
        let ty = self.intern_type(Type::FunPtr(signature));
        Some(hir::Expr {
            kind: ExprKind::FunPtrNull,
            ty,
            span: call.span,
        })
    }

    /// Struct construction with positional arguments: argument count
    /// and types must match the declared fields one by one (the field
    /// type is the argument's expected-type hint).
    fn lower_struct_init(
        &mut self,
        struct_id: hir::StructId,
        definition_ty: TypeId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let CallSite {
            type_args: type_arg_refs,
            args,
            span,
        } = call;
        let name = self.structs[struct_id].name.clone();
        if matches!(
            self.structs[struct_id].representation,
            hir::StructRepresentation::Intrinsic(_)
        ) {
            self.error(
                span,
                format!("intrinsic struct `{name}` has no source constructor"),
            );
            return None;
        }
        let type_params = self.structs[struct_id].type_params.clone();
        let fields: Vec<(String, TypeId)> = self.structs[struct_id]
            .semantic_fields()
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        if args.len() != fields.len() {
            let expected = fields.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "struct `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        let explicit_type_args = self.resolve_call_type_args(type_arg_refs)?;
        let mut bindings = vec![None; type_params.len()];
        if !self.bind_explicit_type_args(
            &mut bindings,
            0,
            &explicit_type_args,
            span,
            &format!("struct `{name}`"),
        ) {
            return None;
        }
        if let Some(expected) = expected {
            if let Type::Struct(application) = self.types[expected] {
                let application = self.struct_applications[application].clone();
                if application.template == struct_id
                    && application.arguments.len() == type_params.len()
                {
                    for (binding, arg) in bindings.iter_mut().zip(application.arguments) {
                        if binding.is_none() {
                            *binding = Some(arg);
                        }
                    }
                }
            }
        }

        let field_tys: Vec<TypeId> = fields.iter().map(|(_, ty)| *ty).collect();
        let inferred = self.lower_inference_args(args, &field_tys, bindings, &type_params)?;

        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        span,
                        format!(
                            "cannot infer type argument `{}` for struct `{name}`",
                            param.name
                        ),
                    );
                    return None;
                }
            }
        }
        if !self.check_type_argument_kinds(
            &type_params,
            &type_args,
            span,
            &format!("struct `{name}`"),
        ) {
            return None;
        }
        let lowered = inferred.finish(sink);

        let mut adapted = Vec::with_capacity(lowered.len());
        for ((field_name, field_ty), arg) in fields.iter().zip(lowered) {
            let field_ty = self.instantiate_ty(*field_ty, &type_args);
            if !self.is_subtype(arg.ty, field_ty) {
                let expected = self.type_name(field_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for field `{field_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, field_ty));
        }
        let application = self.struct_application_id(struct_id, type_args);
        let ty = self.struct_applications[application].canonical_type;
        debug_assert!(
            !self.structs[struct_id].type_params.is_empty() || self.types_equal(ty, definition_ty)
        );
        Some(hir::Expr {
            kind: ExprKind::StructInit {
                application,
                args: adapted,
            },
            ty,
            span,
        })
    }

    fn lower_field_access(
        &mut self,
        access: &ast::FieldAccess,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        // `E.V` where `E` is an enum: a unit variant construction
        // (`Color.Red`). Variants with fields are constructors and must
        // be called (`E.V(...)`).
        if let ast::Expr::Var(name) = &*access.receiver {
            if let Some(&enum_id) = self.enums_by_name.get(&name.text) {
                return self.lower_qualified_variant(enum_id, access, expected);
            }
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
                    ty: self.int,
                    span: access.span,
                });
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
        })
    }

    /// `E.V` with `E` an enum (see `lower_field_access`).
    fn lower_qualified_variant(
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
        let Some(variant) = self.find_variant(enum_id, &variant_name.text) else {
            self.error(
                variant_name.span,
                format!("enum `{enum_name}` has no variant `{}`", variant_name.text),
            );
            return None;
        };
        let arity = self.enums[enum_id].variants[variant as usize].fields.len();
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
        self.lower_unit_variant(variant_name, enum_id, variant, expected)
    }

    /// `receiver?.field`: the receiver must be an `Option<S>`; the
    /// result is an `Option<F>` where `F` is the field type. Desugared
    /// (see the module docs): `$opt.N = receiver`, then
    /// `if isSome($opt.N) { $res.M = Some(unwrap($opt.N).field) } else { $res.M = None }`
    /// and the expression evaluates to `$res.M`.
    fn lower_safe_field_access(
        &mut self,
        access: &ast::FieldAccess,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(&access.receiver, sink, None)?;
        let Some(inner) = self.as_option(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                access.span,
                format!("`?.` requires an Option receiver, found {found}"),
            );
            return None;
        };
        let (field, field_ty) = self.resolve_field(inner, &access.selector)?;
        let result_ty = self.option_type(field_ty);
        let span = access.span;
        let then_value = move |tmp: hir::Expr| {
            let unwrapped = hir::Expr {
                kind: ExprKind::Unwrap {
                    operand: Box::new(tmp),
                    trap_on_none: false,
                },
                ty: inner,
                span,
            };
            let field_access = hir::Expr {
                kind: ExprKind::FieldAccess {
                    receiver: Box::new(unwrapped),
                    field,
                },
                ty: field_ty,
                span,
            };
            hir::Expr {
                kind: ExprKind::SomeWrap(Box::new(field_access)),
                ty: result_ty,
                span,
            }
        };
        let else_value = hir::Expr {
            kind: ExprKind::NoneLiteral,
            ty: result_ty,
            span,
        };
        Some(self.desugar_option(
            receiver,
            result_ty,
            span,
            sink,
            then_value,
            ElseBranch {
                statements: Vec::new(),
                value: else_value,
            },
        ))
    }

    /// `lhs ?: rhs`: `lhs` must be an `Option<T>` and `rhs` a `T` (the
    /// right-hand side gets `T` as its expected-type hint). Desugared:
    /// `$opt.N = lhs`, then
    /// `if isSome($opt.N) { $res.M = unwrap($opt.N) } else { $res.M = rhs }`
    /// and the expression evaluates to `$res.M`. The right-hand side is
    /// lowered into the else branch directly, so it (including its own
    /// desugaring statements) is only evaluated on the `None` path.
    fn lower_elvis(
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
            self.error(
                rhs.span,
                format!("right-hand side of `?:` must be of type {expected}, found {found}"),
            );
            return None;
        }
        let then_value = move |tmp: hir::Expr| hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(tmp),
                trap_on_none: false,
            },
            ty: inner,
            span,
        };
        Some(self.desugar_option(
            lhs,
            inner,
            span,
            sink,
            then_value,
            ElseBranch {
                statements: else_body,
                value: rhs,
            },
        ))
    }

    /// `operand!!`: the operand must be an `Option<T>`; the result is
    /// `T`, trapping on `None` (M3: `scoop_rt_trap`; M8: a real
    /// `UnwrapException`, DESIGN.md 5.2).
    fn lower_null_assert(
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
        })
    }

    /// The shared `?.` / `?:` desugaring skeleton (see the module
    /// docs): push `$opt.N = receiver` and an `if isSome($opt.N)` whose
    /// branches each initialize the hidden `$res.N` result local —
    /// `then_value($opt.N)` in the then-branch, the else branch's value
    /// (preceded by its own statements) in the else-branch. Returns a
    /// reference to `$res.N`.
    fn desugar_option(
        &mut self,
        receiver: hir::Expr,
        result_ty: TypeId,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        then_value: impl FnOnce(hir::Expr) -> hir::Expr,
        else_branch: ElseBranch,
    ) -> hir::Expr {
        let option_ty = receiver.ty;
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
        };
        let cond = hir::Expr {
            kind: ExprKind::IsSome(Box::new(tmp_expr(span))),
            ty: self.boolean,
            span,
        };
        let result = self.alloc_hidden("res", result_ty);
        let then_body = vec![hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: then_value(tmp_expr(span)),
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
        hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span,
        }
    }

    /// Resolve a field selector against a receiver type: a struct field
    /// by name, a class constructor property by name (base chain
    /// included; the index is the absolute layout index — base fields
    /// prefix, own fields consecutive), or a tuple element by (1-based)
    /// index.
    fn resolve_field(
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
                        let Some((declaring, index, ty, _)) =
                            self.find_class_application_field(application, &field.text)
                        else {
                            self.error(
                                field.span,
                                format!("class `{class_name}` has no field `{}`", field.text),
                            );
                            return None;
                        };
                        Some((
                            hir::FieldRef::ClassField {
                                application: declaring,
                                index,
                            },
                            ty,
                        ))
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

    fn lower_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // `===` / `!==` (spec 4.4.2): reference identity, only on
        // reference types; on value types it is a compile error.
        if matches!(op, ast::BinOp::RefEq | ast::BinOp::RefNe) {
            return self.lower_ref_eq(op, lhs, rhs, span, sink);
        }
        let (op, symbol) = convert_bin_op(op);
        // `x == None` / `None == x`: the `None` construction takes its
        // type from the other operand (expected-type hint), so the
        // other side is lowered first. (`None` itself is side-effect
        // free, so lowering order is unobservable here.)
        let (lhs, rhs) = if matches!(op, hir::BinOp::Eq | hir::BinOp::Ne)
            && is_none_literal(lhs)
            && !is_none_literal(rhs)
        {
            let rhs = self.lower_expr(rhs, sink, None)?;
            let lhs = self.lower_expr(lhs, sink, Some(rhs.ty))?;
            (lhs, rhs)
        } else if op == hir::BinOp::And {
            // `x is T && ...`: the right side is only evaluated when
            // the left holds, so its smart-cast narrowings apply there
            // (milestone6 DESIGN.md 5.4).
            let lhs_ast = lhs;
            let before = self.diagnostics.len();
            let lhs = self.lower_expr(lhs_ast, sink, None)?;
            let narrowings = if self.diagnostics.len() == before {
                self.resolve_smart_casts(lhs_ast, true)
            } else {
                Vec::new()
            };
            let rhs = self.with_smart_casts(narrowings, |this| this.lower_expr(rhs, sink, None))?;
            (lhs, rhs)
        } else {
            let lhs = self.lower_expr(lhs, sink, None)?;
            let rhs_hint = if matches!(op, hir::BinOp::Eq | hir::BinOp::Ne) && is_none_literal(rhs)
            {
                Some(lhs.ty)
            } else {
                None
            };
            let rhs = self.lower_expr(rhs, sink, rhs_hint)?;
            (lhs, rhs)
        };
        if matches!(op, hir::BinOp::Add | hir::BinOp::Sub)
            && matches!(self.types[lhs.ty], Type::Ptr(_))
        {
            if rhs.ty != self.int {
                self.error(
                    span,
                    format!(
                        "pointer operator `{symbol}` requires an Int offset, found {}",
                        self.type_name(rhs.ty)
                    ),
                );
                return None;
            }
            self.require_unsafe_operation(span, "pointer arithmetic");
            let ty = lhs.ty;
            return Some(hir::Expr {
                kind: ExprKind::PtrOffset {
                    pointer: Box::new(lhs),
                    offset: Box::new(rhs),
                    subtract: op == hir::BinOp::Sub,
                },
                ty,
                span,
            });
        }
        let ty = match op {
            hir::BinOp::Add => {
                // `String + String` concatenates (docs/milestone2/
                // DESIGN.md 2.3); all other arithmetic is numeric
                // (Int, or UInt since M9).
                if lhs.ty == self.string && rhs.ty == self.string {
                    self.string
                } else {
                    self.expect_numeric_operands(symbol, &lhs, &rhs, span)?
                }
            }
            hir::BinOp::Sub | hir::BinOp::Mul | hir::BinOp::Div => {
                self.expect_numeric_operands(symbol, &lhs, &rhs, span)?
            }
            hir::BinOp::Lt | hir::BinOp::Le | hir::BinOp::Gt | hir::BinOp::Ge => {
                self.expect_numeric_operands(symbol, &lhs, &rhs, span)?;
                self.boolean
            }
            hir::BinOp::Eq | hir::BinOp::Ne | hir::BinOp::RefEq | hir::BinOp::RefNe => {
                // Every type supports structural equality (including
                // type parameters and enums — `== None` relies on
                // this); the two sides just have to agree. The
                // expansion over enum payloads happens in MIR
                // (milestone4 DESIGN.md 3.3). (`RefEq` / `RefNe` never
                // reach here — `lower_ref_eq` intercepts them and
                // builds the `Binary` node directly — but the
                // same-type check would be correct for them too.)
                if !self.types_equal(lhs.ty, rhs.ty) {
                    let lhs_ty = self.type_name(lhs.ty);
                    let rhs_ty = self.type_name(rhs.ty);
                    self.error(
                        span,
                        format!(
                            "operator `{symbol}` requires operands of the same type, found {lhs_ty} and {rhs_ty}"
                        ),
                    );
                    return None;
                }
                self.boolean
            }
            hir::BinOp::And | hir::BinOp::Or => {
                if lhs.ty != self.boolean || rhs.ty != self.boolean {
                    let lhs_ty = self.type_name(lhs.ty);
                    let rhs_ty = self.type_name(rhs.ty);
                    self.error(
                        span,
                        format!(
                            "operator `{symbol}` requires Boolean operands, found {lhs_ty} and {rhs_ty}"
                        ),
                    );
                    return None;
                }
                self.boolean
            }
        };
        Some(hir::Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
            span,
        })
    }

    /// `===` / `!==` (spec 4.4.2): both operands must be reference
    /// types (class / interface / `Any` / `String` / arrays); on value
    /// types it is a compile error. Identity vs structural equality is
    /// decided at MIR from the operand types — reference operands
    /// compare by pointer.
    fn lower_ref_eq(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let symbol = if op == ast::BinOp::RefEq {
            "==="
        } else {
            "!=="
        };
        let lhs = self.lower_expr(lhs, sink, None)?;
        let rhs = self.lower_expr(rhs, sink, None)?;
        if !self.is_ref_ty(lhs.ty) || !self.is_ref_ty(rhs.ty) {
            self.error(
                span,
                format!("reference equality `{symbol}` is not supported on value types"),
            );
            return None;
        }
        let op = if op == ast::BinOp::RefEq {
            hir::BinOp::RefEq
        } else {
            hir::BinOp::RefNe
        };
        Some(hir::Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: self.boolean,
            span,
        })
    }

    /// Arithmetic and comparison operators require both operands to
    /// share one numeric type and return it (`Int` or — M9,
    /// milestone9 DESIGN.md section 1 — `UInt`, under the same rules;
    /// comparisons yield `Boolean` at the call site). Mixing `Int`
    /// and `UInt` is an error: the types are distinct and there is no
    /// implicit conversion (spec 11.2). Type parameters are rejected
    /// too: `T` is unconstrained, so no operation beyond `==` / `!=`
    /// can be proven valid at the definition site (DESIGN.md 2.2).
    /// The unsigned semantics risks of `UInt` arithmetic (subtraction
    /// underflow, signed-vs-unsigned comparison) are deferred to a
    /// later milestone; the machine word wraps for now.
    fn expect_numeric_operands(
        &mut self,
        symbol: &str,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> Option<TypeId> {
        if lhs.ty == self.int && rhs.ty == self.int {
            return Some(self.int);
        }
        if lhs.ty == self.uint && rhs.ty == self.uint {
            return Some(self.uint);
        }
        let lhs_ty = self.type_name(lhs.ty);
        let rhs_ty = self.type_name(rhs.ty);
        let message = if lhs.ty == self.uint || rhs.ty == self.uint {
            format!(
                "operator `{symbol}` requires Int or UInt operands of the same type, found {lhs_ty} and {rhs_ty}"
            )
        } else {
            format!("operator `{symbol}` requires Int operands, found {lhs_ty} and {rhs_ty}")
        };
        self.error(span, message);
        None
    }

    fn lower_unary(
        &mut self,
        op: ast::UnOp,
        operand: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let (op, symbol, expected, ty) = match op {
            ast::UnOp::Neg => (hir::UnOp::Neg, "-", self.int, self.int),
            ast::UnOp::Not => (hir::UnOp::Not, "!", self.boolean, self.boolean),
        };
        if operand.ty != expected {
            let article = if expected == self.int { "an" } else { "a" };
            let expected_name = self.type_name(expected);
            let found = self.type_name(operand.ty);
            self.error(
                span,
                format!(
                    "operator `{symbol}` requires {article} {expected_name} operand, found {found}"
                ),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            ty,
            span,
        })
    }
}

/// What a `Name` / `Name(...)` construction site resolved to (see
/// `classify_constructor`).
enum Constructor {
    Variant {
        enum_id: hir::EnumId,
        variant: u32,
    },
    Struct {
        struct_id: hir::StructId,
        ty: TypeId,
    },
    Class {
        class_id: hir::ClassId,
    },
    Unmatched,
}

/// Collect the `x is T` facts established by `cond` evaluating to
/// `outcome` (see `resolve_smart_casts`).
fn collect_smart_cast_candidates<'a>(
    cond: &'a ast::Expr,
    outcome: bool,
    out: &mut Vec<(&'a ast::Ident, &'a ast::TypeRef)>,
) {
    match cond {
        // `x is T` holds exactly when the check is not negated and the
        // condition is true (or it is negated and the condition is
        // false).
        ast::Expr::Is {
            operand,
            ty,
            negated,
            ..
        } => {
            if outcome == !negated {
                if let ast::Expr::Var(name) = &**operand {
                    out.push((name, ty));
                }
            }
        }
        ast::Expr::Unary {
            op: ast::UnOp::Not,
            operand,
            ..
        } => collect_smart_cast_candidates(operand, !outcome, out),
        // `a && b` is true only when both hold; a false conjunction
        // establishes nothing (M6: no `||` support).
        ast::Expr::Binary {
            op: ast::BinOp::And,
            lhs,
            rhs,
            ..
        } if outcome => {
            collect_smart_cast_candidates(lhs, true, out);
            collect_smart_cast_candidates(rhs, true, out);
        }
        _ => {}
    }
}

fn block_contains_return(block: &ast::Block) -> bool {
    block.statements.iter().any(statement_contains_return)
}

fn statement_contains_return(statement: &ast::Statement) -> bool {
    match &statement.kind {
        ast::StatementKind::Return { .. } => true,
        ast::StatementKind::LocalFunction(_) => false,
        ast::StatementKind::Expr(expr) | ast::StatementKind::Throw(expr) => {
            expr_contains_return(expr)
        }
        ast::StatementKind::ValDecl(decl) => expr_contains_return(&decl.init),
        ast::StatementKind::Assign(assign) => expr_contains_return(&assign.value),
        ast::StatementKind::If(if_) => {
            expr_contains_return(&if_.cond)
                || block_contains_return(&if_.then_block)
                || if_.else_block.as_ref().is_some_and(block_contains_return)
        }
        ast::StatementKind::While(while_) => {
            expr_contains_return(&while_.cond) || block_contains_return(&while_.body)
        }
        ast::StatementKind::Block(block) | ast::StatementKind::SafetyBlock { block, .. } => {
            block_contains_return(block)
        }
        ast::StatementKind::When(when) => {
            expr_contains_return(&when.subject)
                || when.arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_contains_return)
                        || block_contains_return(&arm.body)
                })
                || when.else_body.as_ref().is_some_and(block_contains_return)
        }
        ast::StatementKind::Try(try_) => {
            block_contains_return(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| block_contains_return(&catch.body))
                || try_
                    .finally_body
                    .as_ref()
                    .is_some_and(block_contains_return)
        }
    }
}

fn expr_contains_return(expr: &ast::Expr) -> bool {
    match expr {
        // A nested callable owns its own return target.
        ast::Expr::Lambda { .. }
        | ast::Expr::AnonymousFunction { .. }
        | ast::Expr::CallableReference { .. } => false,
        ast::Expr::TupleLiteral { elements, .. } | ast::Expr::ArrayLiteral { elements, .. } => {
            elements.iter().any(expr_contains_return)
        }
        ast::Expr::StructInit { args, .. } => args.iter().any(expr_contains_return),
        ast::Expr::FieldAccess(access) => expr_contains_return(&access.receiver),
        ast::Expr::Call(call) => call.args.iter().any(expr_contains_return),
        ast::Expr::Invoke { callee, args, .. } => {
            expr_contains_return(callee) || args.iter().any(expr_contains_return)
        }
        ast::Expr::Binary { lhs, rhs, .. } | ast::Expr::Elvis { lhs, rhs, .. } => {
            expr_contains_return(lhs) || expr_contains_return(rhs)
        }
        ast::Expr::Unary { operand, .. }
        | ast::Expr::NullAssert { operand, .. }
        | ast::Expr::Is { operand, .. }
        | ast::Expr::Cast { operand, .. } => expr_contains_return(operand),
        ast::Expr::MethodCall { receiver, args, .. } => {
            expr_contains_return(receiver) || args.iter().any(expr_contains_return)
        }
        ast::Expr::Index {
            receiver, index, ..
        } => expr_contains_return(receiver) || expr_contains_return(index),
        ast::Expr::If(if_) => {
            expr_contains_return(&if_.cond)
                || block_contains_return(&if_.then_block)
                || if_.else_block.as_ref().is_some_and(block_contains_return)
        }
        ast::Expr::When(when) => {
            expr_contains_return(&when.subject)
                || when.arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_contains_return)
                        || block_contains_return(&arm.body)
                })
                || when.else_body.as_ref().is_some_and(block_contains_return)
        }
        ast::Expr::Try(try_) => {
            block_contains_return(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| block_contains_return(&catch.body))
                || try_
                    .finally_body
                    .as_ref()
                    .is_some_and(block_contains_return)
        }
        ast::Expr::StringLiteral { .. }
        | ast::Expr::IntLiteral { .. }
        | ast::Expr::BoolLiteral { .. }
        | ast::Expr::UnitLiteral { .. }
        | ast::Expr::Var(_)
        | ast::Expr::This { .. } => false,
    }
}

/// The else half of a `?.` / `?:` desugaring: the statements evaluating
/// the fallback (lazily, inside the branch), then the fallback value.
struct ElseBranch {
    statements: Vec<hir::Statement>,
    value: hir::Expr,
}

/// Whether the expression is the `None` construction (see
/// `lower_binary`).
fn is_none_literal(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Var(name) if name.text == "None")
}

/// Copy a variant field default. Defaults are literals
/// (`resolve_variant_default` enforces this), so copying is trivial.
fn clone_literal(expr: &hir::Expr) -> hir::Expr {
    let kind = match &expr.kind {
        ExprKind::IntLiteral(value) => ExprKind::IntLiteral(*value),
        ExprKind::StringLiteral(value) => ExprKind::StringLiteral(value.clone()),
        ExprKind::BoolLiteral(value) => ExprKind::BoolLiteral(*value),
        ExprKind::Unary { op, operand } => ExprKind::Unary {
            op: *op,
            operand: Box::new(clone_literal(operand)),
        },
        _ => unreachable!("variant defaults are literals (resolve_variant_default)"),
    };
    hir::Expr {
        kind,
        ty: expr.ty,
        span: expr.span,
    }
}

fn convert_bin_op(op: ast::BinOp) -> (hir::BinOp, &'static str) {
    match op {
        ast::BinOp::Add => (hir::BinOp::Add, "+"),
        ast::BinOp::Sub => (hir::BinOp::Sub, "-"),
        ast::BinOp::Mul => (hir::BinOp::Mul, "*"),
        ast::BinOp::Div => (hir::BinOp::Div, "/"),
        ast::BinOp::Lt => (hir::BinOp::Lt, "<"),
        ast::BinOp::Le => (hir::BinOp::Le, "<="),
        ast::BinOp::Gt => (hir::BinOp::Gt, ">"),
        ast::BinOp::Ge => (hir::BinOp::Ge, ">="),
        ast::BinOp::Eq => (hir::BinOp::Eq, "=="),
        ast::BinOp::Ne => (hir::BinOp::Ne, "!="),
        // Intercepted by `lower_ref_eq` before `convert_bin_op` is
        // reached; mapped here for completeness.
        ast::BinOp::RefEq => (hir::BinOp::RefEq, "==="),
        ast::BinOp::RefNe => (hir::BinOp::RefNe, "!=="),
        ast::BinOp::And => (hir::BinOp::And, "&&"),
        ast::BinOp::Or => (hir::BinOp::Or, "||"),
    }
}
