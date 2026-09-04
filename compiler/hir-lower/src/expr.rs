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
//! Loop conditions and `when` guards retain their setup explicitly so
//! it executes at each condition/arm attempt rather than being hoisted.
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
use crate::types::ArrayKind;
use crate::{
    AvailableCapture, CaptureContext, CaptureSource, ForbiddenSuspendContext, Lowerer,
    PendingCapture, ReturnInference, SuspensionContext,
};

mod callable_literals;
mod callable_references;
mod callables;
mod callbacks;
mod generic_calls;
mod generic_inference;

mod aggregates;
mod analysis;
mod captures;
mod constructors;
mod fields;
mod members;
mod names;
mod operators;
mod support;
mod type_checks;

use analysis::*;
use support::*;

pub(crate) struct NominalArguments {
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) argument_sinks: Vec<Vec<hir::Statement>>,
    pub(crate) type_args: Vec<TypeId>,
}

pub(crate) struct NominalArgumentInput<'a> {
    pub(crate) view: &'a crate::call_resolution::candidates::NominalConstructorView,
    pub(crate) argument_map: &'a crate::call_resolution::arguments::CandidateArgumentMap,
    pub(crate) expressions: &'a [ast::CallArgument],
    pub(crate) explicit_type_args: &'a [ResolvedCallTypeArgument],
    pub(crate) expected_type_args: Option<&'a [TypeId]>,
    pub(crate) span: Span,
}

#[derive(Clone)]
pub(crate) struct QualifiedInterfaceProperty {
    pub(crate) property: hir::PropertyId,
    pub(crate) owner: hir::InterfaceApplicationId,
    pub(crate) receiver: hir::Expr,
    pub(crate) ty: TypeId,
}

#[derive(Clone, Copy)]
pub(crate) struct CallSite<'a> {
    pub(crate) type_args: &'a [ast::CallTypeArgument],
    pub(crate) args: &'a [ast::CallArgument],
    pub(crate) span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedCallTypeArgument {
    Explicit { ty: TypeId, span: Span },
    Infer { span: Span },
}

impl ResolvedCallTypeArgument {
    pub(crate) fn span(self) -> Span {
        match self {
            Self::Explicit { span, .. } | Self::Infer { span } => span,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct RequiredCallableModifiers {
    pub(crate) operator: Option<hir::OperatorKind>,
    pub(crate) infix: bool,
}

#[derive(Clone, Copy)]
enum ReferenceExtensionMode {
    Exclude,
    IncludeUnbound,
    Bound(TypeId),
}

#[derive(Clone, Copy)]
struct ReferenceResolutionContext<'a> {
    expected: Option<&'a (TypeId, hir::FunctionType)>,
    name: &'a str,
    display: &'a str,
    span: Span,
    extension_mode: ReferenceExtensionMode,
}

struct ResolvedReference {
    callable: hir::Callable,
    source: crate::CallableCandidateSource,
    type_args: Vec<TypeId>,
    ty: TypeId,
}

struct SuccessfulExprLayer {
    state: Box<Lowerer>,
    expression: hir::Expr,
    sink: Vec<hir::Statement>,
}

impl Lowerer {
    fn probe_expr_layer(
        &self,
        lower: impl FnOnce(&mut Lowerer, &mut Vec<hir::Statement>) -> Option<hir::Expr>,
    ) -> Result<SuccessfulExprLayer, Box<Lowerer>> {
        let mut state = self.clone();
        let mut sink = Vec::new();
        match lower(&mut state, &mut sink) {
            Some(expression) => Ok(SuccessfulExprLayer {
                state: Box::new(state),
                expression,
                sink,
            }),
            None => Err(Box::new(state)),
        }
    }

    fn commit_expr_layer(
        &mut self,
        layer: SuccessfulExprLayer,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        *self = *layer.state;
        sink.extend(layer.sink);
        layer.expression
    }

    pub(crate) fn commit_layer_diagnostics(&mut self, failed: Lowerer) {
        let baseline = self.diagnostics.len();
        debug_assert!(failed.diagnostics.len() > baseline);
        self.diagnostics
            .extend(failed.diagnostics.into_iter().skip(baseline));
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
                origin: self.expression_origin(*span),
            }),
            ast::Expr::IntLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::IntLiteral(*value),
                ty: self.int,
                span: *span,
                origin: self.expression_origin(*span),
            }),
            ast::Expr::BoolLiteral { value, span } => Some(hir::Expr {
                kind: ExprKind::BoolLiteral(*value),
                ty: self.boolean,
                span: *span,
                origin: self.expression_origin(*span),
            }),
            ast::Expr::UnitLiteral { span } => Some(hir::Expr {
                kind: ExprKind::UnitLiteral,
                ty: self.unit,
                span: *span,
                origin: self.expression_origin(*span),
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
            ast::Expr::Var(name) => self.lower_var(name, sink, expected),
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
            ast::Expr::FieldAccess(access) if access.navigation == ast::Navigation::Safe => {
                self.lower_safe_field_access(access, sink)
            }
            ast::Expr::FieldAccess(access) => self.lower_field_access(access, sink, expected),
            ast::Expr::Call(call) => self.lower_call(call, sink, expected),
            ast::Expr::Invoke {
                callee,
                type_args,
                args,
                span,
            } => {
                let callee = self.lower_expr(callee, sink, None)?;
                self.lower_value_invoke(
                    callee,
                    CallSite {
                        type_args,
                        args,
                        span: *span,
                    },
                    sink,
                    expected,
                    false,
                )
            }
            ast::Expr::InfixCall {
                lhs,
                target,
                rhs,
                span,
            } => self.lower_infix_call(lhs, target, rhs, *span, sink, expected),
            ast::Expr::Binary { op, lhs, rhs, span } => {
                self.lower_binary(*op, lhs, rhs, *span, sink)
            }
            ast::Expr::Unary { op, operand, span } => self.lower_unary(*op, operand, *span, sink),
            ast::Expr::Update {
                place,
                op,
                notation,
                span,
            } => self.lower_update(place, *op, *notation, *span, sink),
            ast::Expr::NullAssert { operand, span } => self.lower_null_assert(operand, *span, sink),
            ast::Expr::Elvis { lhs, rhs, span } => self.lower_elvis(lhs, rhs, *span, sink),
            ast::Expr::This { span } => self.lower_this(*span),
            ast::Expr::MethodCall {
                receiver,
                name,
                navigation,
                type_args,
                args,
                span,
            } => {
                let call = CallSite {
                    type_args,
                    args,
                    span: *span,
                };
                match navigation {
                    ast::Navigation::Direct => {
                        self.lower_method_call(receiver, name, call, sink, expected)
                    }
                    ast::Navigation::Safe => {
                        self.lower_safe_method_call(receiver, name, call, sink, expected)
                    }
                }
            }
            ast::Expr::SuperMethodCall {
                name,
                type_args,
                args,
                span,
                ..
            } => self.lower_super_method_call(
                name,
                CallSite {
                    type_args,
                    args,
                    span: *span,
                },
                sink,
                expected,
            ),
            ast::Expr::QualifiedInterfaceSuperAccess {
                qualifier,
                name,
                span,
                ..
            } => self.lower_qualified_interface_super_property_read(qualifier, name, *span),
            ast::Expr::QualifiedInterfaceSuperMethodCall {
                qualifier,
                name,
                type_args,
                args,
                span,
                ..
            } => self.lower_qualified_interface_super_method_call(
                qualifier,
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
                indices,
                span,
            } => self.lower_index_read(receiver, indices.as_slice(), *span, sink),
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
