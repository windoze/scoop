use super::*;

pub(super) fn mutate(payload: &[u8], failure: Failure) -> Vec<u8> {
    let mut budget = meter();
    let mut decoder = scoop_wire::Decoder::new(payload, &mut budget).unwrap();
    decoder.expect_map(8).unwrap();
    let (head, content, end, count, records) = decoder
        .field(1, |decoder| {
            let head = decoder.position() as usize;
            let count = decoder.array()?;
            let content = decoder.position() as usize;
            let mut records = Vec::new();
            for _ in 0..count {
                let start = decoder.position() as usize;
                decoder.expect_map(3)?;
                let tag = decoder.field(0, |d| d.unsigned())?;
                let member = decoder.field(1, |d| {
                    assert_eq!(d.bytes()?.len(), 32);
                    Ok(d.position() as usize - 1)
                })?;
                let unit = decoder.field(2, |d| {
                    let count = d.array()?;
                    assert!(count > 0);
                    let mut first = 0;
                    for i in 0..count {
                        assert_eq!(d.bytes()?.len(), 32);
                        if i == 0 {
                            first = d.position() as usize - 1;
                        }
                    }
                    Ok(first)
                })?;
                records.push((tag, start, decoder.position() as usize, member, unit));
            }
            Ok((head, content, decoder.position() as usize, count, records))
        })
        .unwrap();
    let (_, start, record_end, member, unit) =
        records.into_iter().find(|record| record.0 == 1).unwrap();
    let mut bytes = payload.to_vec();
    match failure {
        Failure::Member => bytes[member] ^= 1,
        Failure::Unit => bytes[unit] ^= 1,
        Failure::Duplicate => {
            bytes = payload[..head].to_vec();
            bytes.extend(encode(&ArrayHead(count + 1)).unwrap());
            bytes.extend_from_slice(&payload[content..end]);
            bytes.extend_from_slice(&payload[start..record_end]);
            bytes.extend_from_slice(&payload[end..]);
        }
        Failure::MissingObject => panic!("directory mutation does not change the projection"),
    }
    bytes
}

struct ArrayHead(u64);
impl WireEncode for ArrayHead {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0)
    }
}
