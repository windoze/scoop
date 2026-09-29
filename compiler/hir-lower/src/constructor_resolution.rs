use scoop_ast as ast;
use scoop_hir as hir;

use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::diagnostics::nominal_source_signature;
use crate::call_resolution::specificity::ApplicableDeclaration;
use crate::expr::ResolvedCallTypeArgument;
use crate::expr::{NominalArgumentInput, NominalArguments};
use crate::{Lowerer, TypeId};

mod named;
mod safety;

pub(crate) use named::NamedNominalProbe;

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
            let view = self.nominal_constructor_view(source, span);
            match self.probe_named_nominal(
                view.clone(),
                NominalConstructorCall {
                    explicit_type_args,
                    expected_type_args,
                    arguments,
                    span,
                },
            ) {
                Ok(probe) => applicable.push(probe.candidate),
                Err(failure) => failures.push((view, failure)),
            }
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
        self.commit_nominal_candidate(winner, span, sink)
    }

    fn most_specific_nominal_constructor(
        &mut self,
        name: &str,
        applicable: &[ApplicableConstructor],
        span: ast::Span,
    ) -> Option<usize> {
        let declarations = applicable
            .iter()
            .map(|candidate| ApplicableDeclaration {
                declaration: candidate.view.forwarding(&candidate.parameter_types),
                parameterized: !candidate.view.owner_parameters.is_empty(),
                defaults: candidate.argument_map.explicit_default_count(),
                vararg: candidate
                    .view
                    .value_parameters
                    .iter()
                    .any(|parameter| parameter.is_vararg()),
            })
            .collect::<Vec<_>>();
        let pool = self.most_specific_declarations(&declarations);
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
