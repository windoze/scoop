use crate::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Debug, Eq, PartialEq)]
struct ProbeDocument {
    tag: u64,
    payload: Vec<u8>,
    entries: Vec<u64>,
}

impl WireDecode for ProbeDocument {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            tag: decoder.field(1, Decoder::unsigned)?,
            payload: decoder.field(2, Decoder::owned_bytes)?,
            entries: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| decoder.unsigned())
            })?,
        })
    }
}

impl WireEncode for ProbeDocument {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), super::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(self.tag)?;
        encoder.field(2)?;
        encoder.bytes(&self.payload)?;
        encoder.field(3)?;
        encoder.array(self.entries.len() as u64)?;
        for entry in &self.entries {
            encoder.unsigned(*entry)?;
        }
        Ok(())
    }
}

#[test]
fn arbitrary_byte_corpus_is_panic_free_and_deterministic() {
    for bytes in arbitrary_byte_corpus() {
        let first = super::decode_canonical::<ProbeDocument>(&bytes);
        let second = super::decode_canonical::<ProbeDocument>(&bytes);
        assert_eq!(first, second, "nondeterministic decode for {bytes:02x?}");
    }
}

fn arbitrary_byte_corpus() -> Vec<Vec<u8>> {
    let mut corpus = vec![
        Vec::new(),
        vec![0],
        vec![0xff],
        vec![0xa3, 1, 0, 2, 0x40, 3, 0x80],
        vec![0xbf, 1, 0, 2, 0x40, 3, 0x80, 0xff],
    ];
    for head in 0_u8..=u8::MAX {
        corpus.push(vec![head]);
        corpus.push(vec![head, 0, 0xff, head.rotate_left(1)]);
    }

    let mut state = 0x8f62_d4a9_731c_b5e1_u64;
    for length in 0..=512 {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.push(state as u8);
        }
        corpus.push(bytes);
    }
    corpus
}
