use scoop_identity::{DecodedLayoutKey, ValidatedIdentityGraph};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{DecodedValueStorageLayoutV1, LinkDataError, link_data::link_error};

/// Inline storage has a typed key, but does not need its own exported symbol.
#[derive(Debug)]
pub(crate) struct DecodedValueLayoutConstituentV1 {
    key: DecodedLayoutKey,
    storage: DecodedValueStorageLayoutV1,
}

impl DecodedValueLayoutConstituentV1 {
    pub(crate) fn read(
        self,
        target: LirTargetProfile,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ValueLayoutConstituentV1, LinkDataError> {
        let key = self.key.resolve(identities).map_err(link_error)?;
        let storage = self.storage.validate(target).map_err(link_error)?;
        ValueLayoutConstituentV1::new(target, key, storage).map_err(link_error)
    }
}

impl WireDecode for DecodedValueLayoutConstituentV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            key: decoder.field(1, DecodedLayoutKey::decode)?,
            storage: decoder.field(2, DecodedValueStorageLayoutV1::decode)?,
        })
    }
}

fn encode(
    encoder: &mut Encoder,
    key: &impl WireEncode,
    storage: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    key.encode(encoder)?;
    encoder.field(2)?;
    storage.encode(encoder)
}

impl WireEncode for DecodedValueLayoutConstituentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.key, &self.storage)
    }
}

impl WireEncode for ValueLayoutConstituentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.key, self.storage())
    }
}

impl WireEncode for NonZeroValueLayoutRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(
            encoder,
            &self.key,
            &ValueStorageLayoutV1(StorageBody::Inline(Arc::clone(&self.storage))),
        )
    }
}
