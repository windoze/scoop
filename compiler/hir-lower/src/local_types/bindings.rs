//! Nominal construction reads the definition's bindings, even under shadowing.
use super::*;
use crate::{AvailableCapture, CaptureKey};

impl Lowerer {
    pub(crate) fn local_type_capture_binding(
        &mut self,
        binding: hir::BindingId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let scope = self.local_type_capture_scope.as_ref()?;
        if let Some((value, captured)) = &scope.environment
            && *captured == binding
        {
            return Some(value.clone());
        }
        let key = self.local_types[&scope.owner]
            .available
            .iter()
            .filter(|(_, available)| available.binding == binding)
            .min_by_key(|(key, _)| *key)?
            .0
            .clone();
        self.lower_local_type_capture(&key, span)
    }

    pub(super) fn read_local_constructor_capture(
        &mut self,
        available: &AvailableCapture,
        key: &CaptureKey,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let name = match key {
            CaptureKey::Name(name) => name.as_str(),
            CaptureKey::Receiver(_) => "this",
            CaptureKey::Binding(_) => "captured value",
        };
        self.require_initialized_condition_binding(available.binding, name, span)?;
        let kind = if let Some((id, _)) = self
            .locals
            .iter()
            .find(|(_, local)| local.binding == available.binding)
        {
            Some(hir::ExprKind::Local(id))
        } else {
            self.constructor_params_in_scope
                .values()
                .find(|(_, _, binding)| *binding == available.binding)
                .map(|(id, _, _)| hir::ExprKind::ConstructorParam(*id))
        };
        if let Some(kind) = kind {
            return Some(hir::Expr {
                kind,
                ty: available.ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        self.lower_capture_binding(available.binding, name, span)
    }
}
