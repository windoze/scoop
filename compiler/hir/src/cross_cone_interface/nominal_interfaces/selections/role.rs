//! Source-level table identity; interface applications retain nominal binders.

use scoop_identity::{DecodedSignatureTypeKey, SignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::NominalInterfaceRecordResolver;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NominalDispatchSelectionRoleV1 {
    ClassVtable,
    Interface { interface: SignatureTypeKey },
}

impl WireEncode for NominalDispatchSelectionRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClassVtable => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Interface { interface } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                interface.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedNominalDispatchSelectionRoleV1 {
    ClassVtable,
    Interface { interface: DecodedSignatureTypeKey },
}

impl DecodedNominalDispatchSelectionRoleV1 {
    pub(super) fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalDispatchSelectionRoleV1, E> {
        Ok(match self {
            Self::ClassVtable => NominalDispatchSelectionRoleV1::ClassVtable,
            Self::Interface { interface } => NominalDispatchSelectionRoleV1::Interface {
                interface: interface.resolve(resolver)?,
            },
        })
    }
}

impl WireEncode for DecodedNominalDispatchSelectionRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClassVtable => NominalDispatchSelectionRoleV1::ClassVtable.encode(encoder),
            Self::Interface { interface } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                interface.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedNominalDispatchSelectionRoleV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            tag => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(if tag == 1 {
            Self::ClassVtable
        } else {
            Self::Interface {
                interface: decoder.field(1, DecodedSignatureTypeKey::decode)?,
            }
        })
    }
}
