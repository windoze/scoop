use scoop_ast as ast;
use scoop_hir as hir;

use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::diagnostics::nominal_source_signature;
use crate::call_resolution::specificity::NominalForwardingDeclaration;
use crate::expr::ResolvedCallTypeArgument;
use crate::expr::{NominalArgumentInput, NominalArguments};
use crate::{Lowerer, TypeId};

pub(crate) struct ResolvedNominalConstructor {
    pub(crate) source: NominalConstructorSource,
    pub(crate) type_args: Vec<TypeId>,
    pub(crate) args: Vec<hir::Expr>,
}

pub(crate) struct NominalConstructorCall<'a> {
    pub(crate) explicit_type_args: &'a [ResolvedCallTypeArgument],
    pub(crate) expected_type_args: Option<&'a [TypeId]>,
    pub(crate) arguments: &'a [ast::CallArgument],
    pub(crate) span: ast::Span,
}

struct ApplicableConstructor {
    state: Box<Lowerer>,
    source: NominalConstructorSource,
    view: NominalConstructorView,
    argument_map: CandidateArgumentMap,
    inferred: NominalArguments,
    parameter_types: Vec<TypeId>,
}

impl Lowerer {
    pub(crate) fn resolve_nominal_constructor_overload(
        &mut self,
        name: &str,
        candidates: &[NominalConstructorSource],
        call: NominalConstructorCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedNominalConstructor> {
        let NominalConstructorCall {
            explicit_type_args,
            expected_type_args,
            arguments,
            span,
        } = call;
        if candidates.is_empty() {
            self.error(span, format!("type `{name}` has no source constructor"));
            return None;
        }
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        let mut inaccessible = false;
        for &source in candidates {
            if !self.constructor_is_accessible(source) {
                inaccessible = true;
                continue;
            }
            let view = self.nominal_constructor_view(source);
            if !explicit_type_args.is_empty()
                && explicit_type_args.len() != view.owner_parameters.len()
            {
                let mut failure = self.clone();
                failure.diagnose_nominal_shape_failure(
                    &view,
                    span,
                    format!(
                        "expects {} explicit type argument(s), but {} were supplied",
                        view.owner_parameters.len(),
                        explicit_type_args.len()
                    ),
                );
                failures.push((view, Box::new(failure)));
                continue;
            }
            let argument_map = match CandidateArgumentMap::source_nominal(&view, arguments) {
                Ok(argument_map) => argument_map,
                Err(reason) => {
                    let mut failure = self.clone();
                    failure.diagnose_nominal_shape_failure(&view, span, reason.describe());
                    failures.push((view, Box::new(failure)));
                    continue;
                }
            };
            let mut state = self.clone();
            let diagnostics_before = state.diagnostics.len();
            let inferred = state.lower_nominal_arguments(NominalArgumentInput {
                view: &view,
                argument_map: &argument_map,
                expressions: arguments,
                explicit_type_args,
                expected_type_args,
                span,
            });
            let Some(inferred) = inferred else {
                failures.push((view, Box::new(state)));
                continue;
            };
            if state.diagnostics.len() != diagnostics_before {
                failures.push((view, Box::new(state)));
                continue;
            }
            let parameter_types = argument_map.forwarding_parameter_types(&view.value_parameters);
            applicable.push(ApplicableConstructor {
                state: Box::new(state),
                source,
                view,
                argument_map,
                inferred,
                parameter_types,
            });
        }

        let winner = match applicable.len() {
            0 => {
                if failures.is_empty() && inaccessible {
                    self.error(
                        span,
                        format!("constructor of type `{name}` is not accessible here"),
                    );
                    return None;
                }
                if failures.len() == 1 {
                    self.commit_layer_diagnostics(*failures.pop().expect("one failure").1);
                } else {
                    let traces = failures
                        .iter()
                        .map(|(view, state)| {
                            let reason = state
                                .diagnostics
                                .last()
                                .map(|diagnostic| diagnostic.message.as_str())
                                .unwrap_or("candidate is not applicable");
                            format!("  - {} — {reason}", nominal_source_signature(self, view))
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.error(
                        span,
                        format!(
                            "no applicable candidate for constructor `{name}` in nominal constructor candidate layer:\n{traces}"
                        ),
                    );
                }
                return None;
            }
            1 => 0,
            _ => self.most_specific_nominal_constructor(name, &applicable, span)?,
        };
        let winner = applicable.swap_remove(winner);
        *self = *winner.state;
        let type_args = winner.inferred.type_args;
        let args = self.materialize_nominal_arguments(
            crate::argument_materialization::NominalArgumentMaterialization {
                view: &winner.view,
                argument_map: &winner.argument_map,
                type_args: &type_args,
                source_args: winner.inferred.args,
                argument_sinks: winner.inferred.argument_sinks,
                call_span: span,
            },
            sink,
        );
        Some(ResolvedNominalConstructor {
            source: winner.source,
            type_args,
            args,
        })
    }

    fn most_specific_nominal_constructor(
        &mut self,
        name: &str,
        applicable: &[ApplicableConstructor],
        span: ast::Span,
    ) -> Option<usize> {
        let mut forwards = vec![vec![false; applicable.len()]; applicable.len()];
        for source in 0..applicable.len() {
            for target in 0..applicable.len() {
                forwards[source][target] = source == target
                    || self.nominal_constructor_forwards(
                        NominalForwardingDeclaration {
                            view: &applicable[source].view,
                            parameter_types: &applicable[source].parameter_types,
                        },
                        NominalForwardingDeclaration {
                            view: &applicable[target].view,
                            parameter_types: &applicable[target].parameter_types,
                        },
                    );
            }
        }
        let mut pool = (0..applicable.len())
            .filter(|&candidate| {
                !(0..applicable.len()).any(|other| {
                    other != candidate && forwards[other][candidate] && !forwards[candidate][other]
                })
            })
            .collect::<Vec<_>>();
        let mutually_forwarding = pool.iter().all(|&source| {
            pool.iter()
                .all(|&target| forwards[source][target] && forwards[target][source])
        });
        if mutually_forwarding {
            let minimum_defaults = pool
                .iter()
                .map(|&candidate| applicable[candidate].argument_map.explicit_default_count())
                .min()
                .expect("constructor MSC receives candidates");
            pool.retain(|&candidate| {
                applicable[candidate].argument_map.explicit_default_count() == minimum_defaults
            });
            if pool.len() > 1 {
                let non_vararg = pool
                    .iter()
                    .copied()
                    .filter(|&candidate| {
                        !applicable[candidate]
                            .view
                            .value_parameters
                            .iter()
                            .any(|parameter| {
                                matches!(
                                    parameter.calling,
                                    crate::defaults::SourceParameterCalling::Vararg { .. }
                                )
                            })
                    })
                    .collect::<Vec<_>>();
                if !non_vararg.is_empty() {
                    pool = non_vararg;
                }
            }
        }
        if let [winner] = pool.as_slice() {
            return Some(*winner);
        }
        let signatures = pool
            .iter()
            .map(|&candidate| {
                format!(
                    "  - {}",
                    nominal_source_signature(self, &applicable[candidate].view)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!("ambiguous constructor call `{name}`:\n{signatures}"),
        );
        None
    }
}
