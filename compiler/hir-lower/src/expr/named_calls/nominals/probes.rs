use super::*;

impl Lowerer {
    pub(super) fn collect_prepared_nominal_probes(
        &mut self,
        prepared: PreparedNominalPlans,
        call: &ast::CallExpr,
        explicit: &[ResolvedCallTypeArgument],
        applicable: &mut Vec<NamedApplicable>,
        failures: &mut Vec<Box<Lowerer>>,
    ) {
        let (plans, pointer) = match prepared {
            PreparedNominalPlans::Imported {
                owner,
                expected,
                fixed_alias,
                alias,
            } => {
                self.clone().collect_imported_constructor_probes(
                    owner,
                    call,
                    expected,
                    fixed_alias,
                    alias.as_ref(),
                    applicable,
                    failures,
                );
                return;
            }
            PreparedNominalPlans::Nominal(plans, pointer) => (plans, pointer),
        };
        for plan in plans {
            let arguments = self
                .named_nominal_expected_arguments(plan.view.signature.return_type, plan.expected);
            let input = NominalConstructorCall {
                explicit_type_args: explicit,
                expected_type_args: arguments.as_deref(),
                arguments: &call.args,
                span: call.span,
            };
            let probe = match plan.alias {
                Some(alias) => {
                    self.probe_named_alias_constructor(plan.view, &alias, input, plan.expected)
                }
                None => self.probe_named_nominal(plan.view, input),
            };
            match probe {
                Ok(mut probe) => {
                    if plan.fixed_alias {
                        probe.fix_forwarding_parameters(
                            self,
                            arguments
                                .as_deref()
                                .expect("a fixed nominal alias supplies its complete application"),
                        );
                    }
                    applicable.push(NamedApplicable {
                        probe: NamedFunctionLikeProbe::Nominal(Box::new(probe)),
                        commit: NamedFunctionCommit::Nominal,
                    });
                }
                Err(failure) => failures.push(failure),
            }
        }
        if let Some((structure, expected, fixed_alias, alias)) = pointer {
            self.clone().collect_pointer_construction_probe(
                crate::call_resolution::named::NamedIntrinsicStructOrigin::Current(structure),
                call,
                expected,
                fixed_alias,
                alias.as_ref(),
                applicable,
                failures,
            );
        }
    }
}
