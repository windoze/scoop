use scoop_identity::{Effect, SignatureCallableShape, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

use super::*;

#[test]
fn compiler_protocol_surface_is_a_closed_nine_field_product() {
    for bytes in [vec![0xa7], vec![0xa8], vec![0xaa, 0x0a, 0x00]] {
        assert!(decode_canonical::<DecodedCoreCompilerProtocolSurfaceV1>(&bytes,).is_err());
    }

    let (surface, foundation) = test_support::standalone();
    let bytes = encode(&surface).unwrap();
    assert_eq!(bytes[0], 0xa9);
    let mut retired = bytes.clone();
    retired[0] = 0xaa;
    retired.extend([0x0a, 0x80]);
    let error = decode_canonical::<DecodedCoreCompilerProtocolSurfaceV1>(&retired).unwrap_err();
    assert!(matches!(
        error.kind(),
        scoop_wire::WireErrorKind::InvalidLength {
            expected: 9,
            actual: 10
        }
    ));
    let decoded: DecodedCoreCompilerProtocolSurfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.validate_against(&foundation), Ok(surface));
}

fn decode(surface: &CoreCompilerProtocolSurfaceV1) -> DecodedCoreCompilerProtocolSurfaceV1 {
    decode_canonical(&encode(surface).unwrap()).unwrap()
}

mod providers;

mod shared_intrinsics;
