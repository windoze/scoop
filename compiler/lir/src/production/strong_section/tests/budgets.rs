use super::*;
use scoop_wire::BudgetMeter;

#[test]
fn forged_digest_kind_cannot_reduce_foundation_resolution_budget() {
    let coordinate = ConeCoordinate::new("test", "digest-owner-budget", "1.0.0").unwrap();
    let (foundation, image) = fixture(&coordinate);
    let atom = foundation.definition_atoms()[0].id();
    let key = DigestNodeKey::object_definition(atom);
    let node = DigestNodeV1::new(key, vec![], vec![]).unwrap();
    let mut nodes = image.nodes().to_vec();
    nodes.push(node);
    let graph = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    let original = encode(&graph).unwrap();
    let key_bytes = encode(&key).unwrap();
    assert_eq!(&key_bytes[..3], &[0xa2, 1, 6]);
    let positions = original
        .windows(key_bytes.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == key_bytes).then_some(position))
        .collect::<Vec<_>>();
    let [position] = positions.as_slice() else {
        panic!("the graph must contain the exact object-definition key once");
    };
    let mut forged = original.clone();
    forged[position + 2] = 10;
    let resolve = |bytes: &[u8], meter: &mut BudgetMeter| {
        let decoded: crate::DecodedStrongDigestFinalizationPlanV1 =
            decode_canonical(bytes, DecodeLimits::default()).unwrap();
        decoded.resolve_foundation(&foundation, meter)
    };
    let mut normal = BudgetMeter::new(DecodeLimits::default());
    resolve(&original, &mut normal).unwrap();
    let mut invalid = BudgetMeter::new(DecodeLimits::default());
    assert!(matches!(
        resolve(&forged, &mut invalid),
        Err(crate::StrongDigestPlanReplayError::Validation(_))
    ));
    assert_eq!(
        normal.usage().validation_work_units,
        invalid.usage().validation_work_units
    );
    let mut cumulative = BudgetMeter::new(DecodeLimits {
        validation_work_units: normal.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    resolve(&original, &mut cumulative).unwrap();
    assert!(matches!(
        resolve(&original, &mut cumulative),
        Err(crate::StrongDigestPlanReplayError::Resource(_))
    ));
}
