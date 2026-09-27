use std::collections::BTreeMap;

use super::*;
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

impl WireEncode for CanonicalCallableLirDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

impl WireEncode for CanonicalCallableLirDefinitionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.definitions.len() as u64)?;
        for definition in &self.definitions {
            definition.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCanonicalCallableLirDefinitionsV1 {
    definitions: Vec<DecodedDefinition>,
}

#[derive(Debug)]
struct DecodedDefinition {
    body: DecodedPersistentId<PersistentCallableBodyId>,
    fingerprint: Digest256,
}

impl WireEncode for DecodedDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

impl WireDecode for DecodedDefinition {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            fingerprint: decoder.field(2, Digest256::decode)?,
        })
    }
}

impl WireEncode for DecodedCanonicalCallableLirDefinitionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.definitions.len() as u64)?;
        for definition in &self.definitions {
            definition.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalCallableLirDefinitionsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(Self {
            definitions: decoder.decode_array(|decoder, _| DecodedDefinition::decode(decoder))?,
        })
    }
}

impl DecodedCanonicalCallableLirDefinitionsV1 {
    pub fn validate(
        self,
        foundation: &ConeLirFoundation,
    ) -> Result<CanonicalCallableLirDefinitionsV1, CanonicalCallableLirError> {
        let known = function_bodies(foundation)
            .map(|body| (*body.as_array(), body))
            .collect::<BTreeMap<_, _>>();
        if self
            .definitions
            .windows(2)
            .any(|pair| pair[0].body.as_array() >= pair[1].body.as_array())
        {
            return Err(CanonicalCallableLirError::UnsortedBodies);
        }
        let definitions = self
            .definitions
            .into_iter()
            .map(|definition| {
                let body = known.get(definition.body.as_array()).copied().ok_or(
                    CanonicalCallableLirError::UnknownBody(*definition.body.as_array()),
                )?;
                Ok(CanonicalCallableLirDefinitionV1::new(
                    body,
                    definition.fingerprint,
                ))
            })
            .collect::<Result<Vec<_>, CanonicalCallableLirError>>()?;
        CanonicalCallableLirDefinitionsV1::new(definitions, foundation)
    }
}
