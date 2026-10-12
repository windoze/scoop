use super::*;
use crate::body_transform::{BodyTransform, walk_expression};

/// Complete recursive calls once the local function's capture set is known.
pub(super) fn patch_local_function_calls(
    lowerer: &mut Lowerer,
    statements: &mut [hir::Statement],
    target: hir::LocalFunctionId,
    captures: &[hir::Capture],
) {
    LocalFunctionCallPatcher { target, captures }.statements(lowerer, statements);
}

struct LocalFunctionCallPatcher<'a> {
    target: hir::LocalFunctionId,
    captures: &'a [hir::Capture],
}
impl BodyTransform for LocalFunctionCallPatcher<'_> {
    fn expression(&mut self, lowerer: &mut Lowerer, expr: &mut hir::Expr) {
        walk_expression(self, lowerer, expr);
        let target = self.target;
        let target_captures = self.captures;
        let span = expr.span;
        let origin = expr.origin;
        match &mut expr.kind {
            hir::ExprKind::CallableReference(id) => {
                let mut reference = lowerer.callable_references[*id].clone();
                if let hir::CallableReferenceTarget::Local {
                    callee: hir::CallableTarget::Local(callee),
                    ..
                } = reference.target
                    && lowerer.callable_function_id(callee)
                        == lowerer.local_functions[target].source_function()
                {
                    // A self reference preserves its lexical owner arguments;
                    // captured outer bindings have the same types in this body.
                    reference.captures = target_captures
                        .iter()
                        .map(|capture| hir::Capture {
                            source: hir::Expr {
                                kind: hir::ExprKind::Capture(capture.binding),
                                ty: capture.ty,
                                span,
                                origin,
                            },
                            ..capture.clone()
                        })
                        .collect();
                }

                lowerer.callable_references[*id] = reference;
            }
            hir::ExprKind::Call {
                callee: hir::CallableTarget::Local(callee),
                args,
                ..
            } => {
                let function = lowerer.callable_function_id(*callee);
                if function == lowerer.local_functions[target].source_function() {
                    let parameter_count = lowerer.signatures[&function].params.len();
                    let previous_count = args
                        .len()
                        .checked_sub(parameter_count)
                        .expect("a resolved recursive call contains every source parameter");
                    if previous_count != target_captures.len() {
                        args.drain(..previous_count);
                        let mut complete = target_captures
                            .iter()
                            .map(|capture| hir::Expr {
                                kind: hir::ExprKind::Capture(capture.binding),
                                ty: capture.ty,
                                span,
                                origin,
                            })
                            .collect::<Vec<_>>();
                        complete.append(args);
                        *args = complete;
                    }
                }
            }
            _ => {}
        }
    }
}
