use super::*;

impl Lowerer {
    pub(crate) fn stable_smart_cast_binding(&self, name: &str) -> Option<(hir::BindingId, TypeId)> {
        if let Some(local) = self.scopes.lookup(name) {
            let local = &self.locals[local];
            return (!local.mutable && !self.local_delegate_plans.contains_key(&local.binding))
                .then_some((local.binding, local.ty));
        }
        if let Some(capture) = self.available_capture(name) {
            return (!capture.mutable && !self.local_delegate_plans.contains_key(&capture.binding))
                .then_some((capture.binding, capture.ty));
        }
        self.constructor_params_in_scope
            .get(name)
            .map(|&(_, ty, binding)| (binding, ty))
    }

    pub(crate) fn smart_cast_type(
        &mut self,
        binding: hir::BindingId,
        declared: TypeId,
        expected: Option<TypeId>,
    ) -> TypeId {
        let Some(types) = self.smart_casts.get(&binding).cloned() else {
            return declared;
        };
        if let Some(expected) = expected {
            if let Some(&ty) = types.iter().find(|&&ty| self.is_subtype(ty, expected)) {
                return ty;
            }
            if self.is_subtype(declared, expected) {
                return declared;
            }
        }
        types.first().copied().unwrap_or(declared)
    }

    pub(crate) fn smart_cast_read(
        &mut self,
        source: hir::Expr,
        binding: hir::BindingId,
        expected: Option<TypeId>,
    ) -> hir::Expr {
        let ty = self.smart_cast_type(binding, source.ty, expected);
        self.known_type_read(source, ty)
    }

    pub(crate) fn known_type_read(&mut self, source: hir::Expr, ty: TypeId) -> hir::Expr {
        if self.types_equal(source.ty, ty) {
            return source;
        }
        if self.is_value_ty(source.ty) && self.is_ref_ty(ty) {
            return self.adapt_to(source, ty);
        }
        if self.is_value_ty(ty) {
            hir::Expr {
                ty,
                span: source.span,
                origin: source.origin,
                kind: ExprKind::Unbox(Box::new(source)),
            }
        } else {
            hir::Expr { ty, ..source }
        }
    }

    pub(crate) fn smart_cast_receiver_views(&mut self, receiver: &hir::Expr) -> Vec<hir::Expr> {
        let source = if let ExprKind::Unbox(source) = &receiver.kind {
            source.as_ref()
        } else {
            receiver
        };
        let (binding, declared) = match source.kind {
            ExprKind::Local(local) => (self.locals[local].binding, self.locals[local].ty),
            ExprKind::Capture(binding) => {
                let Some(capture) = self.capture_contexts.last().and_then(|context| {
                    context
                        .available
                        .values()
                        .find(|capture| capture.binding == binding)
                }) else {
                    return vec![receiver.clone()];
                };
                (binding, capture.ty)
            }
            ExprKind::ConstructorParam(parameter) => {
                let Some(&(_, ty, binding)) = self
                    .constructor_params_in_scope
                    .values()
                    .find(|&&(id, _, _)| id == parameter)
                else {
                    return vec![receiver.clone()];
                };
                (binding, ty)
            }
            _ => return vec![receiver.clone()],
        };
        let Some(types) = self.smart_casts.get(&binding).cloned() else {
            return vec![receiver.clone()];
        };
        let source = hir::Expr {
            ty: declared,
            ..source.clone()
        };
        let mut views = vec![receiver.clone()];
        for ty in types.into_iter().chain([declared]) {
            if !views.iter().any(|view| self.is_subtype(view.ty, ty)) {
                views.push(self.known_type_read(source.clone(), ty));
            }
        }
        views
    }
}
