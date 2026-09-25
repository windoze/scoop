use super::*;
use std::convert::Infallible;

#[test]
fn default_type_access_visits_nested_types_and_preserves_binders() {
    let binder = Type::Binder {
        depth: u32::MAX,
        index: u32::MAX,
    };
    let mut demands = Vec::new();
    hir::visit_default_source_type_access_demands(&binder, &WirePath::root(), &mut |demand,

                                                                                    _|
     -> Result<
        (),
        Infallible,
    > {
        demands.push(demand);
        Ok(())
    })
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
    let result =
        hir::visit_default_source_type_access_demands(&ty, &WirePath::root(), &mut |_,

                                                                                    _|
         -> Result<
            (),
            Infallible,
        > {
            visits += 1;
            Ok(())
        });
    result.unwrap();
    assert_eq!(visits, 65);
}
