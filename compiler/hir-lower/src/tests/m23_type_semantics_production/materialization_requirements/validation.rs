use super::*;

#[test]
fn materialization_requirements_reject_orphan_constructor_and_slot_owners() {
    source_dispatch::with_hir_source(COMBINED, |output, _| {
        let public = public_interface(output);
        let identities = source_inventory::identity_closure(output);
        for missing in ["Factory", "Root"] {
            let original = public.nominal_interfaces();
            let removed = original
                .all_records()
                .find_map(|record| match record.declaration() {
                    hir::SourceNominalId::Concrete(owner)
                        if name(owner, &identities) == missing =>
                    {
                        Some(owner)
                    }
                    _ => None,
                })
                .unwrap();
            let keep = |record: &&hir::NominalInterfaceRecordV1| {
                record.declaration() != hir::SourceNominalId::Concrete(removed)
            };
            let broken = hir::CanonicalNominalInterfacesV1::with_support(
                original.records().iter().filter(keep).cloned().collect(),
                original
                    .support_records()
                    .iter()
                    .filter(keep)
                    .cloned()
                    .collect(),
            )
            .unwrap();
            let error = broken
                .visit_materialization_requirements::<Error>(
                    public.callable_interfaces(),
                    &mut meter(),
                    |_, _| Ok(()),
                )
                .unwrap_err();
            assert_eq!(error, Error::MissingNominal(removed));
            assert_eq!(
                hir::NominalMaterializationClosure::from_declarations(
                    &broken,
                    public.callable_interfaces(),
                    &mut meter()
                )
                .unwrap_err(),
                error
            );
            assert!(
                error
                    .to_string()
                    .contains("invalid shared nominal materialization closure")
            );
        }
    });
}

#[test]
fn materialization_requirement_callback_failure_stops_at_the_actual_position() {
    source_dispatch::with_hir_source(STANDALONE, |output, _| {
        let public = public_interface(output);
        let mut visits = 0;
        let mut rejected = None;
        let error = public
            .nominal_interfaces()
            .visit_materialization_requirements::<Error>(
                public.callable_interfaces(),
                &mut meter(),
                |requirement, _| {
                    visits += 1;
                    if visits == 2 {
                        rejected = Some(requirement.owner());
                        Err(Error::MissingNominal(requirement.owner()))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(visits, 2);
        assert_eq!(error, Error::MissingNominal(rejected.unwrap()));
    });
}

#[test]
fn materialization_requirements_share_inclusive_budget_with_the_consumer() {
    source_dispatch::with_hir_source(COMBINED, |output, _| {
        let public = public_interface(output);
        let run = |budget: &mut BudgetMeter| {
            public
                .nominal_interfaces()
                .visit_materialization_requirements::<Error>(
                    public.callable_interfaces(),
                    budget,
                    |_, budget| {
                        budget.charge_work(7, &WirePath::root())?;
                        Ok(())
                    },
                )
        };
        let mut complete = meter();
        run(&mut complete).unwrap();
        let needed = complete.usage().validation_work_units;
        for (limit, accepted) in [(0, false), (needed - 1, false), (needed, true)] {
            let mut budget = BudgetMeter::new(DecodeLimits {
                validation_work_units: limit,
                ..DecodeLimits::default()
            });
            assert_eq!(run(&mut budget).is_ok(), accepted);
        }
        let mut budget = BudgetMeter::new(DecodeLimits {
            validation_work_units: needed,
            ..DecodeLimits::default()
        });
        budget.charge_work(1, &WirePath::root()).unwrap();
        assert!(run(&mut budget).is_err());
        let mut budget = BudgetMeter::new(DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        });
        assert!(run(&mut budget).is_err());
    });
}
