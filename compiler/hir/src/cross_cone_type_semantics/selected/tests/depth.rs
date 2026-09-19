use super::*;

#[test]
fn selected_use_depth_includes_nested_construction_member_and_direct_edge() {
    let f = Fixture::new();
    for (usage, _) in f.cases() {
        let nested = matches!(
            usage,
            SelectedTypeUseV1::Construct { .. }
                | SelectedTypeUseV1::MemberCall { .. }
                | SelectedTypeUseV1::Inheritance { .. }
        );
        let required = if nested { 2 } else { 1 };
        let decoded_usage: DecodedSelectedTypeUseV1 = parsed(&usage);
        let mut resolver = f.resolver();
        resource(
            decoded_usage
                .clone()
                .resolve(
                    &mut resolver,
                    &mut BudgetMeter::new(DecodeLimits {
                        semantic_recursion: required - 1,
                        ..DecodeLimits::default()
                    }),
                    &path(),
                )
                .unwrap_err(),
            ResourceKind::SemanticRecursion,
            &path(),
        );
        assert!(resolver.calls.is_empty());
        assert_eq!(
            decoded_usage
                .resolve(
                    &mut resolver,
                    &mut BudgetMeter::new(DecodeLimits {
                        semantic_recursion: required,
                        ..DecodeLimits::default()
                    }),
                    &path()
                )
                .unwrap(),
            usage
        );

        let record = f.record(usage);
        let decoded: DecodedSelectedExternalTypeUseV1 = parsed(&record);
        let mut resolver = f.resolver();
        resource(
            decoded
                .clone()
                .resolve(
                    &mut resolver,
                    &mut BudgetMeter::new(DecodeLimits {
                        semantic_recursion: required,
                        ..DecodeLimits::default()
                    }),
                    &path(),
                )
                .unwrap_err(),
            ResourceKind::SemanticRecursion,
            &path().field(2),
        );
        assert_eq!(resolver.calls, [Family::Cone]);
        assert_eq!(
            decoded
                .resolve(
                    &mut resolver,
                    &mut BudgetMeter::new(DecodeLimits {
                        semantic_recursion: required + 1,
                        ..DecodeLimits::default()
                    }),
                    &path()
                )
                .unwrap(),
            record
        );
    }
}
