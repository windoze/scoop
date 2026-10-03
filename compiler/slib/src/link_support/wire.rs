use super::*;
use crate::link_object::DecodedStrongDefinitionOwnerV1;
use scoop_wire::{Decoder, Digest256, Encoder, WireDecode, WireEncode, WireError};

#[derive(Debug)]
pub struct DecodedLirLinkSupportSectionV1 {
    aliases: Vec<DecodedAlias>,
}

#[derive(Debug)]
struct DecodedAlias {
    contract: Digest256,
    owner: DecodedStrongDefinitionOwnerV1,
}

impl DecodedLirLinkSupportSectionV1 {
    pub fn read_link(
        &self,
        descriptors: &scoop_lir::CanonicalExactDescriptorExportsV1,
    ) -> Result<LirLinkSupportSectionV1, LinkSupportError> {
        if self.aliases.is_empty() {
            return Ok(LirLinkSupportSectionV1::default());
        }
        if self.aliases.len() != 1 {
            return Err(err("duplicate or unknown runtime data alias"));
        }
        let alias = &self.aliases[0];
        if alias.contract.as_array() != string_contract(descriptors.target())?.as_array() {
            return Err(err("unknown runtime data alias contract"));
        }
        let exact = alias
            .owner
            .exact_descriptor()
            .ok_or_else(|| err("runtime String alias target is not a TypeDescriptor owner"))?;
        let descriptor = descriptors
            .records()
            .iter()
            .find(|descriptor| descriptor.exact().as_array() == exact.as_array())
            .ok_or_else(|| err("runtime String alias target is not defined by this Cone"))?;
        LirLinkSupportSectionV1::from_string_descriptor(Some(descriptor))
    }
}

impl WireEncode for RuntimeDataAliasV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        self.contract.encode(e)?;
        e.field(2)?;
        self.owner.encode(e)
    }
}

impl WireEncode for LirLinkSupportSectionV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(1)?;
        e.field(1)?;
        e.array(self.aliases.len() as u64)?;
        for alias in &self.aliases {
            alias.encode(e)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedLirLinkSupportSectionV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(1)?;
        let aliases = d.field(1, |d| {
            d.decode_array(|d, _| {
                d.expect_map(2)?;
                Ok(DecodedAlias {
                    contract: d.field(1, Digest256::decode)?,
                    owner: d.field(2, DecodedStrongDefinitionOwnerV1::decode)?,
                })
            })
        })?;
        Ok(Self { aliases })
    }
}

impl WireEncode for DecodedLirLinkSupportSectionV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(1)?;
        e.field(1)?;
        e.array(self.aliases.len() as u64)?;
        for alias in &self.aliases {
            e.map(2)?;
            e.field(1)?;
            alias.contract.encode(e)?;
            e.field(2)?;
            alias.owner.encode(e)?;
        }
        Ok(())
    }
}
