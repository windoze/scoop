use super::*;

#[derive(Debug)]
enum VisitError {
    Resource(WireError),
    Stop,
}
impl From<WireError> for VisitError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

#[test]
fn default_source_origin_walk_stops_at_callback_failure() {
    with_hir_source(ORIGINS, |output, _| {
        let production = Production::from_ordinary_hir(output, &mut meter()).unwrap();
        let mut visits = 0;
        let result = production.templates().visit_definition_sources_metered(
            &mut |_, _, site, _, _| {
                assert!(matches!(site, Site::Root));
                visits += 1;
                Err(VisitError::Stop)
            },
            &mut meter(),
            &WirePath::root(),
        );
        assert!(matches!(result, Err(VisitError::Stop)));
        assert_eq!(visits, 1);
    });
}

#[test]
fn default_source_origin_walk_and_callbacks_share_resource_limits() {
    with_hir_source(ORIGINS, |output, _| {
        let production = Production::from_ordinary_hir(output, &mut meter()).unwrap();
        let walk = |meter: &mut BudgetMeter| {
            production.templates().visit_definition_sources_metered(
                &mut |_, _, _, meter, path| {
                    meter.charge_work(10, path)?;
                    Ok::<_, VisitError>(())
                },
                meter,
                &WirePath::root(),
            )
        };
        let mut measured = meter();
        walk(&mut measured).unwrap();
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        });
        walk(&mut shared).unwrap();
        assert!(
            matches!(walk(&mut shared), Err(VisitError::Resource(ref error)) if matches!(error.kind(), scoop_wire::WireErrorKind::LimitExceeded { .. }))
        );
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
        ] {
            let mut visits = 0;
            let result = production.templates().visit_definition_sources_metered(
                &mut |_, _, _, _, _| {
                    visits += 1;
                    Ok::<_, VisitError>(())
                },
                &mut BudgetMeter::new(limits),
                &WirePath::root(),
            );
            assert!(matches!(result, Err(VisitError::Resource(_))));
            assert_eq!(visits, 0);
        }
    });
}
