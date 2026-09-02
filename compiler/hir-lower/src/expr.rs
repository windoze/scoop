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

mod callable_literals;
mod callable_references;
mod callables;
mod callbacks;
mod generic_calls;
mod generic_inference;

mod aggregates;
mod captures;
mod constructors;
mod fields;
mod members;
mod names;
mod operators;
mod type_checks;

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
