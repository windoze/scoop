use super::*;

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

    pub(super) fn lower_capture(&mut self, name: &ast::Ident) -> Option<hir::Expr> {
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

    pub(crate) fn lower_capture_binding(
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
}
