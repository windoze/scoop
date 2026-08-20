//! Untrusted callables registration carriers.

use super::*;

#[derive(Debug)]
pub struct DecodedStrongSafepointRegistrationPlanV1 {
    pub(in crate::production::registration_production) site:
        DecodedPersistentId<PersistentSafepointSiteId>,
    pub(in crate::production::registration_production) safepoint: u64,
    pub(in crate::production::registration_production) owner:
        DecodedPersistentId<PersistentCallableBodyId>,
    pub(in crate::production::registration_production) role: u32,
    pub(in crate::production::registration_production) root_pair_count: u32,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    normalized_stackmap_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    normalized_stackmap_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongSafepointRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encode_field(encoder, 1, &self.site)?;
        encode_unsigned_field(encoder, 2, self.safepoint)?;
        encode_field(encoder, 3, &self.owner)?;
        encode_unsigned_field(encoder, 4, u64::from(self.role))?;
        encode_unsigned_field(encoder, 5, u64::from(self.root_pair_count))?;
        encode_field(encoder, 6, &self.symbol)?;
        encode_field(encoder, 7, &self.definition_plan)?;
        encode_field(encoder, 8, &self.primary_atom)?;
        encode_field(encoder, 10, &self.normalized_stackmap_fingerprint_node)?;
        encode_field(encoder, 12, &self.normalized_stackmap_patch)
    }
}

impl WireDecode for DecodedStrongSafepointRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            site: decoder.field(1, DecodedPersistentId::decode)?,
            safepoint: decoder.field(2, Decoder::unsigned)?,
            owner: decoder.field(3, DecodedPersistentId::decode)?,
            role: decoder.field(4, Decoder::u32)?,
            root_pair_count: decoder.field(5, Decoder::u32)?,
            symbol: decoder.field(6, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(7, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(8, DecodedPersistentId::decode)?,
            normalized_stackmap_fingerprint_node: decoder.field(10, DecodedPersistentId::decode)?,
            normalized_stackmap_patch: decoder.field(12, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongCallableRegistrationPlanV1 {
    body: DecodedPersistentId<PersistentCallableBodyId>,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    entry_symbol: DecodedPersistentSymbolRequest,
    body_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    body_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,

    body_definition_node: DecodedPersistentId<DigestNodeId>,
    body_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongCallableRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encode_field(encoder, 1, &self.body)?;
        encode_field(encoder, 2, &self.symbol)?;
        encode_field(encoder, 3, &self.definition_plan)?;
        encode_field(encoder, 4, &self.primary_atom)?;
        encode_field(encoder, 5, &self.entry_symbol)?;
        encode_field(encoder, 6, &self.body_definition_plan)?;
        encode_field(encoder, 7, &self.body_primary_atom)?;
        encode_field(encoder, 9, &self.body_definition_node)?;
        encode_field(encoder, 12, &self.body_definition_patch)
    }
}

impl WireDecode for DecodedStrongCallableRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(3, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(4, DecodedPersistentId::decode)?,
            entry_symbol: decoder.field(5, DecodedPersistentSymbolRequest::decode)?,
            body_definition_plan: decoder.field(6, DecodedPersistentId::decode)?,
            body_primary_atom: decoder.field(7, DecodedPersistentId::decode)?,

            body_definition_node: decoder.field(9, DecodedPersistentId::decode)?,
            body_definition_patch: decoder.field(12, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub(in crate::production::registration_production) struct DecodedStrongCallableRuntimeScanAtomV1 {
    pub(in crate::production::registration_production) atom:
        DecodedPersistentId<ObjectDefinitionAtomId>,
    pub(in crate::production::registration_production) scan: DecodedRefScan,
}

impl WireEncode for DecodedStrongCallableRuntimeScanAtomV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encode_field(encoder, 1, &self.atom)?;
        encode_field(encoder, 2, &self.scan)
    }
}

impl WireDecode for DecodedStrongCallableRuntimeScanAtomV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            atom: decoder.field(1, DecodedPersistentId::decode)?,
            scan: decoder.field(2, DecodedRefScan::decode)?,
        })
    }
}

#[derive(Debug)]
pub(in crate::production::registration_production) struct DecodedStrongCallableRuntimeScanPlanV1 {
    pub(in crate::production::registration_production) context_keys:
        Vec<DecodedPersistentId<PersistentExactTypeId>>,
    pub(in crate::production::registration_production) body:
        DecodedPersistentId<PersistentCallableBodyId>,
    pub(in crate::production::registration_production) atoms:
        Vec<DecodedStrongCallableRuntimeScanAtomV1>,
}

impl WireEncode for DecodedStrongCallableRuntimeScanPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encode_field(encoder, 1, &self.body)?;
        encode_array_field(encoder, 2, &self.atoms)?;
        encode_array_field(encoder, 3, &self.context_keys)
    }
}

impl WireDecode for DecodedStrongCallableRuntimeScanPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            atoms: decode_array_field(decoder, 2, DecodedStrongCallableRuntimeScanAtomV1::decode)?,
            context_keys: decode_array_field(decoder, 3, DecodedPersistentId::decode)?,
        })
    }
}
