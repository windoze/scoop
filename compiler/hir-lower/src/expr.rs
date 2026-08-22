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
//! `Some(None)` under an `Int??` annotation). Every other expression
//! ignores the hint and mismatches are reported by the context's own
//! type check.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{ExprKind, Type, TypeId};

use crate::Lowerer;

impl Lowerer {
    /// Lower an expression, recording a diagnostic and returning `None`
    /// on error. See the module docs for the `sink` / `expected`
    /// mechanisms.
    pub(crate) fn lower_expr(
        &mut self,
        expr: &ast::Expr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match expr {
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
                self.lower_tuple_literal(elements, *span, sink)
            }
            // `Name(args...)` where the parser already knows `Name` is
            // a type (struct or enum variant path).
            ast::Expr::StructInit { name, args, span } => match self.classify_constructor(name)? {
                Constructor::Struct { struct_id, ty } => {
                    self.lower_struct_init(struct_id, ty, args, *span, sink)
                }
                Constructor::Variant { enum_id, variant } => {
                    self.lower_variant_construct(enum_id, variant, args, *span, sink, expected)
                }
                Constructor::Unmatched => {
                    self.error(name.span, format!("unknown struct `{}`", name.text));
                    None
                }
            },
            ast::Expr::Var(name) => self.lower_var(name, expected),
            ast::Expr::FieldAccess(access) if access.safe => {
                self.lower_safe_field_access(access, sink)
            }
            ast::Expr::FieldAccess(access) => self.lower_field_access(access, sink, expected),
            ast::Expr::Call(call) => self.lower_call(call, sink, expected),
            ast::Expr::Binary { op, lhs, rhs, span } => {
                self.lower_binary(*op, lhs, rhs, *span, sink)
            }
            ast::Expr::Unary { op, operand, span } => self.lower_unary(*op, operand, *span, sink),
            ast::Expr::NullAssert { operand, span } => self.lower_null_assert(operand, *span, sink),
            ast::Expr::Elvis { lhs, rhs, span } => self.lower_elvis(lhs, rhs, *span, sink),
        }
    }

    fn lower_tuple_literal(
        &mut self,
        elements: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
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
        let mut lowered = Vec::with_capacity(elements.len());
        for element in elements {
            lowered.push(self.lower_expr(element, sink, None)?);
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

    /// A bare identifier in expression position. The `Option` variants
    /// (`Some` / `None`) are visible without a prefix (spec 7.2 default
    /// import) and take precedence over locals (M3 behavior); every
    /// other enum's variants need the `E.V` prefix (M4 simplification,
    /// milestone4 DESIGN.md 3.2).
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
            self.error(name.span, format!("unknown variable `{}`", name.text));
            return None;
        };
        let ty = self.locals[local].ty;
        Some(hir::Expr {
            kind: ExprKind::Local(local),
            ty,
            span: name.span,
        })
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
                Type::Enum(id, args) if id == enum_id && args.len() == arity => Some(args),
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
        let ty = self.intern_type(Type::Enum(enum_id, type_args.clone()));
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                enum_id,
                variant,
                type_args,
                args: Vec::new(),
            },
            ty,
            span: name.span,
        })
    }

    /// What a `Name` / `Name(...)` construction site resolves to. A
    /// dotted path `E.V` is always an enum variant; a bare name is a
    /// globally visible `Option` variant (`Some` / `None`), then a
    /// struct, then — for `Call` nodes only — a function.
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
        Some(Constructor::Unmatched)
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
        match self.classify_constructor(&call.callee)? {
            Constructor::Variant { enum_id, variant } => self
                .lower_variant_construct(enum_id, variant, &call.args, call.span, sink, expected),
            Constructor::Struct { struct_id, ty } => {
                self.lower_struct_init(struct_id, ty, &call.args, call.span, sink)
            }
            Constructor::Unmatched => self.lower_function_call(call, sink),
        }
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
        args: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
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

        let mut bindings = vec![None; type_params.len()];
        if let Some(expected) = expected {
            if let Type::Enum(id, expected_args) = self.types[expected].clone() {
                if id == enum_id && expected_args.len() == type_params.len() {
                    for (binding, arg) in bindings.iter_mut().zip(expected_args) {
                        *binding = Some(arg);
                    }
                }
            }
        }
        let mut lowered = Vec::with_capacity(total);
        for (arg_expr, (_, field_ty)) in args.iter().zip(&fields) {
            let hint = self.try_substitute(*field_ty, &bindings);
            let arg = self.lower_expr(arg_expr, sink, hint)?;
            if !self.bind_type_args(*field_ty, arg.ty, &mut bindings, &type_params, arg.span) {
                return None; // conflict diagnostic already recorded
            }
            lowered.push(arg);
        }

        // Arity first: missing trailing fields must have
        // constructor-style defaults (checked before inference so a
        // short call reports arity, not an unbound type argument).
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

        let mut type_args = Vec::with_capacity(bindings.len());
        for (binding, param_name) in bindings.into_iter().zip(&type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        span,
                        format!(
                            "cannot infer type argument `{param_name}` for `{enum_name}.{variant_name}`"
                        ),
                    );
                    return None;
                }
            }
        }

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

        let ty = self.intern_type(Type::Enum(enum_id, type_args.clone()));
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                enum_id,
                variant,
                type_args,
                args: lowered,
            },
            ty,
            span,
        })
    }

    fn lower_function_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = match self.functions_by_name.get(&call.callee.text) {
            Some(&id) => id,
            None => {
                self.error(
                    call.callee.span,
                    format!("unknown function `{}`", call.callee.text),
                );
                return None;
            }
        };
        let name = self.functions[function].name.clone();

        // Intrinsics are checked against the registry's signature rules
        // (impl spec 2.10), not their declared parameter list: the M4
        // entries `rt_print` / `rt_println` take exactly one
        // `String` / `Int` / `Boolean` argument and return `Unit`
        // (M2/M3 behavior, milestone4 DESIGN.md 1.3). A type parameter
        // is not printable: `T` is unconstrained, so there is no way to
        // prove it at the definition site.
        if let hir::FunctionKind::Intrinsic(intrinsic) = &self.functions[function].kind {
            debug_assert!(
                matches!(intrinsic.as_str(), "rt_print" | "rt_println"),
                "every registry entry needs a signature rule here"
            );
            if call.args.len() != 1 {
                let supplied = call.args.len();
                self.error(
                    call.span,
                    format!("`{name}` takes exactly 1 argument, but {supplied} were supplied"),
                );
                return None;
            }
            let arg = self.lower_expr(&call.args[0], sink, None)?;
            if arg.ty != self.string && arg.ty != self.int && arg.ty != self.boolean {
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!("argument of `{name}` must be String, Int or Boolean, found {found}"),
                );
                return None;
            }
            return Some(hir::Expr {
                kind: ExprKind::Call {
                    function,
                    type_args: Vec::new(),
                    args: vec![arg],
                },
                ty: self.unit,
                span: call.span,
            });
        }

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

        // Type-argument inference (DESIGN.md 2.2): walk the arguments
        // left to right, binding each type parameter by matching the
        // parameter type against the argument type. An argument whose
        // parameter type is already fully known gets it as the
        // expected-type hint (this types `None` in argument position).
        let mut bindings: Vec<Option<TypeId>> = vec![None; sig.type_params.len()];
        let mut args = Vec::with_capacity(call.args.len());
        for (param, arg_expr) in sig.params.iter().zip(&call.args) {
            let hint = self.try_substitute(param.ty, &bindings);
            let arg = self.lower_expr(arg_expr, sink, hint)?;
            if !self.bind_type_args(param.ty, arg.ty, &mut bindings, &sig.type_params, arg.span) {
                return None; // conflict diagnostic already recorded
            }
            args.push(arg);
        }
        let mut type_args = Vec::with_capacity(bindings.len());
        for (binding, param_name) in bindings.into_iter().zip(&sig.type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        call.span,
                        format!("cannot infer type argument `{param_name}` for `{name}`"),
                    );
                    return None;
                }
            }
        }

        // Argument types must match the (instantiated) parameter types.
        for (param, arg) in sig.params.iter().zip(&args) {
            let expected = self.instantiate_ty(param.ty, &type_args);
            if !self.types_equal(expected, arg.ty) {
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
        }
        let ty = self.instantiate_ty(sig.return_ty, &type_args);

        // Every successful generic call (including calls inside generic
        // function bodies, whose type arguments may still mention
        // `Type::Param`) requests an instantiation; mir-lower
        // materializes them.
        if !type_args.is_empty() {
            self.record_instantiation(function, type_args.clone());
        }

        Some(hir::Expr {
            kind: ExprKind::Call {
                function,
                type_args,
                args,
            },
            ty,
            span: call.span,
        })
    }

    /// Bind type arguments by matching a parameter (or variant field)
    /// type against the argument type: `T` binds to the argument type,
    /// `Option<T>` vs `Option<Int>` recurses (so `T = Int`) — as do
    /// other enum applications — and tuples match elementwise. Anything
    /// else is left to the argument type check. Returns `false` after
    /// recording a conflict diagnostic.
    fn bind_type_args(
        &mut self,
        param_ty: TypeId,
        arg_ty: TypeId,
        bindings: &mut [Option<TypeId>],
        type_params: &[String],
        span: Span,
    ) -> bool {
        match (self.types[param_ty].clone(), self.types[arg_ty].clone()) {
            (Type::Param(index), _) => {
                let index = index as usize;
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
                                    type_params[index]
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
            (Type::Enum(param_id, param_args), Type::Enum(arg_id, arg_args))
                if param_id == arg_id && param_args.len() == arg_args.len() =>
            {
                let mut ok = true;
                for (param, arg) in param_args.iter().zip(arg_args.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
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
            _ => true,
        }
    }

    /// Struct construction with positional arguments: argument count
    /// and types must match the declared fields one by one (the field
    /// type is the argument's expected-type hint).
    fn lower_struct_init(
        &mut self,
        struct_id: hir::StructId,
        ty: TypeId,
        args: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let name = self.structs[struct_id].name.clone();
        let fields: Vec<(String, TypeId)> = self.structs[struct_id]
            .fields
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
        let mut lowered = Vec::with_capacity(args.len());
        for (arg, (field_name, field_ty)) in args.iter().zip(fields) {
            let arg = self.lower_expr(arg, sink, Some(field_ty))?;
            if !self.types_equal(field_ty, arg.ty) {
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
            lowered.push(arg);
        }
        Some(hir::Expr {
            kind: ExprKind::StructInit {
                struct_id,
                args: lowered,
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
    /// by name or a tuple element by (1-based) index.
    fn resolve_field(
        &mut self,
        receiver_ty: TypeId,
        selector: &ast::FieldSelector,
    ) -> Option<(hir::FieldRef, TypeId)> {
        match self.types[receiver_ty].clone() {
            Type::Struct(struct_id) => {
                let struct_name = self.structs[struct_id].name.clone();
                match selector {
                    ast::FieldSelector::Name(field) => {
                        let fields = &self.structs[struct_id].fields;
                        let Some(index) = fields.iter().position(|f| f.name == field.text) else {
                            self.error(
                                field.span,
                                format!("struct `{struct_name}` has no field `{}`", field.text),
                            );
                            return None;
                        };
                        let ty = fields[index].ty;
                        Some((
                            hir::FieldRef::StructField {
                                struct_id,
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
        let ty = match op {
            hir::BinOp::Add => {
                // `String + String` concatenates (docs/milestone2/
                // DESIGN.md 2.3); all other arithmetic is Int-only.
                if lhs.ty == self.string && rhs.ty == self.string {
                    self.string
                } else {
                    self.expect_int_operands(symbol, &lhs, &rhs, span)?;
                    self.int
                }
            }
            hir::BinOp::Sub | hir::BinOp::Mul | hir::BinOp::Div => {
                self.expect_int_operands(symbol, &lhs, &rhs, span)?;
                self.int
            }
            hir::BinOp::Lt | hir::BinOp::Le | hir::BinOp::Gt | hir::BinOp::Ge => {
                self.expect_int_operands(symbol, &lhs, &rhs, span)?;
                self.boolean
            }
            hir::BinOp::Eq | hir::BinOp::Ne => {
                // Every type supports structural equality (including
                // type parameters and enums — `== None` relies on
                // this); the two sides just have to agree. The
                // expansion over enum payloads happens in MIR
                // (milestone4 DESIGN.md 3.3).
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

    /// Arithmetic and comparison operators only accept `Int` operands —
    /// in particular not type parameters: `T` is unconstrained, so no
    /// operation beyond `==` / `!=` can be proven valid at the
    /// definition site (DESIGN.md 2.2).
    fn expect_int_operands(
        &mut self,
        symbol: &str,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> Option<()> {
        if lhs.ty == self.int && rhs.ty == self.int {
            return Some(());
        }
        let lhs_ty = self.type_name(lhs.ty);
        let rhs_ty = self.type_name(rhs.ty);
        self.error(
            span,
            format!("operator `{symbol}` requires Int operands, found {lhs_ty} and {rhs_ty}"),
        );
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
    Unmatched,
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
        ast::BinOp::And => (hir::BinOp::And, "&&"),
        ast::BinOp::Or => (hir::BinOp::Or, "||"),
    }
}
