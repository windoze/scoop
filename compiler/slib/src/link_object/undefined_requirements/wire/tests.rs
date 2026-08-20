use super::*;

#[derive(Debug, Eq, PartialEq)]
struct Form(VerifiedObjectRelocationFormV1);
impl WireEncode for Form {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_relocation_form(encoder, self.0)
    }
}
impl WireDecode for Form {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_relocation_form(decoder).map(Self)
    }
}

#[test]
fn elf_rela_wire_preserves_signed_addends_and_has_a_distinct_tag() {
    for addend in [i64::MIN, -4, 0, 17, i64::MAX] {
        let value = Form(VerifiedObjectRelocationFormV1::ElfRela {
            kind: object::elf::R_X86_64_PLT32,
            width: 4,
            addend,
        });
        let bytes = scoop_wire::encode(&value).unwrap();
        assert_eq!(scoop_wire::decode_canonical::<Form>(&bytes).unwrap(), value);
        if addend == -4 {
            assert_eq!(
                bytes,
                [
                    0xa4, 0, 11, 1, 4, 2, 4, 3, 0x1b, 255, 255, 255, 255, 255, 255, 255, 252
                ]
            );
        }
    }
    assert_eq!(
        scoop_wire::encode(&Form(VerifiedObjectRelocationFormV1::Unsigned64)).unwrap(),
        [0xa1, 0, 1]
    );
    let invalid = Form(VerifiedObjectRelocationFormV1::ElfRela {
        kind: 4,
        width: 3,
        addend: 0,
    });
    assert!(scoop_wire::decode_canonical::<Form>(&scoop_wire::encode(&invalid).unwrap()).is_err());
}
