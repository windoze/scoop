use super::*;
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

impl WireEncode for CanonicalShapeLirDefinitionsV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.definitions.len() as u64)?;
        for definition in &self.definitions {
            encode_record(
                e,
                &definition.member(),
                definition.fingerprint,
                definition.abi,
            )?;
        }
        Ok(())
    }
}

fn encode_record(
    e: &mut Encoder,
    member: &impl WireEncode,
    fingerprint: Digest256,
    abi: Digest256,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(3)?;
    e.field(1)?;
    member.encode(e)?;
    e.field(2)?;
    fingerprint.encode(e)?;
    e.field(3)?;
    abi.encode(e)
}

#[derive(Debug)]
pub struct DecodedCanonicalShapeLirDefinitionsV1 {
    definitions: Vec<(DecodedPersistentId<OdrMemberId>, Digest256, Digest256)>,
}

impl WireDecode for DecodedCanonicalShapeLirDefinitionsV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let definitions = d.decode_array(|d, _| {
            d.expect_map(3)?;
            Ok((
                d.field(1, DecodedPersistentId::decode)?,
                d.field(2, Digest256::decode)?,
                d.field(3, Digest256::decode)?,
            ))
        })?;
        Ok(Self { definitions })
    }
}

impl WireEncode for DecodedCanonicalShapeLirDefinitionsV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.definitions.len() as u64)?;
        for (member, fingerprint, abi) in &self.definitions {
            encode_record(e, member, *fingerprint, *abi)?;
        }
        Ok(())
    }
}

impl DecodedCanonicalShapeLirDefinitionsV1 {
    pub fn validate(
        self,
        foundation: &ConeLirFoundation,
    ) -> Result<CanonicalShapeLirDefinitionsV1, CanonicalShapeLirError> {
        let known = identities(foundation)?;
        let ids = known
            .keys()
            .map(|id| (*id.as_array(), *id))
            .collect::<BTreeMap<_, _>>();
        let records = self
            .definitions
            .into_iter()
            .map(|(member, lir, abi)| {
                let id = ids
                    .get(member.as_array())
                    .copied()
                    .ok_or(CanonicalShapeLirError::UnknownMember(*member.as_array()))?;
                Ok((id, lir, abi))
            })
            .collect::<Result<_, CanonicalShapeLirError>>()?;
        CanonicalShapeLirDefinitionsV1::from_canonical(records, known)
    }
}
