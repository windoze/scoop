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
        let inputs = properties
            .iter()
            .filter_map(|property| {
                let candidates = property
                    .read
                    .state
                    .named_executable_extension_call_layers("invoke")
                    .into_iter()
                    .filter(|layer| {
                        let invoke_rank = layer.kind.call_rank();
                        if real_member {
                            invoke_rank == rank
                        } else {
                            property.rank.max(invoke_rank) == rank
                        }
                    })
                    .flat_map(|layer| layer.candidates)
                    .filter(|target| {
                        property.read.state.extension_call_target_matches_required(
                            target,
                            RequiredCallableModifiers {
                                operator: Some(hir::OperatorKind::Invoke),
                                ..Default::default()
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                (!candidates.is_empty()).then(|| {
                    PropertyExtensionInvokeInput::new(
                        property.origin,
                        property.read.clone(),
                        candidates,
                    )
                })
            })
            .collect();
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
                self.commit_layer_diagnostics(*failure);
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
