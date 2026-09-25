use super::*;
use scoop_wire::{Encoder, WireEncode};

struct Records<'a>(&'a [hir::DefaultSourceProfileV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(e)?;
        }
        Ok(())
    }
}

pub(super) fn check(output: &hir::DependencyHirOutput, source: &Production) {
    let profiles = source.profiles();
    let mut reversed = profiles.records().to_vec();
    reversed.reverse();
    for records in [reversed, vec![profiles.records()[0], profiles.records()[0]]] {
        let decoded: DecodedProfiles =
            decode_canonical(&encode(&Records(&records)).unwrap()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut identity_closure(output)),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
    }
    let one = encode(&Records(&profiles.records()[..1])).unwrap();
    for tag in [0, 3, 23] {
        let mut bytes = one.clone();
        *bytes.last_mut().unwrap() = tag;
        assert!(decode_canonical::<DecodedProfiles>(&bytes).is_err());
    }
    for fields in [0xa1, 0xa3] {
        let mut bytes = one.clone();
        assert_eq!(bytes[1], 0xa2);
        bytes[1] = fields;
        assert!(decode_canonical::<DecodedProfiles>(&bytes).is_err());
    }
    let mut empty_graph = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    let decoded: DecodedProfiles = decode_canonical(&one).unwrap();
    assert!(decoded.resolve(&mut empty_graph).is_err());

    let empty = Profiles::default();
    assert_eq!(encode(&restore(output, &empty)).unwrap(), [0x80]);
    empty.validate_template_coverage(&Table::default()).unwrap();
}
