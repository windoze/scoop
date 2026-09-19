use scoop_wire::{
    BudgetMeter, DecodeLimits, WireErrorKind, decode_canonical, decode_canonical_with_meter, encode,
};

use super::support::*;

#[test]
fn independent_template_keeps_all_twelve_fields_and_canonical_local_indices() {
    let f = Fixture::new();
    let value = template(&f);
    let bytes = encode(&value.index_locals().unwrap()).unwrap();
    assert_eq!(&bytes[..2], &[0xac, 1]);
    let mut shared = meter();
    let input: DecodedProtectedDefaultTemplateV1 =
        decode_canonical_with_meter(&bytes, &mut shared).unwrap();
    assert_eq!(encode(&input).unwrap(), bytes);
    let before = shared.usage();
    let result = input.resolve(&mut f.resolver(), &mut shared).unwrap();
    assert_eq!(result, value);
    assert!(shared.usage().decoded_nodes > before.decoded_nodes);
    assert!(shared.usage().validation_work_units > before.validation_work_units);
    assert_eq!(result.key().parameter_position(), 1);
    assert_eq!(result.locals().len_u32(), 2);
    assert_eq!(result.value_parameters().len_u32(), 1);
    assert!(result.receiver().receiver().is_some());
    assert_eq!(result.references().globals().len(), 1);
}

#[test]
fn wire_requires_exact_product_and_known_key_id() {
    let error =
        decode_canonical::<DecodedProtectedDefaultTemplateV1>(&[0xa0], DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 12,
            actual: 0
        }
    );
    let f = Fixture::new();
    assert!(matches!(
        decoded(&template(&f)).resolve(&mut Resolver::rejecting(), &mut meter()),
        Err(ProtectedDefaultTemplateResolutionError::Key(_))
    ));
}

#[test]
fn reader_uses_one_budget_for_body_locals_signatures_and_intrinsic_projection() {
    let f = Fixture::new();
    let value = template(&f);
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_edges: 0,
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
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decoded(&value).resolve(&mut f.resolver(), &mut BudgetMeter::new(limits)),
            Err(ProtectedDefaultTemplateResolutionError::Resource(_))
        ));
    }
    let mut unlimited = meter();
    decoded(&value)
        .resolve(&mut f.resolver(), &mut unlimited)
        .unwrap();
    let mut bounded = BudgetMeter::new(DecodeLimits {
        decoded_nodes: unlimited.usage().decoded_nodes,
        ..DecodeLimits::default()
    });
    decoded(&value)
        .resolve(&mut f.resolver(), &mut bounded)
        .unwrap();
    assert!(matches!(
        decoded(&value).resolve(&mut f.resolver(), &mut bounded),
        Err(ProtectedDefaultTemplateResolutionError::Resource(_))
    ));
}
