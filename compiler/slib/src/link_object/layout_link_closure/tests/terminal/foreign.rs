use super::*;
use crate::CanonicalDefinedLinkSymbolOwnerSetV1;
use scoop_identity::ConeCoordinate;
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind};

#[test]
fn incremental_terminal_owners_reject_siblings_in_both_arrival_orders() {
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let sibling = ConeCoordinate::new("test", "layout-terminal-sibling", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let owners = owner_set_for_callable(sibling, "siblingOwner");
    let imports = consumer.section.selected().physical_imports();
    let run = |owners: &[&CanonicalDefinedLinkSymbolOwnerSetV1]| {
        reject_layout_foreign_strong_owners_v1(
            consumer.identity(),
            TARGET,
            imports,
            owners.iter().copied(),
            &mut meter(),
        )
    };
    run(&[&fixture.provider_owners, &owners]).unwrap();
    run(&[&owners]).unwrap();
    let import = &imports.records()[0];
    let symbol = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(import.expected_symbol().symbol().as_str())
        .into_bytes();
    let expected = fixture
        .provider_owners
        .owners()
        .iter()
        .find(|owner| owner.symbol() == symbol)
        .unwrap()
        .owner();
    let changed = owners.insert_foreign_owner_for_test(symbol, expected);
    for candidates in [vec![&fixture.provider_owners, &changed], vec![&changed]] {
        assert!(matches!(
            run(&candidates),
            Err(CrossConeLayoutTerminalValidationError::ForeignStrongDefinition {
                context, actual_owner, symbol,
            }) if actual_owner == sibling && context.consumer == consumer.identity()
                && context.provider == fixture.provider_id()
                && context.subject == import.subject() && symbol == import.expected_symbol()
        ));
    }
}

#[test]
fn incremental_terminal_owner_checks_consume_the_existing_budget() {
    let fixture = Fixture::new();
    let consumer = Consumer::new(&fixture.provider);
    let run = |meter: &mut BudgetMeter| {
        reject_layout_foreign_strong_owners_v1(
            consumer.identity(),
            TARGET,
            consumer.section.selected().physical_imports(),
            std::iter::once(&consumer.defined_symbols),
            meter,
        )
    };
    let mut measured = meter();
    run(&mut measured).unwrap();
    let usage = measured.usage();
    let exact = DecodeLimits {
        validation_work_units: usage.validation_work_units,
        owned_bytes: usage.owned_bytes,
        ..DecodeLimits::default()
    };
    run(&mut BudgetMeter::new(exact)).unwrap();
    for (resource, limits) in [
        (
            ResourceKind::ValidationWorkUnits,
            DecodeLimits {
                validation_work_units: usage.validation_work_units - 1,
                ..exact
            },
        ),
        (
            ResourceKind::OwnedBytes,
            DecodeLimits {
                owned_bytes: usage.owned_bytes - 1,
                ..exact
            },
        ),
        (
            ResourceKind::LogicalHeapBytes,
            DecodeLimits {
                logical_heap_bytes: 0,
                ..exact
            },
        ),
    ] {
        assert!(matches!(
            run(&mut BudgetMeter::new(limits)),
            Err(CrossConeLayoutTerminalValidationError::Resource(error))
                if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. }
                    if *actual == resource)
        ));
    }
    let mut consumed = BudgetMeter::new(exact);
    consumed
        .charge_work(1, &scoop_wire::WirePath::root())
        .unwrap();
    assert!(matches!(
        run(&mut consumed),
        Err(CrossConeLayoutTerminalValidationError::Resource(_))
    ));
}
