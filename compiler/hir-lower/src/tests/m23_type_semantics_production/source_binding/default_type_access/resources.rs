use super::*;
use std::convert::Infallible;

#[test]
fn default_type_access_demand_failure_stops_at_the_exact_occurrence() {
    with_types(COMBINATIONS, &[("Envelope.combined", 1)], |_, templates| {
        let ty = templates[0].1.result();
        let mut paths = Vec::new();
        hir::visit_default_source_type_access_demands(ty, &WirePath::root(), &mut |_,

                                                                                   path|
         -> Result<
            (),
            Infallible,
        > {
            paths.push(path.clone());
            Ok(())
        })
        .unwrap();
        for stop in 0..paths.len() {
            let mut seen = Vec::new();
            let error = hir::visit_default_source_type_access_demands(
                ty,
                &WirePath::root(),
                &mut |_, path| {
                    seen.push(path.clone());
                    if seen.len() == stop + 1 {
                        Err(path.clone())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert_eq!(error, paths[stop]);
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
        hir::visit_default_source_type_access_demands(root, &WirePath::root(), &mut |demand,

                                                                                     path|
         -> Result<
            (),
            Infallible,
        > {
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
        })
        .unwrap();
    }
}
