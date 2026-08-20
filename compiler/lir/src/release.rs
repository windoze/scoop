use super::*;

pub type ReleaseHookId = la_arena::Idx<ReleaseHook>;

/// An exact-type-owned machine body, outside ordinary callable lookup.
#[derive(Debug)]
pub struct ReleaseHook {
    pub owner: TypeDescriptorRef,
    pub code: Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReleasePolicy<H = scoop_identity::PersistentCallableBodyId> {
    #[default]
    None,
    SynchronousGcFree {
        hook: H,
    },
}

impl<H> ReleasePolicy<H> {
    pub const fn hook(&self) -> Option<&H> {
        match self {
            Self::None => None,
            Self::SynchronousGcFree { hook } => Some(hook),
        }
    }
}

impl<H: scoop_wire::WireEncode> scoop_wire::WireEncode for ReleasePolicy<H> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.hook().is_some() { 2 } else { 1 })?;
        encoder.field(0)?;
        match self {
            Self::None => encoder.unsigned(1),
            Self::SynchronousGcFree { hook } => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                hook.encode(encoder)
            }
        }
    }
}

impl<H: scoop_wire::WireDecode> scoop_wire::WireDecode for ReleasePolicy<H> {
    fn decode(decoder: &mut scoop_wire::Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, scoop_wire::Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            tag => {
                return Err(scoop_wire::WireError::new(
                    scoop_wire::WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if fields != expected {
            return Err(scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        if tag == 1 {
            Ok(Self::None)
        } else {
            Ok(Self::SynchronousGcFree {
                hook: decoder.field(1, H::decode)?,
            })
        }
    }
}
