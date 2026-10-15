use super::support::*;
use super::*;
use scoop_wire::{decode_canonical, encode};

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
fn odr_callable_wire_preserves_abi_and_requires_full_digest() {
    let mut module = scalar(false);
    let foundation = odr_foundation(&mut module, OdrMemberRole::DispatchAdapter);
    let records = CanonicalCallableAbisV1::from_module(&module, &foundation).unwrap();
    let bytes = encode(&records).unwrap();
    assert_eq!(bytes[1], 0xa2);
    let decoded = decode_canonical::<DecodedCanonicalCallableAbisV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);

    let mut short = bytes;
    let length = short.len();
    short[length - 33] = 31;
    short.pop();
    assert!(decode_canonical::<DecodedCanonicalCallableAbisV1>(&short).is_err());
}
