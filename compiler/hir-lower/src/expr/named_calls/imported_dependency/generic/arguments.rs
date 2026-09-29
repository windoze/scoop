//! Contextual arguments feed the shared solver without changing source order.

use super::*;
use crate::call_resolution::arguments::SourceInputId;
use crate::call_resolution::constraints::InferenceEnvironmentId;

pub(super) struct InferredImportedArguments {
    pub(super) source_args: Vec<hir::Expr>,
    pub(super) argument_sinks: Vec<Vec<hir::Statement>>,
}

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn infer_imported_generic_arguments(
        &mut self,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        signature: &ImportedInferenceSignature,
        patterns: &[hir::TypeId],
        session: &mut InferenceSession,
        environment: InferenceEnvironmentId,
        address_place: Option<(hir::Place, hir::TypeId)>,
    ) -> Option<InferredImportedArguments> {
        debug_assert_eq!(patterns.len(), call.arguments.len());
        let mut arguments = vec![None; patterns.len()];
        let mut sinks = (0..patterns.len()).map(|_| Vec::new()).collect::<Vec<_>>();
        let lower = |state: &mut Lowerer, index: usize, expected| {
            let before = state.diagnostics.len();
            let mut sink = Vec::new();
            let value = if let Some((place, ty)) = address_place {
                let span = call.arguments.span(index);
                Some(hir::Expr {
                    kind: match place {
                        hir::Place::Local(local) => hir::ExprKind::Local(local),
                        hir::Place::Global(global) => hir::ExprKind::GlobalRead(global),
                    },
                    ty,
                    span,
                    origin: state.expression_origin(span),
                })
            } else {
                call.arguments.lower(index, state, &mut sink, expected)
            };
            value
                .filter(|_| state.diagnostics.len() == before)
                .map(|value| (value, sink))
        };

        for (index, &pattern) in patterns.iter().enumerate() {
            if address_place.is_none()
                && call
                    .arguments
                    .source(index)
                    .is_some_and(|source| self.expr_requires_expected_type(source))
            {
                continue;
            }
            let (value, sink) = lower(self, index, None)?;
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(value.ty), TypeTerm::Type(pattern)),
                ConstraintOrigin::Argument(SourceInputId::from_index(index)),
            );
            arguments[index] = Some(value);
            sinks[index] = sink;
        }

        while arguments.iter().any(Option::is_none) {
            let partial = match self.solve_constraints_partially(session, environment) {
                Ok(partial) => partial,
                Err(failure) => {
                    self.imported_generic_inference_error(name, call, signature, &failure);
                    return None;
                }
            };
            let bindings = signature
                .owner_parameters
                .iter()
                .chain(&signature.type_parameters)
                .zip(partial.owner.into_iter().chain(partial.callable))
                .map(|(parameter, ty)| {
                    (
                        parameter.id,
                        ty.unwrap_or_else(|| self.intern_type(hir::Type::Param(parameter.id))),
                    )
                })
                .collect::<Vec<_>>();
            let mut progress = false;
            for (index, &pattern) in patterns.iter().enumerate() {
                if arguments[index].is_some() {
                    continue;
                }
                let hint = self.instantiate_method_ty(pattern, &bindings);
                // Resolved outer binders remain valid contextual types.
                if crate::call_resolution::type_contains_session_parameter(self, session, hint) {
                    continue;
                }
                let (value, sink) = lower(self, index, Some(hint))?;
                session.push(
                    Constraint::Subtype(TypeTerm::Rigid(value.ty), TypeTerm::Type(pattern)),
                    ConstraintOrigin::Argument(SourceInputId::from_index(index)),
                );
                arguments[index] = Some(value);
                sinks[index] = sink;
                progress = true;
            }
            if progress {
                continue;
            }

            let mut seeds = arguments
                .iter()
                .enumerate()
                .filter_map(|(index, value)| value.is_none().then_some(index))
                .collect::<Vec<_>>();
            seeds.sort_by_key(|&index| {
                std::cmp::Reverse(
                    call.arguments
                        .source(index)
                        .and_then(|source| self.expr_default_seed_rank(source)),
                )
            });
            let mut first_failure = None;
            for index in seeds {
                let mut attempt = self.clone();
                if let Some((value, sink)) = lower(&mut attempt, index, None) {
                    *self = attempt;
                    session.push(
                        Constraint::Subtype(
                            TypeTerm::Rigid(value.ty),
                            TypeTerm::Type(patterns[index]),
                        ),
                        ConstraintOrigin::Argument(SourceInputId::from_index(index)),
                    );
                    arguments[index] = Some(value);
                    sinks[index] = sink;
                    progress = true;
                    break;
                }
                first_failure.get_or_insert_with(|| Box::new(attempt));
            }
            if !progress {
                *self = *first_failure.expect("an unresolved source argument was probed");
                return None;
            }
        }

        Some(InferredImportedArguments {
            source_args: arguments
                .into_iter()
                .map(|argument| argument.expect("the argument fixed point is complete"))
                .collect(),
            argument_sinks: sinks,
        })
    }
}
