//! Shared value-call and extension-invoke partitions for named calls.

use super::*;

impl Lowerer {
    pub(super) fn probe_named_value_member(
        &self,
        property: &SuccessfulExprLayer,
        call: CallSite<'_>,
        expected: Option<TypeId>,
    ) -> PropertyExtensionInvokeOutcome {
        match property.state.probe_property_member_invoke_partition(
            property.expression.clone(),
            call,
            expected,
            false,
        ) {
            PropertyExtensionInvokeOutcome::Resolved(mut layer) => {
                let mut setup = property.sink.clone();
                setup.append(&mut layer.sink);
                layer.sink = setup;
                PropertyExtensionInvokeOutcome::Resolved(layer)
            }
            outcome => outcome,
        }
    }

    pub(super) fn probe_named_extension_values(
        &self,
        properties: &[NamedValueLayer],
        call: CallSite<'_>,
        expected: Option<TypeId>,
        rank: usize,
        real_member: bool,
        layer_name: &str,
    ) -> PropertyExtensionInvokeOutcome {
        let mut inputs = Vec::new();
        for property in properties {
            let mut candidates = Vec::new();
            for layer in property
                .read
                .state
                .named_executable_extension_operator_layers(hir::OperatorKind::Invoke)
            {
                let invoke_rank = layer.kind.call_rank();
                let effective_rank = if real_member {
                    invoke_rank
                } else {
                    property.rank.max(invoke_rank)
                };
                if effective_rank != rank {
                    continue;
                }
                if !layer.unavailable.is_empty() {
                    let mut failure = property.read.state.clone();
                    failure.report_unavailable_lookup(
                        &ast::Ident {
                            text: "invoke".to_owned(),
                            span: call.span,
                        },
                        &layer.unavailable,
                    );
                    return PropertyExtensionInvokeOutcome::Failed(failure);
                }
                candidates.extend(layer.candidates);
            }
            if !candidates.is_empty() {
                inputs.push(PropertyExtensionInvokeInput::new(
                    property.origin,
                    property.read.clone(),
                    candidates,
                ));
            }
        }
        self.probe_property_extension_invoke_partition(inputs, call, expected, layer_name)
    }

    pub(super) fn finish_property_partition(
        &mut self,
        outcome: PropertyExtensionInvokeOutcome,
        sink: &mut Vec<hir::Statement>,
        first_failure: &mut Option<Box<Lowerer>>,
    ) -> Result<Option<hir::Expr>, ()> {
        match outcome {
            PropertyExtensionInvokeOutcome::Resolved(layer) => {
                Ok(Some(self.commit_expr_layer(layer, sink)))
            }
            PropertyExtensionInvokeOutcome::Blocked => Err(()),
            PropertyExtensionInvokeOutcome::Failed(failure) => {
                self.commit_layer_diagnostics(&failure);
                Err(())
            }
            PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                if let Some(failure) = failure {
                    first_failure.get_or_insert(failure);
                }
                Ok(None)
            }
        }
    }
}
