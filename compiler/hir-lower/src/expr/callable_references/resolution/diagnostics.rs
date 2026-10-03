use super::*;
use crate::call_resolution::diagnostics::{callable_layer_name, callable_source_signature};

impl ReferenceDeclaration {
    fn signature(&self, state: &Lowerer, name: &str) -> String {
        match self {
            Self::Local(declaration) => callable_source_signature(state, name, &declaration.view),
            Self::Imported(declaration) => declaration.signature(state, name),
        }
    }

    fn layer(&self, state: &Lowerer) -> &'static str {
        match self {
            Self::Local(declaration) => {
                callable_layer_name(state, std::slice::from_ref(&declaration.view))
            }
            Self::Imported(declaration) => declaration.layer(),
        }
    }
}

fn layer_name(layers: impl IntoIterator<Item = &'static str>) -> &'static str {
    let mut layers = layers.into_iter();
    let first = layers
        .next()
        .expect("a failed or ambiguous reference has candidates");
    if layers.all(|layer| layer == first) {
        first
    } else {
        "declaration candidate"
    }
}

impl Lowerer {
    pub(super) fn reference_failures_diagnostic(
        &mut self,
        context: ReferenceResolutionContext<'_>,
        failures: &[ReferenceFailure],
    ) {
        if failures.is_empty() {
            self.error(
                context.span,
                format!(
                    "no declaration candidate has a permitted form for {}",
                    context.display
                ),
            );
            return;
        }
        let layer = layer_name(failures.iter().map(|failure| failure.layer));
        let traces = failures
            .iter()
            .map(|failure| format!("  - {} — {}", failure.signature, failure.reason))
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            context.span,
            format!(
                "no applicable candidate for {} in {layer} layer:\n{traces}",
                context.display
            ),
        );
    }

    pub(super) fn reference_ambiguity_diagnostic(
        &mut self,
        context: ReferenceResolutionContext<'_>,
        applicable: &[&ApplicableReference],
    ) {
        let layer = layer_name(
            applicable
                .iter()
                .map(|candidate| candidate.declaration.layer(&candidate.state)),
        );
        let reason = if context.expected.is_some() {
            "matches the expected function type"
        } else {
            "forms a complete non-generic function type"
        };
        let traces = applicable
            .iter()
            .map(|candidate| {
                format!(
                    "  - {} — {reason}",
                    candidate
                        .declaration
                        .signature(&candidate.state, context.name)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            context.span,
            format!(
                "{} is ambiguous in {layer} layer:\n{traces}",
                context.display
            ),
        );
    }
}
