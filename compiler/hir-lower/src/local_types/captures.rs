use super::*;
use crate::{AvailableCapture, CaptureKey, CaptureSource};

impl Lowerer {
    pub(crate) fn local_type_named_capture(&self, name: &str) -> Option<AvailableCapture> {
        let capture = self.local_type_available(&CaptureKey::Name(name.to_owned()))?;
        if self
            .current_this
            .is_some_and(|(_, ty)| self.receiver_has_property(ty, name))
            || (self.initialization_context.is_some()
                && self.clone().initializing_receiver_has_field(name))
        {
            return None;
        }
        Some(capture)
    }

    pub(crate) fn local_type_available(&self, key: &CaptureKey) -> Option<AvailableCapture> {
        let scope = self.local_type_capture_scope.as_ref()?;
        self.local_types[&scope.owner].available.get(key).cloned()
    }

    pub(crate) fn local_type_capture_environment(
        &self,
    ) -> std::collections::HashMap<CaptureKey, AvailableCapture> {
        let Some(scope) = &self.local_type_capture_scope else {
            return Default::default();
        };
        let mut available = self.local_types[&scope.owner].available.clone();
        available.retain(|key, _| match key {
            CaptureKey::Name(name) => self.local_type_named_capture(name).is_some(),
            _ => true,
        });
        for (key, capture) in &mut available {
            capture.source = CaptureSource::ClassValue {
                owner: scope.owner,
                key: key.clone(),
                environment: scope
                    .environment
                    .as_ref()
                    .map(|(value, _)| Box::new(value.clone())),
            };
        }
        if let Some((value, binding)) = &scope.environment {
            let source = match value.kind {
                hir::ExprKind::Local(local) => CaptureSource::Local(local),
                hir::ExprKind::ConstructorParam(parameter) => {
                    CaptureSource::ConstructorParam(parameter)
                }
                _ => unreachable!("a captured environment is a saved input"),
            };
            available.insert(
                CaptureKey::Receiver(*binding),
                AvailableCapture {
                    binding: *binding,
                    ty: value.ty,
                    mutable: false,
                    source,
                    declaration_depth: self.capture_contexts.len(),
                },
            );
        }
        available
    }

    pub(crate) fn register_class_capture(
        &mut self,
        owner: Owner,
        key: &CaptureKey,
        has_environment: bool,
        span: ast::Span,
    ) -> Option<usize> {
        let name = match key {
            CaptureKey::Name(name) => name.as_str(),
            CaptureKey::Receiver(_) => "this",
            CaptureKey::Binding(_) => "captured value",
        };
        if !has_environment {
            self.error(span, format!("this type cannot capture runtime binding `{name}`; only a local class instance may capture values"));
            return None;
        }
        let available = self.local_types[&owner].available[key].clone();
        if available.mutable {
            self.error(span, format!("cannot capture mutable local `{name}`; bind its current value to a `val` snapshot or capture explicit reference state"));
            return None;
        }
        if let CaptureSource::ClassValue {
            owner,
            key,
            environment,
        } = available.source
        {
            self.register_class_capture(owner, &key, environment.is_some(), span)?;
        }
        let local = self.local_types.get_mut(&owner).unwrap();
        let slot = local
            .captures
            .iter()
            .position(|key| local.available[key].binding == available.binding);
        Some(slot.unwrap_or_else(|| {
            let slot = local.captures.len();
            local.captures.push(key.clone());
            slot
        }))
    }

    pub(crate) fn class_capture_read(
        &mut self,
        owner: Owner,
        key: &CaptureKey,
        environment: Option<hir::Expr>,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let index = self.register_class_capture(owner, key, environment.is_some(), span)?;
        let value = self.local_types[&owner].available[key].clone();
        Some(hir::Expr {
            kind: hir::ExprKind::FieldAccess {
                receiver: Box::new(environment.expect("a class capture has an environment")),
                field: hir::FieldRef::TupleIndex(
                    u32::try_from(index).expect("capture index fits u32"),
                ),
            },
            ty: value.ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(crate) fn lower_local_type_capture(
        &mut self,
        key: &CaptureKey,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let scope = self.local_type_capture_scope.clone()?;
        self.class_capture_read(
            scope.owner,
            key,
            scope.environment.map(|(value, _)| value),
            span,
        )
    }

    pub(crate) fn local_constructor_environment(
        &mut self,
        class: hir::ClassId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let owner = Owner::Class(class);
        if let Some(scope) = &self.local_type_capture_scope
            && scope.owner == owner
            && let Some((value, binding)) = scope.environment.clone()
        {
            if self.capture_contexts.is_empty() {
                return Some(value);
            }
            return self.lower_capture_binding(binding, "$captures", span);
        }
        let local = self.local_types[&owner].clone();
        let mut values = Vec::with_capacity(local.captures.len());
        for key in &local.captures {
            let available = local.available[key].clone();
            let value = self.read_local_constructor_capture(&available, key, span)?;
            values.push(value);
        }
        if values.is_empty() {
            return Some(self.unit_expression(span));
        }
        let ty = self.intern_type(hir::Type::Tuple(
            values.iter().map(|value| value.ty).collect(),
        ));
        Some(hir::Expr {
            kind: hir::ExprKind::TupleLiteral(values),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
