use super::*;
use std::convert::Infallible;

#[test]
fn default_type_access_checks_table_limits_before_exposing_applied_arguments() {
    with_types(SOURCE, &[("applied", 0)], |_, templates| {
        let mut visits = 0;
        let result = hir::visit_default_source_type_access_demands(
            templates[0].1.result(),
            &mut BudgetMeter::new(DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            }),
            &WirePath::root(),
            &mut |_, _, _| -> Result<(), Infallible> {
                visits += 1;
                Ok(())
            },
        );
        assert!(matches!(result, Err(Error::Resource(_))));
        assert_eq!(visits, 0);
    });
}

#[test]
fn default_type_access_caps_recursive_types_and_preserves_unvalidated_binders() {
    let binder = Type::Binder {
        depth: u32::MAX,
        index: u32::MAX,
    };
    let mut demands = Vec::new();
    hir::visit_default_source_type_access_demands(
        &binder,
        &mut meter(),
        &WirePath::root(),
        &mut |demand, _, _| -> Result<(), Infallible> {
            demands.push(demand);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        demands,
        [Demand::Binder {
            depth: u32::MAX,
            index: u32::MAX
        }]
    );
    let mut ty = binder;
    for _ in 0..64 {
        ty = Type::RawPointer(Box::new(ty));
    }
    let mut visits = 0;
    let result = hir::visit_default_source_type_access_demands(
        &ty,
        &mut BudgetMeter::new(DecodeLimits {
            semantic_recursion: 5,
            ..DecodeLimits::default()
        }),
        &WirePath::root(),
        &mut |_, _, _| -> Result<(), Infallible> {
            visits += 1;
            Ok(())
        },
    );
    assert!(matches!(result, Err(Error::Resource(_))));
    assert_eq!(visits, 5);
}
