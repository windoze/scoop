use super::*;
use crate::link_object::DecodedStrongDefinitionOwnerV1;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Debug)]
pub struct DecodedLirLinkSupportSectionV1 {
    aliases: Vec<DecodedAlias>,
}

#[derive(Debug)]
struct DecodedAlias {
    contract: RuntimeSymbolContractId,
    owner: DecodedStrongDefinitionOwnerV1,
}

impl DecodedLirLinkSupportSectionV1 {
    pub fn read_link(
        &self,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<LirLinkSupportSectionV1, LinkSupportError> {
        let aliases = self
            .aliases
            .iter()
            .map(|alias| {
                Ok(RuntimeDataAliasV1 {
                    contract: alias.contract,
                    owner: alias.owner.resolve(identities)?,
                })
            })
            .collect::<Result<Vec<_>, LinkSupportError>>()?;
        Ok(LirLinkSupportSectionV1 { aliases })
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
                    contract: d.field(1, RuntimeSymbolContractId::decode)?,
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
