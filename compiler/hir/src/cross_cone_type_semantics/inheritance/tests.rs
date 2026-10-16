use scoop_identity::SourceNominalKind;
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::cross_cone_type_semantics::wire;

pub(in crate::cross_cone_type_semantics) mod support;
use support::Fixture;

#[test]
fn inheritance_edge_wire_is_closed_and_reader_never_repairs_duplicate_interfaces() {
    for (modality, tag) in [
        (NominalInheritanceModalityV1::Final, 1),
        (NominalInheritanceModalityV1::Open, 2),
        (NominalInheritanceModalityV1::Abstract, 3),
        (NominalInheritanceModalityV1::Interface, 4),
    ] {
        assert_eq!(encode(&modality).unwrap(), [0xa1, 0, tag]);
        assert_eq!(
            decode_canonical::<NominalInheritanceModalityV1>(&[0xa1, 0, tag]).unwrap(),
            modality
        );
    }
    assert!(decode_canonical::<NominalInheritanceModalityV1>(&[0xa1, 0, 5]).is_err());
    assert_eq!(
        encode(&DirectClassBaseV1::NoClassBase).unwrap(),
        [0xa1, 0, 1]
    );
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let interface = fixture.add("I", SourceNominalKind::Interface, &[]);
    let record = NominalInheritanceEdgesV1::try_new(
        base.exact,
        NominalInheritanceModalityV1::Open,
        DirectClassBaseV1::ClassBase { exact: base.exact },
        vec![interface.exact],
    )
    .unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedNominalInheritanceEdgesV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
    let bad = encode(&BadInterfaces(base.exact, interface.exact)).unwrap();
    let decoded: DecodedNominalInheritanceEdgesV1 = decode_canonical(&bad).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(InheritanceEdgeResolutionError::Order(_))
    ));
}

struct BadInterfaces(
    scoop_identity::PersistentExactTypeId,
    scoop_identity::PersistentExactTypeId,
);
impl WireEncode for BadInterfaces {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.0.encode(encoder)?;
        encoder.field(2)?;
        NominalInheritanceModalityV1::Open.encode(encoder)?;
        encoder.field(3)?;
        DirectClassBaseV1::NoClassBase.encode(encoder)?;
        encoder.field(4)?;
        wire::sequence(encoder, &[self.1, self.1])
    }
}
