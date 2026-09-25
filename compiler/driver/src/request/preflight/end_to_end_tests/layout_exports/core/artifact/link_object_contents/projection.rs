use super::*;

pub(super) fn mutate(payload: &[u8], failure: Failure) -> Vec<u8> {
    let mut budget = meter();
    let mut decoder = scoop_wire::Decoder::new(payload, &mut budget).unwrap();
    decoder.expect_map(8).unwrap();
    decoder
        .field(1, |d| {
            for _ in 0..d.array()? {
                d.expect_map(3)?;
                d.field(0, |d| d.unsigned())?;
                d.field(1, |d| {
                    d.bytes()?;
                    Ok(())
                })?;
                d.field(2, |d| {
                    for _ in 0..d.array()? {
                        d.bytes()?;
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })
        .unwrap();
    let mut atom = None;
    decoder
        .field(2, |d| {
            for _ in 0..d.array()? {
                d.expect_map(5)?;
                for field in 1..=3 {
                    d.field(field, |d| {
                        d.bytes()?;
                        Ok(())
                    })?;
                }
                d.field(4, |d| d.unsigned())?;
                d.field(5, |d| {
                    for _ in 0..d.array()? {
                        d.expect_map(6)?;
                        d.field(1, |d| {
                            d.bytes()?;
                            Ok(())
                        })?;
                        for field in 2..=6 {
                            d.field(field, |d| {
                                let start = d.position() as usize;
                                let value = d.unsigned()?;
                                if field == 5 && atom.is_none() {
                                    atom = Some((start, d.position() as usize, value));
                                }
                                Ok(())
                            })?;
                        }
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })
        .unwrap();
    let mut bytes = payload.to_vec();
    match failure {
        Failure::AtomRange => {
            let (start, end, value) = atom.unwrap();
            bytes.splice(start..end, encode(&Unsigned(value + 1)).unwrap());
        }
        Failure::DigestIntent => {
            let offset = decoder
                .field(3, |d| {
                    assert!(d.array()? > 0);
                    d.expect_map(3)?;
                    d.field(1, |d| {
                        assert_eq!(d.bytes()?.len(), 32);
                        Ok(d.position() as usize - 1)
                    })
                })
                .unwrap();
            bytes[offset] ^= 1;
        }
        _ => panic!("expected a Link closure projection mutation"),
    }
    bytes
}

struct Unsigned(u64);
impl WireEncode for Unsigned {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.0)
    }
}
