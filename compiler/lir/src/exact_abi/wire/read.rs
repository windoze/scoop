use scoop_identity::{DecodedCallableDefinitionOwner, DecodedCanonicalScoopAbiFunctionSignature};
use scoop_wire::{Decoder, WireDecode, WireErrorKind};

use super::*;

mod definition;

#[derive(Debug)]
pub struct DecodedExactCallableAbiExportV1 {
    target: DecodedCallableDefinitionOwner,
    signature: DecodedCanonicalScoopAbiFunctionSignature,
    convention: crate::CallingConvention,
    protocol: ExactCallableProtocolV1,
    definition: crate::production::DecodedStrongShapeDefinitionV1<PersistentCallableBodyId>,
}

#[derive(Debug)]
pub struct DecodedCanonicalExactCallableAbiExportsV1 {
    records: Vec<DecodedExactCallableAbiExportV1>,
}

impl DecodedExactCallableAbiExportV1 {
    pub fn read(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<ExactCallableAbiExportV1, crate::LinkDataError> {
        use crate::link_data::link_error;
        let owner = self.target.resolve(identities).map_err(link_error)?;
        let signature = self.signature.resolve(identities).map_err(link_error)?;
        if self.convention != crate::CallingConvention::Cdecl
            || self.protocol.gc_effect() != signature.gc_effect()
        {
            return Err(link_error(
                "callable ABI protocol does not match its signature",
            ));
        }
        let definition = self.definition.read(identities)?;
        let physical = definition::read(owner, definition, foundation, identities)?;
        Ok(ExactCallableAbiExportV1(Arc::new(CallableAbiBodyV1 {
            target: owner,
            target_profile: target,
            signature,
            protocol: self.protocol,
            physical,
            definition,
        })))
    }
}

impl DecodedCanonicalExactCallableAbiExportsV1 {
    pub fn read(
        self,
        target: crate::LirTargetProfile,
        foundation: &crate::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<CanonicalExactCallableAbiExportsV1, crate::LinkDataError> {
        let records = self
            .records
            .into_iter()
            .map(|record| record.read(target, foundation, identities))
            .collect::<Result<Vec<_>, _>>()?;
        if records
            .windows(2)
            .any(|pair| pair[0].target() >= pair[1].target())
        {
            return Err(crate::LinkDataError(
                "noncanonical callable ABI table order".into(),
            ));
        }
        Ok(CanonicalExactCallableAbiExportsV1::from_saved(
            target,
            foundation.producer(),
            records,
        ))
    }
}

impl WireDecode for ExactCallableProtocolV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::OrdinaryManaged),
            2 => Ok(Self::OrdinaryNoGc),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

impl WireDecode for DecodedExactCallableAbiExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            target: decoder.field(1, DecodedCallableDefinitionOwner::decode)?,
            signature: decoder.field(2, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            convention: decoder.field(3, crate::CallingConvention::decode)?,
            protocol: decoder.field(4, ExactCallableProtocolV1::decode)?,
            definition: decoder
                .field(6, crate::production::DecodedStrongShapeDefinitionV1::decode)?,
        })
    }
}

impl WireDecode for DecodedCanonicalExactCallableAbiExportsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactCallableAbiExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedExactCallableAbiExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)?;
        encoder.field(3)?;
        self.convention.encode(encoder)?;
        encoder.field(4)?;
        self.protocol.encode(encoder)?;
        encoder.field(6)?;
        self.definition.encode(encoder)
    }
}

impl WireEncode for DecodedCanonicalExactCallableAbiExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
