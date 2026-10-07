use std::collections::BTreeMap;

use super::*;
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for CanonicalCallableAbiV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let abi = match self.owner {
            CanonicalCallableAbiOwnerV1::Strong => None,
            CanonicalCallableAbiOwnerV1::Odr { abi, .. } => Some(abi),
        };
        encoder.map(if abi.is_some() { 2 } else { 1 })?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        if let Some(abi) = abi {
            encoder.field(3)?;
            abi.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalCallableAbisV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.definitions.len() as u64)?;
        for definition in &self.definitions {
            definition.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCanonicalCallableAbisV1 {
    definitions: Vec<DecodedDefinition>,
}

#[derive(Debug)]
struct DecodedDefinition {
    body: DecodedPersistentId<PersistentCallableBodyId>,
    odr_abi: Option<Digest256>,
}

impl WireEncode for DecodedDefinition {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.odr_abi.is_some() { 2 } else { 1 })?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
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
        if !matches!(fields, 1 | 2) {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 1,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            odr_abi: if fields == 2 {
                Some(decoder.field(3, Digest256::decode)?)
            } else {
                None
            },
        })
    }
}

impl WireEncode for DecodedCanonicalCallableAbisV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.definitions.len() as u64)?;
        for definition in &self.definitions {
            definition.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalCallableAbisV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(Self {
            definitions: decoder.decode_array(|decoder, _| DecodedDefinition::decode(decoder))?,
        })
    }
}

impl DecodedCanonicalCallableAbisV1 {
    pub fn validate(
        self,
        foundation: &ConeLirFoundation,
    ) -> Result<CanonicalCallableAbisV1, CanonicalCallableAbiError> {
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
            return Err(CanonicalCallableAbiError::UnsortedBodies);
        }
        let definitions = self
            .definitions
            .into_iter()
            .map(|definition| {
                let record = known.get(definition.body.as_array()).copied().ok_or(
                    CanonicalCallableAbiError::UnknownBody(*definition.body.as_array()),
                )?;
                let body = record.id();
                let owner = match (body_odr_member(record, foundation), definition.odr_abi) {
                    (None, None) => CanonicalCallableAbiOwnerV1::Strong,
                    (Some((group, member, role)), Some(abi)) => CanonicalCallableAbiOwnerV1::Odr {
                        group,
                        member,
                        role,
                        abi,
                    },
                    _ => return Err(CanonicalCallableAbiError::DefinitionOwner { body }),
                };
                Ok(CanonicalCallableAbiV1::new(body, owner))
            })
            .collect::<Result<Vec<_>, CanonicalCallableAbiError>>()?;
        CanonicalCallableAbisV1::new(definitions, foundation)
    }
}
