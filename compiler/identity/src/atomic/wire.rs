use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod expression;

macro_rules! atomic_wire {
    ($name:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant),+];
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.unsigned(match self { $(Self::$variant => $tag),+ })
            }
        }

        impl WireDecode for $name {
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
                match decoder.unsigned()? {
                    $($tag => Ok(Self::$variant),)+
                    tag => Err(WireError::new(
                        WireErrorKind::UnknownTag { tag },
                        decoder.path().clone(),
                        Some(decoder.position()),
                    )),
                }
            }
        }
    };
}

atomic_wire!(AtomicValueKind { Int = 0, Long = 1, Boolean = 2, Reference = 3 });
atomic_wire!(AtomicMemoryOrder {
    Relaxed = 0, Acquire = 1, Release = 2, AcqRel = 3, SeqCst = 4,
});
atomic_wire!(AtomicLoadOrder { Relaxed = 0, Acquire = 1, SeqCst = 2 });
atomic_wire!(AtomicStoreOrder { Relaxed = 0, Release = 1, SeqCst = 2 });
atomic_wire!(AtomicCompareExchangeOrder {
    Relaxed = 0, AcquireRelaxed = 1, Acquire = 2, Release = 3, AcqRelRelaxed = 4,
    AcqRelAcquire = 5, SeqCstRelaxed = 6, SeqCstAcquire = 7, SeqCst = 8,
});
atomic_wire!(AtomicRmwOperation {
    Exchange = 0, Add = 1, Subtract = 2, And = 3, Or = 4, Xor = 5,
});
atomic_wire!(AtomicCompareExchangeResult { ObservedValue = 0, Success = 1 });
