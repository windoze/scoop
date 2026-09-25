use super::*;

#[test]
fn suffix_comparison_visits_the_last_hop_before_rejecting_a_common_prefix() {
    let fixture = chain();
    let route = ReexportRouteV1::try_new(
        fixture.direct,
        vec![
            ReexportRouteHopV1::new(fixture.direct, fixture.direct_binding),
            ReexportRouteHopV1::new(fixture.terminal, fixture.terminal_binding),
        ],
    )
    .unwrap();
    let mut different = route.hops().to_vec();
    different[1] = ReexportRouteHopV1::new(fixture.terminal, fixture.current_binding);
    let routes = CanonicalReexportRoutesV1::try_new(vec![route.clone()]).unwrap();

    assert!(!routes.contains_exact_suffix(&different));

    assert!(routes.contains_exact_suffix(route.hops()));
}
