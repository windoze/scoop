//! Frozen declaration-side callable membership for body lookup.
//!
//! Signature validation builds the rejected set off to the side and publishes
//! it in one step.  Rejected declarations retain their arena identities so
//! their own bodies and unrelated declarations can still be diagnosed, but no
//! body lookup may probe one as a semantic overload candidate.

use std::collections::{HashMap, HashSet};

use scoop_ast as ast;
use scoop_hir as hir;

#[derive(Clone, Default)]
pub(crate) struct DeclarationSurface {
    state: DeclarationSurfaceState,
    unavailable_signatures: HashMap<hir::FunctionId, UnavailableSignature>,
}

#[derive(Clone, Copy)]
pub(crate) struct UnavailableSignature {
    pub(crate) kind: ast::DeclarationHeaderKind,
    pub(crate) name_span: ast::Span,
}

#[derive(Clone, Default)]
enum DeclarationSurfaceState {
    #[default]
    Collecting,
    Frozen {
        rejected_functions: HashSet<hir::FunctionId>,
    },
}

impl DeclarationSurface {
    pub(crate) fn mark_signature_unavailable(
        &mut self,
        function: hir::FunctionId,
        header: ast::FunctionHeaderRef<'_>,
    ) {
        self.unavailable_signatures.insert(
            function,
            UnavailableSignature {
                kind: ast::DeclarationHeaderKind::Function {
                    is_extension: header.receiver_ty.is_some(),
                    is_operator: header.operator.is_some(),
                    is_infix: header.infix.is_some(),
                },
                name_span: header.name.span,
            },
        );
    }

    pub(crate) fn unavailable_signature(
        &self,
        function: hir::FunctionId,
    ) -> Option<UnavailableSignature> {
        self.unavailable_signatures.get(&function).copied()
    }

    pub(crate) fn freeze(&mut self, rejected_functions: HashSet<hir::FunctionId>) {
        match &mut self.state {
            DeclarationSurfaceState::Collecting => {
                self.state = DeclarationSurfaceState::Frozen { rejected_functions };
            }
            DeclarationSurfaceState::Frozen {
                rejected_functions: previous,
            } => {
                previous.extend(rejected_functions);
            }
        }
    }

    pub(crate) fn is_frozen(&self) -> bool {
        matches!(self.state, DeclarationSurfaceState::Frozen { .. })
    }

    pub(crate) fn rejects_function(&self, function: hir::FunctionId) -> bool {
        match &self.state {
            DeclarationSurfaceState::Collecting => false,
            DeclarationSurfaceState::Frozen { rejected_functions } => {
                rejected_functions.contains(&function)
            }
        }
    }
}

impl crate::Lowerer {
    pub(crate) fn function_matches_required_modifiers(
        &self,
        function: hir::FunctionId,
        required: crate::expr::RequiredCallableModifiers,
    ) -> bool {
        let Some(unavailable) = self.declaration_surface.unavailable_signature(function) else {
            return Self::matches_required_modifiers(
                self.signatures[&function].modifiers,
                required,
            );
        };
        let ast::DeclarationHeaderKind::Function {
            is_operator,
            is_infix,
            ..
        } = unavailable.kind
        else {
            unreachable!("source function signatures retain their function kind")
        };
        use crate::callable_modifiers::{CallableOperatorKind, callable_operator_kind};
        let role = is_operator
            .then(|| callable_operator_kind(self.source_function_name(function)).ok())
            .flatten();
        required
            .operator
            .is_none_or(|operator| role == Some(CallableOperatorKind::Ordinary(operator)))
            && required.property_delegate_operator.is_none_or(|operator| {
                role == Some(CallableOperatorKind::PropertyDelegate(operator))
            })
            && (!required.infix || is_infix)
    }

    pub(crate) fn report_unavailable_signatures(
        &mut self,
        name: &str,
        span: ast::Span,
        functions: impl IntoIterator<Item = hir::FunctionId>,
    ) -> bool {
        let mut failures = functions
            .into_iter()
            .filter_map(|function| {
                self.declaration_surface
                    .unavailable_signature(function)
                    .map(|failure| (function, failure))
            })
            .collect::<Vec<_>>();
        if failures.is_empty() {
            return false;
        }
        failures.sort_by_key(|(function, _)| function.into_raw().into_u32());
        failures.dedup_by_key(|(function, _)| *function);
        let mut diagnostic = ast::Diagnostic::at_file(
            self.current_file,
            span,
            format!("signature of `{name}` is unavailable"),
        );
        diagnostic
            .notes
            .extend(failures.into_iter().map(|(function, failure)| {
                ast::DiagnosticNote::at(
                    self.function_files[&function],
                    failure.name_span,
                    "this declaration's signature could not be resolved",
                )
            }));
        self.diagnostics.push(diagnostic);
        true
    }
}
