//! Exact native callback references use the ordinary declaration layers.

use super::*;
use crate::call_resolution::constraints::CallableCategory;
use crate::imports::lookup::calls::{NamedCallBinding, NamedCallOrigin, NamedCallTarget};

mod declarations;
use declarations::{NativeReferenceDeclaration, NativeReferenceFailure};

struct ApplicableNativeReference {
    state: Box<Lowerer>,
    declaration: NativeReferenceDeclaration,
}

impl Lowerer {
    pub(super) fn lower_native_function_reference(
        &mut self,
        name: &ast::Ident,
        span: Span,
        expected: TypeId,
    ) -> Option<hir::Expr> {
        if self.scopes.lookup(&name.text).is_some()
            || !self.local_function_scopes.lookup(&name.text).is_empty()
        {
            self.error(
                span,
                "a native `FunPtr` address cannot target a local function or function value"
                    .to_string(),
            );
            return None;
        }
        let mut first_failure = None;
        for layer in self.named_callable_reference_layers(&name.text) {
            let mut matching = Vec::new();
            let mut failures = Vec::new();
            let mut suppressed = !layer.suppressed_callables.is_empty();
            for candidate in &layer.candidates {
                if let NamedCallTarget::Function(function) = candidate.target
                    && self.declaration_surface.rejects_function(function)
                {
                    suppressed = true;
                    continue;
                }
                let mut state = self.clone();
                let declaration =
                    match NativeReferenceDeclaration::resolve(&mut state, candidate, &name.text) {
                        Ok(declaration) => declaration,
                        Err(failure) => {
                            failures.push(failure);
                            continue;
                        }
                    };
                match state.check_concrete_callable_signature(
                    CallableCategory::Native,
                    declaration.effects.is_suspend,
                    &declaration.parameters,
                    declaration.return_type,
                    expected,
                ) {
                    Ok(()) => matching.push(ApplicableNativeReference {
                        state: Box::new(state),
                        declaration,
                    }),
                    Err(constraint) => failures.push(declaration.failure(&state, &constraint)),
                }
            }
            if matching.len() == 1 {
                let selected = matching
                    .pop()
                    .expect("one native reference candidate exists");
                *self = *selected.state;
                if selected.declaration.effects.attributes.safety == hir::Safety::Unsafe {
                    self.require_unsafe_operation(span, "taking the address of an unsafe callback");
                }
                let target = match selected.declaration.commit(self) {
                    Ok(target) => target,
                    Err(message) => {
                        self.error(span, message);
                        return None;
                    }
                };
                return Some(hir::Expr {
                    kind: ExprKind::FunctionAddress(target),
                    ty: expected,
                    span,
                    origin: self.expression_origin(span),
                });
            }
            if matching.len() > 1 {
                let layer = matching[0].declaration.layer;
                let traces = matching
                    .iter()
                    .map(|candidate| {
                        format!(
                            "  - {} — exactly matches the expected FunPtr signature",
                            candidate.declaration.display
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.error(
                    span,
                    format!(
                        "native function reference `::{}` is ambiguous in {layer} layer:\n{traces}",
                        name.text
                    ),
                );
                return None;
            }
            if suppressed {
                if !failures.is_empty() {
                    self.native_reference_failures(name, &failures, span);
                }
                return None;
            }
            if !failures.is_empty() {
                first_failure.get_or_insert(failures);
            }
        }
        self.native_reference_failures(name, first_failure.as_deref().unwrap_or_default(), span);
        None
    }

    fn native_reference_failures(
        &mut self,
        name: &ast::Ident,
        failures: &[NativeReferenceFailure],
        span: Span,
    ) {
        let Some(first) = failures.first() else {
            self.error(span, format!("no eligible `@NoGC` top-level function `::{}` exactly matches the expected FunPtr signature", name.text));
            return;
        };
        let layer = first.layer;
        let traces = failures
            .iter()
            .map(|failure| format!("  - {} — {}", failure.display, failure.reason))
            .collect::<Vec<_>>()
            .join("\n");
        self.error(span, format!("no applicable candidate for native function reference `::{}` in {layer} layer:\n{traces}", name.text));
    }
}
