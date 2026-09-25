use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::support::*;

#[test]
fn independent_template_keeps_all_twelve_fields_and_canonical_local_indices() {
    let f = Fixture::new();
    let value = template(&f);
    let bytes = encode(&value.index_locals().unwrap()).unwrap();
    assert_eq!(&bytes[..2], &[0xac, 1]);

    let input: DecodedProtectedDefaultTemplateV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&input).unwrap(), bytes);

    let result = input.resolve(&mut f.resolver()).unwrap();
    assert_eq!(result, value);

    assert_eq!(result.key().parameter_position(), 1);
    assert_eq!(result.locals().len_u32(), 2);
    assert_eq!(result.value_parameters().len_u32(), 1);
    assert!(result.receiver().receiver().is_some());
    assert_eq!(result.references().globals().len(), 1);
}

#[test]
fn wire_requires_exact_product_and_known_key_id() {
    let error = decode_canonical::<DecodedProtectedDefaultTemplateV1>(&[0xa0]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 12,
            actual: 0
        }
    );
    let f = Fixture::new();
    assert!(matches!(
        decoded(&template(&f)).resolve(&mut Resolver::rejecting()),
        Err(ProtectedDefaultTemplateResolutionError::Key(_))
    ));
}
