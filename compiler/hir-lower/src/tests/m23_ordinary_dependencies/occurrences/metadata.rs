use scoop_identity::EvaluationOrigin;
use scoop_wire::WirePath;

use super::*;

#[test]
fn common_reference_metadata_preserves_each_actual_call_and_its_route() {
    for case in ["standalone", "routes", "combined", "rejected"] {
        support::with_output(&fixture(case), |output, world| {
            let mut foundation =
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let mut authority = hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &output.output().export.public_export_bindings,
                world,
            );
            let interface = hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &output,
                &[],
                &mut authority,
            )
            .unwrap();
            foundation
                .complete_cross_cone_interface_source_points(
                    output.output().export.module(),
                    &interface,
                    &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
                )
                .unwrap();
            let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
            let calls = output
                .committed_dependency_call_occurrences(&mut meter())
                .unwrap();
            let mut observed = 0;
            for reference in interface.external_references().records() {
                for site in reference.call_sites().records() {
                    observed += 1;
                    let actual = calls
                        .iter()
                        .find(|call| call.position() == site.position())
                        .unwrap();
                    assert_eq!(
                        reference.target(),
                        hir::ExternalHirTargetV1::Callable(
                            actual.callable().interface().declaration()
                        )
                    );
                    assert_eq!(site.arguments().len(), actual.arguments().len());
                    let mut routes = actual
                        .binding()
                        .sources()
                        .map(|source| source.witness().dependency())
                        .collect::<Vec<_>>();
                    routes.sort_unstable();
                    assert_eq!(
                        site.witness_indices()
                            .iter()
                            .map(|index| &reference.witnesses().witnesses()[*index as usize])
                            .collect::<Vec<_>>(),
                        routes
                    );
                    foundation
                        .validate_executable_evaluation_origin(
                            output.output().export.cone,
                            site.position().root,
                            site.origin().evaluation(),
                            &mut meter(),
                            &WirePath::root(),
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "{case}: {}: {error}",
                                root_name(output.output().local.module(), site.position().root)
                            )
                        });
                    foundation
                        .validate_definition_origin_location(
                            output.output().export.cone,
                            site.origin().definition(),
                            &mut meter(),
                            &WirePath::root(),
                        )
                        .unwrap();
                }
            }
            assert_eq!(observed, calls.len());
        });
    }
}

#[test]
fn call_evaluation_rejects_another_executable_context_in_the_same_source() {
    support::with_output(&fixture("combined"), |output, world| {
        let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        let mut authority = hir::CrossConeHirProductionAuthority::new(
            &foundation,
            &output.output().export.public_export_bindings,
            world,
        );
        let interface =
            hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(&output, &[], &mut authority)
                .unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let sites = interface
            .external_references()
            .records()
            .iter()
            .flat_map(|record| record.call_sites().records())
            .collect::<Vec<_>>();
        let first = sites[0];
        let second = sites
            .iter()
            .find(|site| {
                site.origin().evaluation().context() != first.origin().evaluation().context()
            })
            .unwrap();
        let evaluation = first.origin().evaluation();
        let wrong_context = foundation
            .source_context_key(second.origin().evaluation().context())
            .unwrap();
        let wrong = EvaluationOrigin::new(
            evaluation.source().clone(),
            evaluation.span(),
            wrong_context,
        )
        .unwrap();
        assert!(matches!(
            foundation.validate_executable_evaluation_origin(
                output.output().export.cone,
                first.position().root,
                &wrong,
                &mut meter(),
                &WirePath::root(),
            ),
            Err(hir::ExecutableEvaluationValidationError::Context { .. })
        ));
    });
}
