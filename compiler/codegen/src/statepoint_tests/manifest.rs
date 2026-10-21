use super::*;

#[test]
fn callable_plans_share_effect_table_and_own_their_sites() {
    let mut expected = manifest(Some((7, one_root())));
    expected.functions = BTreeMap::from([
        ("f".into(), GcEffect::Managed),
        ("g".into(), GcEffect::Managed),
        ("leaf".into(), GcEffect::NoGc),
    ])
    .into();
    expected.sites.insert(
        8,
        ExpectedSite {
            function: "g".into(),
            block: "entry".into(),
            statepoint: ExpectedStatepoint::ZeroLiveInvoke,
        },
    );

    let mut selected = expected.clone().into_functions();
    let mut first = selected.remove("f").unwrap();
    let second = selected.remove("g").unwrap();
    assert_eq!(selected["leaf"].site_count(), 0);
    let empty = first.without_body_sites();
    assert_eq!(first.sites.keys().copied().collect::<Vec<_>>(), [7]);
    assert_eq!(first.root_count(7), Some(1));
    assert_eq!(second.sites.keys().copied().collect::<Vec<_>>(), [8]);
    assert_eq!(second.root_count(8), Some(0));
    assert_eq!(empty.site_count(), 0);
    for plan in [&first, &second, &empty, &first.clone()] {
        assert_eq!(plan.functions, expected.functions);
        for name in ["f", "g", "leaf"] {
            assert!(std::ptr::eq(
                plan.functions.get(name).unwrap(),
                expected.functions.get(name).unwrap(),
            ));
        }
    }
    first.sites.clear();
    assert_eq!(expected.site_count(), 2);
    assert_eq!(second.site_count(), 1);
    assert!(!selected.contains_key("missing"));
}
