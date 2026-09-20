use super::*;
use std::cell::Cell;

#[test]
fn declaration_domain_binds_every_required_table_before_entering_the_callback() {
    with_domain(SOURCE, |_, fixture, independent, source, core| {
        let foundation = fixture.bind().unwrap();
        for field in 1..=9 {
            let mut e = source.entries().clone();
            match field {
                1 => e.required_protected = Default::default(),
                2 => e.nominals = Default::default(),
                3 => e.constructors = Default::default(),
                4 => e.properties = Default::default(),
                5 => e.callables = Default::default(),
                6 => {
                    e.inheritance =
                        hir::CanonicalSourceInheritanceInventoriesV1::try_new(vec![], &mut meter())
                            .unwrap()
                }
                7 => e.interfaces = Default::default(),
                8 => e.selections = Default::default(),
                9 => e.dispatch_callables = Default::default(),
                _ => unreachable!(),
            }
            let source = Domain::new(e);
            let entered = Cell::new(false);
            let error = source
                .with_bound_sources(
                    &foundation,
                    &independent.protocols,
                    core,
                    &mut meter(),
                    |_, _| entered.set(true),
                )
                .unwrap_err();
            assert!(!entered.get(), "field {field}: {error}");
            if field == 1 {
                assert!(matches!(error, Error::ProtectedInventory));
            }
        }
        let entered = Cell::new(false);
        assert!(
            source
                .with_bound_sources(
                    &foundation,
                    &hir::CanonicalNominalSourceParameterProtocolsV1::default(),
                    core,
                    &mut meter(),
                    |_, _| entered.set(true)
                )
                .is_err()
        );
        assert!(!entered.get());
    });
}

#[test]
fn declaration_domain_rejects_shared_budget_exhaustion_before_callback_publication() {
    with_domain(SOURCE, |output, fixture, independent, source, core| {
        let foundation = fixture.bind().unwrap();
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
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
        ] {
            let entered = Cell::new(false);
            let error = source
                .with_bound_sources(
                    &foundation,
                    &independent.protocols,
                    core,
                    &mut BudgetMeter::new(limits),
                    |_, _| entered.set(true),
                )
                .unwrap_err();
            assert!(matches!(error, Error::Resource(_)), "{limits:?}: {error}");
            assert!(!entered.get());
            assert!(Domain::from_ordinary_hir(output, &mut BudgetMeter::new(limits)).is_err());
        }
    });
}

#[test]
fn declaration_domain_cannot_promote_a_public_source_to_a_protected_root() {
    with_domain(SOURCE, |_, fixture, independent, source, core| {
        let mut entries = source.entries().clone();
        let public = entries
            .callables
            .records()
            .iter()
            .find(|r| {
                r.declaration_access().declared_visibility() == hir::DeclaredVisibilityV1::Public
                    && matches!(
                        r.declaration(),
                        scoop_identity::CallableTemplateOrigin::Function(_)
                    )
            })
            .unwrap();
        let extra = hir::ProtectedDeclarationRefV1::Callable(
            hir::ProtectedCallableDeclarationRefV1::try_new(public.declaration()).unwrap(),
        );
        assert!(!entries.required_protected.values().contains(&extra));
        let mut required = entries.required_protected.values().to_vec();
        required.push(extra);
        entries.required_protected =
            hir::CanonicalProtectedDeclarationRefsV1::try_new(required).unwrap();
        let entered = Cell::new(false);
        let foundation = fixture.bind().unwrap();
        let error = Domain::new(entries)
            .with_bound_sources(
                &foundation,
                &independent.protocols,
                core,
                &mut meter(),
                |_, _| entered.set(true),
            )
            .unwrap_err();
        assert!(matches!(error, Error::ProtectedInventory));
        assert!(!entered.get());
    });
}
