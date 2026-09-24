use super::*;
use hir::HirExpressionTypeRoleV1;
use source_dispatch::with_hir_source;

fn fixture() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-executable-type-sites/type-test.scoop"),
    )
    .unwrap()
}

#[test]
fn shared_type_sites_preserve_type_test_operands_without_unexpanded_defaults() {
    with_hir_source(&fixture(), |output, _| {
        let interface = public_interface(output);
        let local = output.output().local.module();
        let string = local.exact_type_identities.get(local.string).unwrap().id();
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
                &mut BudgetMeter::new(DecodeLimits::default()),
            )
            .unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let mut uses = Vec::new();
        for reference in interface.external_references().records() {
            for site in reference
                .type_sites()
                .records()
                .iter()
                .filter_map(|site| site.as_expression())
            {
                assert_eq!(reference.origin(), ConeIdentity::CORE);
                foundation
                    .validate_definition_origin_location(
                        local.cone,
                        site.origin().definition(),
                        &mut BudgetMeter::new(DecodeLimits::default()),
                        &WirePath::root(),
                    )
                    .unwrap();
                foundation
                    .validate_executable_evaluation_origin(
                        local.cone,
                        site.position().root,
                        site.origin().evaluation(),
                        &mut BudgetMeter::new(DecodeLimits::default()),
                        &WirePath::root(),
                    )
                    .unwrap();
                uses.push((site.role(), site.exact()));
            }
        }
        assert!(uses.contains(&(HirExpressionTypeRoleV1::TypeTest, string)));
        assert_eq!(uses.len(), 4);
        let operand = local
            .types
            .iter()
            .find(|(_, ty)| {
                ty.kind == hir::concrete::TypeKind::Integer(hir::IntegerKind::UNSIGNED_16)
            })
            .unwrap()
            .0;
        let dormant = local.exact_type_identities.get(operand).unwrap().id();
        assert!(uses.iter().all(|(_, exact)| *exact != dormant));
    });
}

#[test]
fn shared_interface_source_completion_uses_a_cumulative_budget() {
    with_hir_source(&fixture(), |output, _| {
        let interface = public_interface(output);
        let original = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        let complete = |foundation: &mut hir::CanonicalHirFoundation, meter: &mut BudgetMeter| {
            foundation.complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
                meter,
            )
        };
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                complete(&mut original.clone(), &mut BudgetMeter::new(limits)),
                Err(hir::HirFoundationBuildError::SourcePointResource(_))
            ));
        }
        let mut measured = BudgetMeter::new(DecodeLimits::default());
        complete(&mut original.clone(), &mut measured).unwrap();
        let mut bounded = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units,
            ..DecodeLimits::default()
        });
        complete(&mut original.clone(), &mut bounded).unwrap();
        assert!(matches!(
            complete(&mut original.clone(), &mut bounded),
            Err(hir::HirFoundationBuildError::SourcePointResource(_))
        ));
    });
}

#[test]
fn shared_constructor_type_sites_keep_their_actual_source_context() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-executable-type-sites/constructors.scoop"),
    )
    .unwrap();
    with_hir_source(&source, |output, _| {
        let interface = public_interface(output);
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
                &mut BudgetMeter::new(DecodeLimits::default()),
            )
            .unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let sites = interface
            .external_references()
            .records()
            .iter()
            .flat_map(|reference| reference.type_sites().records())
            .filter_map(|site| site.as_expression())
            .collect::<Vec<_>>();
        for site in &sites {
            foundation
                .validate_executable_evaluation_origin(
                    output.output().export.cone,
                    site.position().root,
                    site.origin().evaluation(),
                    &mut BudgetMeter::new(DecodeLimits::default()),
                    &WirePath::root(),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{:?}: {error}: {:?}",
                        site.position(),
                        foundation.source_context_key(site.origin().evaluation().context())
                    )
                });
        }
        let mut constructors = sites.iter().copied().filter(|site| {
            matches!(
                site.position().root.template(),
                scoop_identity::CallableTemplateOwner::Constructor(_)
            )
        });
        let first = constructors.next().unwrap();
        let other = constructors
            .find(|site| site.position().root != first.position().root)
            .unwrap();
        assert!(matches!(
            foundation.validate_executable_evaluation_origin(
                output.output().export.cone,
                first.position().root,
                other.origin().evaluation(),
                &mut BudgetMeter::new(DecodeLimits::default()),
                &WirePath::root(),
            ),
            Err(hir::ExecutableEvaluationValidationError::Context { .. })
        ));
    });
}
