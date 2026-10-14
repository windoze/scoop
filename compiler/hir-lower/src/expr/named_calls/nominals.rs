use super::*;
use crate::constructor_resolution::NominalConstructorCall;

mod finish;
mod functions;
mod imported;
mod plans;
mod pointers;
mod probes;
mod selection;

pub(in crate::expr) struct NominalPlan {
    pub(in crate::expr) view: NominalConstructorView,
    pub(in crate::expr) expected: Option<TypeId>,
    pub(in crate::expr) fixed_alias: bool,
    pub(in crate::expr) alias: Option<crate::aliases::AliasTemplate>,
}

pub(in crate::expr) enum PreparedNominalPlans {
    Nominal(
        Vec<NominalPlan>,
        Option<(
            hir::StructId,
            Option<TypeId>,
            bool,
            Option<crate::aliases::AliasTemplate>,
        )>,
    ),
    Imported {
        owner: hir::SourceNominalId,
        expected: Option<TypeId>,
        fixed_alias: bool,
        alias: Option<crate::aliases::AliasTemplate>,
    },
}

impl Lowerer {
    pub(crate) fn lower_named_function_partition(
        &mut self,
        targets: &[NamedCallBinding],
        kind: ImportLookupLayer,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let mut plans = Vec::new();
        let diagnostics_before = self.diagnostics.len();
        let mut failures = Vec::new();
        let mut intrinsics = Vec::new();
        let mut imported = Vec::new();
        for binding in targets {
            let mut preparation = self.clone();
            match preparation.named_nominal_plans(binding, call, expected) {
                Ok(PreparedNominalPlans::Nominal(mut prepared, intrinsic)) => {
                    *self = preparation;
                    plans.append(&mut prepared);
                    if let Some(intrinsic) = intrinsic {
                        intrinsics.push(intrinsic);
                    }
                }
                Ok(PreparedNominalPlans::Imported {
                    owner,
                    expected,
                    fixed_alias,
                    alias,
                }) => {
                    *self = preparation.clone();
                    imported.push((preparation, owner, expected, fixed_alias, alias));
                }
                Err(()) => {
                    failures.push(Box::new(preparation));
                }
            }
        }
        let explicit = self.resolve_call_type_args(&call.type_args).ok_or(())?;
        let overload = OverloadCall {
            explicit_type_args: &explicit,
            arg_exprs: &call.args,
            span: call.span,
            expected_result: expected,
            argument_protocol: CallArgumentProtocol::Ordinary,
        };
        let functions = targets
            .iter()
            .filter_map(|binding| match binding.target {
                NamedCallTarget::Function(id) => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();
        if plans.is_empty()
            && intrinsics.is_empty()
            && !functions.is_empty()
            && functions.len() == targets.len()
            && functions.iter().all(|id| {
                !self.extension_receivers.contains_key(id) && !self.function_owner.contains_key(id)
            })
        {
            return match self.resolve_overload_outcome(
                &call.callee.text,
                &functions,
                &[],
                overload,
                sink,
            ) {
                crate::overload::OverloadResolutionOutcome::NoApplicable => Ok(None),
                crate::overload::OverloadResolutionOutcome::Blocked => Ok(None),
                crate::overload::OverloadResolutionOutcome::Failed => Err(()),
                crate::overload::OverloadResolutionOutcome::Resolved(resolved) => self
                    .finish_resolved_top_level_function_call(call, *resolved, sink)
                    .map(Some)
                    .ok_or(()),
            };
        }
        let mut applicable = Vec::new();
        self.collect_named_function_probes(targets, call, overload, &mut applicable, &mut failures);
        for (mut state, owner, expected, fixed_alias, alias) in imported {
            state.collect_prepared_nominal_probes(
                PreparedNominalPlans::Imported {
                    owner,
                    expected,
                    fixed_alias,
                    alias,
                },
                call,
                &explicit,
                &mut applicable,
                &mut failures,
            );
        }
        self.collect_prepared_nominal_probes(
            PreparedNominalPlans::Nominal(plans, None),
            call,
            &explicit,
            &mut applicable,
            &mut failures,
        );
        for intrinsic in intrinsics {
            self.collect_prepared_nominal_probes(
                PreparedNominalPlans::Nominal(Vec::new(), Some(intrinsic)),
                call,
                &explicit,
                &mut applicable,
                &mut failures,
            );
        }
        if applicable.is_empty() {
            if let Some(blocked) = failures
                .iter()
                .position(|failure| failure.diagnostics.len() == diagnostics_before)
            {
                // An argument can be blocked by a previously rejected
                // declaration. Do not relabel that failure as an unknown name.
                self.commit_layer_diagnostics(*failures.swap_remove(blocked));
                return Err(());
            }
            let functions = targets
                .iter()
                .filter_map(|binding| match binding.target {
                    NamedCallTarget::Function(id)
                        if !self.extension_receivers.contains_key(&id)
                            && !self.function_owner.contains_key(&id) =>
                    {
                        Some(id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if functions.len() > 1
                && targets
                    .iter()
                    .all(|binding| matches!(binding.target, NamedCallTarget::Function(_)))
            {
                return Ok(self.lower_top_level_function_layer(
                    &call.callee.text,
                    &functions,
                    call,
                    sink,
                    expected,
                ));
            }
            if failures.len() == 1 {
                self.commit_layer_diagnostics(*failures.pop().expect("one failed candidate"));
            } else if !failures.is_empty() {
                let mut traces = failures
                    .iter()
                    .map(|failure| {
                        failure.diagnostics[diagnostics_before..]
                            .iter()
                            .map(|diagnostic| diagnostic.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .collect::<Vec<_>>();
                traces.sort();
                self.error(
                    call.span,
                    format!(
                        "no applicable candidate for `{}` in {} layer:\n{}",
                        call.callee.text,
                        kind.call_name(),
                        traces
                            .iter()
                            .map(|trace| format!("  - {trace}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                );
            }
            return Ok(None);
        }
        self.commit_named_function_partition(applicable, targets, kind, call, sink)
    }
}
