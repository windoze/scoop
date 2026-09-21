use super::*;
use std::convert::Infallible;

fn visit(ty: &Type, meter: &mut BudgetMeter) -> Result<(), Error<Infallible>> {
    hir::visit_default_source_type_access_demands(ty, meter, &WirePath::root(), &mut |_, _, _| {
        Ok(())
    })
}
#[test]
fn default_type_access_walk_and_callbacks_share_resource_budgets() {
    with_types(COMBINATIONS, &[("Envelope.combined", 1)], |_, templates| {
        let ty = templates[0].1.result();
        let mut measured = meter();
        visit(ty, &mut measured).unwrap();
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        });
        visit(ty, &mut shared).unwrap();
        assert!(matches!(visit(ty, &mut shared), Err(Error::Resource(_))));
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 1,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                matches!(
                    visit(ty, &mut BudgetMeter::new(limits)),
                    Err(Error::Resource(_))
                ),
                "{limits:?}"
            );
        }
        let mut limited = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units,
            ..DecodeLimits::default()
        });
        let error = hir::visit_default_source_type_access_demands(
            ty,
            &mut limited,
            &WirePath::root(),
            &mut |_, meter, path| meter.charge_work(measured.usage().validation_work_units, path),
        )
        .unwrap_err();
        assert!(matches!(error, Error::Visitor(_)));
    });
}
#[test]
fn default_type_access_demand_failure_stops_at_the_exact_occurrence() {
    with_types(COMBINATIONS, &[("Envelope.combined", 1)], |_, templates| {
        let ty = templates[0].1.result();
        let mut paths = Vec::new();
        hir::visit_default_source_type_access_demands(
            ty,
            &mut meter(),
            &WirePath::root(),
            &mut |_, _, path| -> Result<(), Infallible> {
                paths.push(path.clone());
                Ok(())
            },
        )
        .unwrap();
        for stop in 0..paths.len() {
            let mut seen = Vec::new();
            let error = hir::visit_default_source_type_access_demands(
                ty,
                &mut meter(),
                &WirePath::root(),
                &mut |_, _, path| {
                    seen.push(path.clone());
                    if seen.len() == stop + 1 {
                        Err(path.clone())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert!(matches!(error, Error::Visitor(path) if path == paths[stop]));
            assert_eq!(seen, paths[..=stop]);
        }
    });
}
#[test]
fn default_type_access_demands_borrow_original_applied_and_pointer_types() {
    with_types(SOURCE, CASES, |_, templates| {
        assert_borrowed_roots(templates);
    });
}
pub(super) fn assert_borrowed_roots(templates: &[(&str, hir::DefaultSourceTemplateV1)]) {
    for (_, template) in templates {
        let root = template.result();
        hir::visit_default_source_type_access_demands(
            root,
            &mut meter(),
            &WirePath::root(),
            &mut |demand, _, path| -> Result<(), Infallible> {
                if !path.segments().is_empty() {
                    return Ok(());
                }
                match (root, demand) {
                    (
                        Type::NominalApplication { arguments, .. },
                        Demand::NominalApplication {
                            arguments: borrowed,
                            ..
                        },
                    ) => assert!(std::ptr::eq(arguments.as_slice(), borrowed)),
                    (Type::RawPointer(pointee), Demand::RawPointer { pointee: borrowed }) => {
                        assert!(std::ptr::eq(pointee.as_ref(), borrowed))
                    }
                    (
                        Type::NativeFunctionPointer {
                            parameters, result, ..
                        },
                        Demand::NativeFunctionPointer {
                            parameters: borrowed_parameters,
                            result: borrowed_result,
                            ..
                        },
                    ) => {
                        assert!(std::ptr::eq(parameters.as_slice(), borrowed_parameters));
                        assert!(std::ptr::eq(result.as_ref(), borrowed_result));
                    }
                    (Type::Nominal(_), Demand::Nominal(_))
                    | (Type::Binder { .. }, Demand::Binder { .. }) => {}
                    _ => panic!("root demand preserves its type"),
                }
                Ok(())
            },
        )
        .unwrap();
    }
}
