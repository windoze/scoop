//! Closure templates capture canonical local values; field reads happen at creation.
use super::*;

impl Lowerer {
    pub(crate) fn materialize_capture_sources(
        &mut self,
        captures: &mut [hir::Capture],
        sink: &mut Vec<hir::Statement>,
    ) {
        for capture in captures {
            if !matches!(
                capture.source.kind,
                hir::ExprKind::Local(_)
                    | hir::ExprKind::ConstructorParam(_)
                    | hir::ExprKind::Capture(_)
            ) {
                capture.source = self.materialize_temporary(
                    format!("$capture.{}", capture.name),
                    capture.source.clone(),
                    capture.source.span,
                    sink,
                );
            }
        }
    }

    pub(crate) fn materialize_callable_captures(
        &mut self,
        expression: &hir::Expr,
        sink: &mut Vec<hir::Statement>,
    ) {
        if let hir::ExprKind::FunctionCoercion { source, .. } = &expression.kind {
            self.materialize_callable_captures(source, sink);
            return;
        }
        let mut captures = match expression.kind {
            hir::ExprKind::Lambda(id) => std::mem::take(&mut self.lambdas[id].captures),
            hir::ExprKind::AnonymousFunction(id) => {
                std::mem::take(&mut self.anonymous_functions[id].captures)
            }
            hir::ExprKind::CallableReference(id) => {
                std::mem::take(&mut self.callable_references[id].captures)
            }
            _ => return,
        };
        self.materialize_capture_sources(&mut captures, sink);
        match expression.kind {
            hir::ExprKind::Lambda(id) => self.lambdas[id].captures = captures,
            hir::ExprKind::AnonymousFunction(id) => {
                self.anonymous_functions[id].captures = captures
            }
            hir::ExprKind::CallableReference(id) => {
                self.callable_references[id].captures = captures
            }
            _ => unreachable!(),
        }
    }
}
