use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for AtomicIntrinsic {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(0)?;
        self.family.encode(e)?;
        e.field(1)?;
        e.unsigned(match self.method {
            AtomicMethod::Load => 0,
            AtomicMethod::Store => 1,
            AtomicMethod::Exchange => 2,
            AtomicMethod::CompareAndSet => 3,
            AtomicMethod::CompareAndExchange => 4,
            AtomicMethod::FetchAdd => 5,
            AtomicMethod::FetchSub => 6,
            AtomicMethod::FetchAnd => 7,
            AtomicMethod::FetchOr => 8,
            AtomicMethod::FetchXor => 9,
        })
    }
}

impl WireDecode for AtomicIntrinsic {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        let family = d.field(0, AtomicValueKind::decode)?;
        d.field(1, |d| {
            let tag = d.unsigned()?;
            let method = match tag {
                0 => AtomicMethod::Load,
                1 => AtomicMethod::Store,
                2 => AtomicMethod::Exchange,
                3 => AtomicMethod::CompareAndSet,
                4 => AtomicMethod::CompareAndExchange,
                5 => AtomicMethod::FetchAdd,
                6 => AtomicMethod::FetchSub,
                7 => AtomicMethod::FetchAnd,
                8 => AtomicMethod::FetchOr,
                9 => AtomicMethod::FetchXor,
                _ => return Err(unknown(d, tag)),
            };
            Self::new(family, method).ok_or_else(|| unknown(d, tag))
        })
    }
}

fn unknown(d: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        d.path().clone(),
        Some(d.position()),
    )
}
