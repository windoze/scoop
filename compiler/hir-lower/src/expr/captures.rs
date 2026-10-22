use super::*;
use crate::CaptureKey;
mod materialize;

impl Lowerer {
    /// Snapshot the bindings visible at a nested callable creation point.
    /// Bindings inherited from the enclosing callable are represented as
    /// transitive capture reads; locals declared by that callable override
    /// them according to ordinary lexical shadowing.
    pub(crate) fn capture_environment(
        &self,
    ) -> std::collections::HashMap<CaptureKey, AvailableCapture> {
        let mut available: std::collections::HashMap<CaptureKey, AvailableCapture> = self
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
            .unwrap_or_else(|| self.local_type_capture_environment());
        let declaration_depth = self.capture_contexts.len();
        for (name, &(parameter, ty, binding)) in &self.constructor_params_in_scope {
            available.insert(
                CaptureKey::Name(name.clone()),
                AvailableCapture {
                    binding,
                    ty,
                    mutable: false,
                    source: CaptureSource::ConstructorParam(parameter),
                    declaration_depth,
                },
            );
        }
        for (name, local) in self.scopes.visible() {
            let local_def = &self.locals[local];
            available.insert(
                CaptureKey::Name(name),
                AvailableCapture {
                    binding: local_def.binding,
                    ty: local_def.ty,
                    mutable: local_def.mutable,
                    source: CaptureSource::Local(local),
                    declaration_depth,
                },
            );
        }
        for receiver in self.implicit_receivers() {
            let Some((local, _)) = self
                .locals
                .iter()
                .find(|(_, local)| local.binding == receiver.binding)
            else {
                continue;
            };
            let binding = receiver.binding;
            available.insert(
                CaptureKey::Receiver(binding),
                AvailableCapture {
                    binding,
                    ty: receiver.ty,
                    mutable: false,
                    source: CaptureSource::Local(local),
                    declaration_depth,
                },
            );
        }
        for (local, definition) in self.locals.iter() {
            available
                .entry(CaptureKey::Binding(definition.binding))
                .or_insert_with(|| AvailableCapture {
                    binding: definition.binding,
                    ty: definition.ty,
                    mutable: definition.mutable,
                    source: CaptureSource::Local(local),
                    declaration_depth,
                });
        }
        for capture in available.values().cloned().collect::<Vec<_>>() {
            available
                .entry(CaptureKey::Binding(capture.binding))
                .or_insert(capture);
        }
        available
    }

    pub(crate) fn register_capture_at(
        &mut self,
        context_index: usize,
        name: &str,
        available: AvailableCapture,
        first_use_span: Span,
    ) -> Option<()> {
        if let CaptureSource::ClassValue {
            owner,
            key,
            environment,
        } = &available.source
        {
            self.register_class_capture(*owner, key, environment.is_some(), first_use_span)?;
        }
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
            self.register_capture_at(parent_index, name, parent_available, first_use_span)?;
        }
        let context = &mut self.capture_contexts[context_index];
        if context.by_binding.contains_key(&available.binding) {
            return Some(());
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
        Some(())
    }

    pub(crate) fn lower_capture(&mut self, name: &ast::Ident) -> Option<hir::Expr> {
        let Some(context_index) = self.capture_contexts.len().checked_sub(1) else {
            return self.lower_local_type_capture(&CaptureKey::Name(name.text.clone()), name.span);
        };
        let available = self.capture_contexts[context_index]
            .available
            .get(&CaptureKey::Name(name.text.clone()))?
            .clone();
        self.require_initialized_condition_binding(available.binding, &name.text, name.span)?;
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
        self.register_capture_at(context_index, &name.text, available.clone(), name.span)?;
        self.observe_receiver_capture(available.binding, name.span);
        self.observe_local_use(available.binding, name.span, hir::EditingLocalRole::Read);
        Some(hir::Expr {
            kind: ExprKind::Capture(available.binding),
            ty: available.ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    pub(crate) fn lower_capture_binding(
        &mut self,
        binding: hir::BindingId,
        fallback_name: &str,
        span: Span,
    ) -> Option<hir::Expr> {
        let Some(context_index) = self.capture_contexts.len().checked_sub(1) else {
            return self.local_type_capture_binding(binding, span);
        };
        let available = self.capture_contexts[context_index]
            .available
            .iter()
            .filter(|(_, candidate)| candidate.binding == binding)
            .min_by_key(|(key, _)| *key)
            .map(|(key, candidate)| (key.clone(), candidate.clone()))?;
        let (key, available) = available;
        let name = match key {
            CaptureKey::Name(name) => name,
            CaptureKey::Receiver(_) => "this".to_owned(),
            CaptureKey::Binding(_) => fallback_name.to_owned(),
        };
        self.require_initialized_condition_binding(binding, fallback_name, span)?;
        if available.mutable {
            self.error(
                span,
                format!(
                    "cannot capture mutable local `{fallback_name}`; bind its current value to a `val` snapshot or capture explicit reference state"
                ),
            );
            return None;
        }
        self.register_capture_at(context_index, &name, available.clone(), span)?;
        self.observe_receiver_capture(available.binding, span);
        Some(hir::Expr {
            kind: ExprKind::Capture(available.binding),
            ty: available.ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(crate) fn available_capture(&self, name: &str) -> Option<AvailableCapture> {
        match self.capture_contexts.last() {
            Some(context) => context
                .available
                .get(&CaptureKey::Name(name.to_owned()))
                .cloned(),
            None => self.local_type_named_capture(name),
        }
    }

    pub(crate) fn finish_current_captures(
        &mut self,
        creation: hir::ExpressionOrigin,
    ) -> Vec<hir::Capture> {
        let provider = self.current_intrinsic_provider();
        let file = u32::try_from(self.current_file).expect("source file index exceeds u32");
        let source_context = self.source_context_for_current_file();
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
        let captures = std::mem::take(&mut context.captures);
        captures
            .into_iter()
            .map(|capture| {
                let kind = match capture.source {
                    CaptureSource::Local(local) => ExprKind::Local(local),
                    CaptureSource::Capture(binding) => ExprKind::Capture(binding),
                    CaptureSource::ConstructorParam(parameter) => {
                        ExprKind::ConstructorParam(parameter)
                    }
                    CaptureSource::ClassValue {
                        owner,
                        key,
                        environment,
                    } => {
                        self.class_capture_read(
                            owner,
                            &key,
                            environment.map(|value| *value),
                            capture.first_use_span,
                        )
                        .expect("a registered class capture has an environment")
                        .kind
                    }
                };
                let source = hir::Expr {
                    kind,
                    ty: capture.ty,
                    span: capture.first_use_span,
                    origin: hir::ExpressionOrigin::Definition(hir::DefinitionOrigin {
                        provider,
                        file,
                        span: capture.first_use_span,
                        context: source_context,
                    })
                    .instantiate(creation.concrete().evaluation),
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
}
