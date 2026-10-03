use std::collections::BTreeMap;

use super::*;
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for CanonicalCallableLirDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let abi = match self.owner {
            CanonicalCallableDefinitionOwnerV1::Strong => None,
            CanonicalCallableDefinitionOwnerV1::Odr { abi, .. } => Some(abi),
        };
        encoder.map(if abi.is_some() { 3 } else { 2 })?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)?;
        if let Some(abi) = abi {
            encoder.field(3)?;
            abi.encode(encoder)?;
        }
        Ok(())
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
    odr_abi: Option<Digest256>,
}

impl WireEncode for DecodedDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.odr_abi.is_some() { 3 } else { 2 })?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)?;
        if let Some(abi) = self.odr_abi {
            encoder.field(3)?;
            abi.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedDefinition {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if !matches!(fields, 2 | 3) {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            fingerprint: decoder.field(2, Digest256::decode)?,
            odr_abi: if fields == 3 {
                Some(decoder.field(3, Digest256::decode)?)
            } else {
                None
            },
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
        let known = foundation
            .callable_bodies()
            .iter()
            .map(|record| (*record.id().as_array(), record))
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
                let record = known.get(definition.body.as_array()).copied().ok_or(
                    CanonicalCallableLirError::UnknownBody(*definition.body.as_array()),
                )?;
                let body = record.id();
                let owner = match (body_odr_member(record), definition.odr_abi) {
                    (None, None) => CanonicalCallableDefinitionOwnerV1::Strong,
                    (Some((group, member, role)), Some(abi)) => {
                        CanonicalCallableDefinitionOwnerV1::Odr {
                            group,
                            member,
                            role,
                            abi,
                        }
                    }
                    _ => return Err(CanonicalCallableLirError::DefinitionOwner { body }),
                };
                Ok(CanonicalCallableLirDefinitionV1::new(
                    body,
                    definition.fingerprint,
                    owner,
                ))
            })
            .collect::<Result<Vec<_>, CanonicalCallableLirError>>()?;
        CanonicalCallableLirDefinitionsV1::new(definitions, foundation)
    }
}
