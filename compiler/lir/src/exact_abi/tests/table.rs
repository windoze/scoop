use super::*;

fn records() -> (
    OdrFreeLirFoundation,
    Vec<ExactCallableAbiExportV1>,
    Vec<StrongCallableDefinitionOwner>,
) {
    let (first, first_foundation) = fixtures::foundation("first", true);
    let (second, second_foundation) = fixtures::foundation("second", true);
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(
            first_foundation
                .callable_bodies()
                .iter()
                .chain(second_foundation.callable_bodies())
                .cloned()
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_plans(
            first_foundation
                .definition_plans()
                .iter()
                .chain(second_foundation.definition_plans())
                .cloned()
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            first_foundation
                .definition_atoms()
                .iter()
                .chain(second_foundation.definition_atoms())
                .cloned()
                .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            first_foundation
                .symbol_requests()
                .iter()
                .chain(second_foundation.symbol_requests())
                .copied()
                .collect(),
        )
        .unwrap(),
    );
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let unit: ExactLayoutExportV1 = unit().into();
    let make = |target| {
        ExactCallableAbiExportV1::from_signature(
            TARGET,
            target,
            scoop_identity::CanonicalScoopAbiFunctionSignature::new(
                ExactCallableSignature::new(
                    Effect::Ordinary,
                    None,
                    Vec::new(),
                    unit.identity().exact(),
                ),
                vec![],
                unit.value_handle()
                    .unwrap()
                    .scoop_abi_return(TARGET)
                    .unwrap(),
                (ExactCallableProtocolV1::OrdinaryNoGc).gc_effect(),
            )
            .unwrap(),
            &foundation,
        )
        .unwrap()
    };
    let records = vec![make(second), make(first)];
    (foundation, records, vec![first, second])
}

#[test]
fn table_canonicalizes_targets_and_round_trips_complete_records() {
    let (foundation, records, mut targets) = records();
    let table = CanonicalExactCallableAbiExportsV1::try_new(TARGET, &foundation, records).unwrap();
    targets.sort_unstable();
    assert_eq!(table.provider(), ConeIdentity::SINGLE_FILE);
    assert_eq!(table.target(), TARGET);
    assert_eq!(
        table
            .records()
            .iter()
            .map(ExactCallableAbiExportV1::target)
            .collect::<Vec<_>>(),
        targets
    );
    for target in targets {
        assert_eq!(
            table.get(target).map(ExactCallableAbiExportV1::target),
            Some(target)
        );
    }
    let bytes = encode(&table).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalExactCallableAbiExportsV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.validate_against(&table).unwrap(), table);
}
