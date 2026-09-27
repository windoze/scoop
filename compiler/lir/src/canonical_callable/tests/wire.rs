use super::support::*;
use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn canonical_body_records_round_trip_and_reject_missing_duplicate_or_unknown_bodies() {
    let mut module = scalar(false);
    module.functions.push(roots(false).functions.remove(0));
    let foundation = foundation(&module);
    let records = CanonicalCallableLirDefinitionsV1::from_module(&module, &foundation).unwrap();
    let bytes = encode(&records).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&bytes).unwrap();
    assert_eq!(decoded.validate(&foundation).unwrap(), records);

    let read = |definitions| {
        // Deliberately bypass the checked constructor to exercise the reader.
        let bytes = encode(&CanonicalCallableLirDefinitionsV1 { definitions }).unwrap();
        decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&bytes)
            .unwrap()
            .validate(&foundation)
    };
    assert!(matches!(
        read(Vec::new()),
        Err(CanonicalCallableLirError::BodySet { .. })
    ));
    let first = records.definitions()[0];
    assert!(matches!(
        read(vec![first, first]),
        Err(CanonicalCallableLirError::UnsortedBodies)
    ));
    let mut reversed = records.definitions().to_vec();
    reversed.reverse();
    assert!(matches!(
        read(reversed),
        Err(CanonicalCallableLirError::UnsortedBodies)
    ));
    let unknown = CanonicalCallableLirDefinitionV1::new(
        crate::tests::callable_body("foreignBody").id(),
        first.fingerprint(),
        CanonicalCallableDefinitionOwnerV1::Strong,
    );
    assert!(matches!(
        read(vec![unknown]),
        Err(CanonicalCallableLirError::UnknownBody(_))
    ));
}

#[test]
fn canonical_body_wire_requires_a_full_digest_and_closed_record() {
    let module = scalar(false);
    let records =
        CanonicalCallableLirDefinitionsV1::from_module(&module, &foundation(&module)).unwrap();
    let bytes = encode(&records).unwrap();
    assert_eq!(bytes[0], 0x81);
    assert_eq!(bytes[1], 0xa2);
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&truncated).is_err());
    for shape in [0xa1, 0xa3] {
        let mut changed = bytes.clone();
        changed[1] = shape;
        assert!(decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&changed).is_err());
    }
    let mut short_digest = bytes;
    let payload = short_digest.len() - 32;
    assert_eq!(short_digest[payload - 2..payload], [0x58, 32]);
    short_digest[payload - 1] = 31;
    short_digest.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&short_digest).is_err());
}

#[test]
fn odr_callable_wire_requires_abi_and_strong_wire_rejects_it() {
    let mut module = scalar(false);
    let foundation = odr_foundation(&mut module, OdrMemberRole::DispatchAdapter);
    let records = CanonicalCallableLirDefinitionsV1::from_module(&module, &foundation).unwrap();
    let bytes = encode(&records).unwrap();
    assert_eq!(bytes[1], 0xa3);
    let decoded = decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&bytes).unwrap();
    assert_eq!(decoded.validate(&foundation).unwrap(), records);

    let mut missing = bytes.clone();
    missing[1] = 0xa2;
    missing.truncate(missing.len() - 35);
    assert!(matches!(
        decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&missing)
            .unwrap()
            .validate(&foundation),
        Err(CanonicalCallableLirError::DefinitionOwner { .. })
    ));

    let strong = scalar(false);
    let strong_foundation = super::support::foundation(&strong);
    let records =
        CanonicalCallableLirDefinitionsV1::from_module(&strong, &strong_foundation).unwrap();
    let mut extra = encode(&records).unwrap();
    extra[1] = 0xa3;
    extra.extend_from_slice(&bytes[bytes.len() - 35..]);
    assert!(matches!(
        decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&extra)
            .unwrap()
            .validate(&strong_foundation),
        Err(CanonicalCallableLirError::DefinitionOwner { .. })
    ));

    let mut short = bytes;
    let length = short.len();
    short[length - 33] = 31;
    short.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableLirDefinitionsV1>(&short).is_err());
}
