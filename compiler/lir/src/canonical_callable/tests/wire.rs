use super::support::*;
use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn canonical_body_records_round_trip_and_reject_missing_duplicate_or_unknown_bodies() {
    let mut module = scalar(false);
    module.functions.push(roots(false).functions.remove(0));
    let foundation = foundation(&module);
    let records = CanonicalCallableAbisV1::from_module(&module, &foundation).unwrap();
    let bytes = encode(&records).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalCallableAbisV1>(&bytes).unwrap();
    assert_eq!(decoded.validate(&foundation).unwrap(), records);

    let read = |definitions| {
        // Deliberately bypass the checked constructor to exercise the reader.
        let bytes = encode(&CanonicalCallableAbisV1 { definitions }).unwrap();
        decode_canonical::<DecodedCanonicalCallableAbisV1>(&bytes)
            .unwrap()
            .validate(&foundation)
    };
    assert!(matches!(
        read(Vec::new()),
        Err(CanonicalCallableAbiError::BodySet { .. })
    ));
    let first = records.definitions()[0];
    assert!(matches!(
        read(vec![first, first]),
        Err(CanonicalCallableAbiError::UnsortedBodies)
    ));
    let mut reversed = records.definitions().to_vec();
    reversed.reverse();
    assert!(matches!(
        read(reversed),
        Err(CanonicalCallableAbiError::UnsortedBodies)
    ));
    let unknown = CanonicalCallableAbiV1::new(
        crate::tests::callable_body("foreignBody").id(),
        CanonicalCallableAbiOwnerV1::Strong,
    );
    assert!(matches!(
        read(vec![unknown]),
        Err(CanonicalCallableAbiError::UnknownBody(_))
    ));
}

#[test]
fn canonical_body_wire_requires_a_full_digest_and_closed_record() {
    let module = scalar(false);
    let records = CanonicalCallableAbisV1::from_module(&module, &foundation(&module)).unwrap();
    let bytes = encode(&records).unwrap();
    assert_eq!(bytes[0], 0x81);
    assert_eq!(bytes[1], 0xa1);
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableAbisV1>(&truncated).is_err());
    for shape in [0xa0, 0xa2] {
        let mut changed = bytes.clone();
        changed[1] = shape;
        assert!(decode_canonical::<DecodedCanonicalCallableAbisV1>(&changed).is_err());
    }
    let mut short_digest = bytes;
    let payload = short_digest.len() - 32;
    assert_eq!(short_digest[payload - 2..payload], [0x58, 32]);
    short_digest[payload - 1] = 31;
    short_digest.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableAbisV1>(&short_digest).is_err());
}

#[test]
fn odr_callable_wire_requires_abi_and_strong_wire_rejects_it() {
    let mut module = scalar(false);
    let foundation = odr_foundation(&mut module, OdrMemberRole::DispatchAdapter);
    let records = CanonicalCallableAbisV1::from_module(&module, &foundation).unwrap();
    let bytes = encode(&records).unwrap();
    assert_eq!(bytes[1], 0xa2);
    let decoded = decode_canonical::<DecodedCanonicalCallableAbisV1>(&bytes).unwrap();
    assert_eq!(decoded.validate(&foundation).unwrap(), records);

    let mut missing = bytes.clone();
    missing[1] = 0xa1;
    missing.truncate(missing.len() - 35);
    assert!(matches!(
        decode_canonical::<DecodedCanonicalCallableAbisV1>(&missing)
            .unwrap()
            .validate(&foundation),
        Err(CanonicalCallableAbiError::DefinitionOwner { .. })
    ));

    let strong = scalar(false);
    let strong_foundation = super::support::foundation(&strong);
    let records = CanonicalCallableAbisV1::from_module(&strong, &strong_foundation).unwrap();
    let mut extra = encode(&records).unwrap();
    extra[1] = 0xa2;
    extra.extend_from_slice(&bytes[bytes.len() - 35..]);
    assert!(matches!(
        decode_canonical::<DecodedCanonicalCallableAbisV1>(&extra)
            .unwrap()
            .validate(&strong_foundation),
        Err(CanonicalCallableAbiError::DefinitionOwner { .. })
    ));

    let mut short = bytes;
    let length = short.len();
    short[length - 33] = 31;
    short.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableAbisV1>(&short).is_err());
}
