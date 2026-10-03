use super::*;

pub type DecodedStrongTypeVtableSemanticPlanV1 =
    DecodedStrongTypeVtableSemanticPlan<DecodedStrongTypeDispatchCallableRefV1>;
pub type DecodedStrongTypeItableSemanticPlanV1 = DecodedStrongTypeItableSemanticPlan<
    DecodedStrongTypeDescriptorRefV1,
    DecodedStrongTypeDispatchCallableRefV1,
>;
pub type DecodedStrongTypeRegistrationPlanV1 = DecodedStrongTypeRegistrationPlan<
    DecodedOptionalStrongTypeDescriptorRefV1,
    DecodedStrongTypeDescriptorRefV1,
    DecodedStrongTypeDispatchCallableRefV1,
>;
pub type DecodedStrongTypeRegistrationPlanV2 = DecodedStrongTypeRegistrationPlan<
    crate::DecodedOptionalStrongTypeDescriptorRefV2,
    crate::DecodedStrongTypeDescriptorRefV2,
    crate::DecodedStrongTypeDispatchCallableRefV2,
>;

#[derive(Debug)]
pub struct DecodedStrongTypeVtableSemanticPlan<C> {
    pub(in crate::production::registration_production) table:
        DecodedPersistentId<PersistentDispatchTableId>,
    pub(in crate::production::registration_production) slots: Vec<C>,
}

impl<C: WireEncode> WireEncode for DecodedStrongTypeVtableSemanticPlan<C> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encode_field(encoder, 1, &self.table)?;
        encode_array_field(encoder, 2, &self.slots)
    }
}

impl<C: WireDecode> WireDecode for DecodedStrongTypeVtableSemanticPlan<C> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            table: decoder.field(1, DecodedPersistentId::decode)?,
            slots: decode_array_field(decoder, 2, C::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongTypeItableSemanticPlan<D, C> {
    pub(in crate::production::registration_production) table:
        DecodedPersistentId<PersistentDispatchTableId>,
    pub(in crate::production::registration_production) interface: D,
    pub(in crate::production::registration_production) slots: Vec<C>,
}

impl<D: WireEncode, C: WireEncode> WireEncode for DecodedStrongTypeItableSemanticPlan<D, C> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encode_field(encoder, 1, &self.table)?;
        encode_field(encoder, 2, &self.interface)?;
        encode_array_field(encoder, 3, &self.slots)
    }
}

impl<D: WireDecode, C: WireDecode> WireDecode for DecodedStrongTypeItableSemanticPlan<D, C> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            table: decoder.field(1, DecodedPersistentId::decode)?,
            interface: decoder.field(2, D::decode)?,
            slots: decode_array_field(decoder, 3, C::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongTypeRegistrationPlan<P, D, C> {
    pub(in crate::production::registration_production) exact_type:
        DecodedPersistentId<PersistentExactTypeId>,
    runtime_type: u64,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    descriptor_symbol: DecodedPersistentSymbolRequest,
    descriptor_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    descriptor_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    diagnostic_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    pub(in crate::production::registration_production) layout:
        DecodedPersistentId<PersistentLayoutId>,
    layout_symbol: DecodedPersistentSymbolRequest,
    layout_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    layout_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_object_node: DecodedPersistentId<DigestNodeId>,
    descriptor_definition_node: DecodedPersistentId<DigestNodeId>,
    layout_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    descriptor_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    layout_fingerprint_patch: DecodedPersistentId<DigestPatchIntentId>,
    pub(in crate::production::registration_production) diagnostic_name: String,
    pub(in crate::production::registration_production) instance_scan:
        DecodedPersistentId<PersistentScanId>,
    pub(in crate::production::registration_production) instance_shape: DecodedTypeInstanceShapeV1,
    pub(in crate::production::registration_production) parent: P,
    pub(in crate::production::registration_production) relations: crate::TypeDescriptorRelations<P>,
    pub(in crate::production::registration_production) release_policy:
        crate::ReleasePolicy<DecodedPersistentId<PersistentCallableBodyId>>,
    pub(in crate::production::registration_production) vtable:
        DecodedStrongTypeVtableSemanticPlan<C>,
    pub(in crate::production::registration_production) itables:
        Vec<DecodedStrongTypeItableSemanticPlan<D, C>>,
    pub(in crate::production::registration_production) inline_scan:
        DecodedTypeDescriptorInlineScanV1,
    itable_directory: DecodedTypeDescriptorITableDirectoryV1,
}

impl<P: WireEncode, D: WireEncode, C: WireEncode> WireEncode
    for DecodedStrongTypeRegistrationPlan<P, D, C>
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(30)?;
        encode_field(encoder, 1, &self.exact_type)?;
        encode_unsigned_field(encoder, 2, self.runtime_type)?;
        encode_field(encoder, 3, &self.symbol)?;
        encode_field(encoder, 4, &self.definition_plan)?;
        encode_field(encoder, 5, &self.primary_atom)?;
        encode_field(encoder, 6, &self.descriptor_symbol)?;
        encode_field(encoder, 7, &self.descriptor_definition_plan)?;
        encode_field(encoder, 8, &self.descriptor_primary_atom)?;
        encode_field(encoder, 9, &self.layout)?;
        encode_field(encoder, 10, &self.layout_symbol)?;
        encode_field(encoder, 11, &self.layout_definition_plan)?;
        encode_field(encoder, 12, &self.layout_primary_atom)?;
        encode_field(encoder, 13, &self.registration_object_node)?;
        encode_field(encoder, 14, &self.descriptor_definition_node)?;
        encode_field(encoder, 15, &self.layout_fingerprint_node)?;
        encode_field(encoder, 16, &self.registration_fingerprint_node)?;
        encode_field(encoder, 17, &self.registration_definition_patch)?;
        encode_field(encoder, 18, &self.descriptor_definition_patch)?;
        encode_field(encoder, 19, &self.layout_fingerprint_patch)?;
        encoder.field(20)?;
        encoder.text(&self.diagnostic_name)?;
        encode_field(encoder, 21, &self.instance_scan)?;
        encode_field(encoder, 22, &self.instance_shape)?;
        encode_field(encoder, 23, &self.parent)?;
        encode_field(encoder, 24, &self.vtable)?;
        encode_array_field(encoder, 25, &self.itables)?;
        encode_field(encoder, 26, &self.diagnostic_atom)?;
        encode_field(encoder, 27, &self.inline_scan)?;
        encode_field(encoder, 28, &self.itable_directory)?;
        encode_field(encoder, 29, &self.relations)?;
        encode_field(encoder, 30, &self.release_policy)
    }
}

impl<P: WireDecode, D: WireDecode, C: WireDecode> WireDecode
    for DecodedStrongTypeRegistrationPlan<P, D, C>
{
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(30)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            runtime_type: decoder.field(2, Decoder::unsigned)?,
            symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(4, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(5, DecodedPersistentId::decode)?,
            descriptor_symbol: decoder.field(6, DecodedPersistentSymbolRequest::decode)?,
            descriptor_definition_plan: decoder.field(7, DecodedPersistentId::decode)?,
            descriptor_primary_atom: decoder.field(8, DecodedPersistentId::decode)?,
            layout: decoder.field(9, DecodedPersistentId::decode)?,
            layout_symbol: decoder.field(10, DecodedPersistentSymbolRequest::decode)?,
            layout_definition_plan: decoder.field(11, DecodedPersistentId::decode)?,
            layout_primary_atom: decoder.field(12, DecodedPersistentId::decode)?,
            registration_object_node: decoder.field(13, DecodedPersistentId::decode)?,
            descriptor_definition_node: decoder.field(14, DecodedPersistentId::decode)?,
            layout_fingerprint_node: decoder.field(15, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(16, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(17, DecodedPersistentId::decode)?,
            descriptor_definition_patch: decoder.field(18, DecodedPersistentId::decode)?,
            layout_fingerprint_patch: decoder.field(19, DecodedPersistentId::decode)?,
            diagnostic_name: decoder.field(20, |decoder| Ok(decoder.text()?.to_owned()))?,
            instance_scan: decoder.field(21, DecodedPersistentId::decode)?,
            instance_shape: decoder.field(22, DecodedTypeInstanceShapeV1::decode)?,
            parent: decoder.field(23, P::decode)?,
            vtable: decoder.field(24, DecodedStrongTypeVtableSemanticPlan::<C>::decode)?,
            itables: decode_array_field(
                decoder,
                25,
                DecodedStrongTypeItableSemanticPlan::<D, C>::decode,
            )?,
            diagnostic_atom: decoder.field(26, DecodedPersistentId::decode)?,
            inline_scan: decoder.field(27, DecodedTypeDescriptorInlineScanV1::decode)?,
            itable_directory: decoder.field(28, DecodedTypeDescriptorITableDirectoryV1::decode)?,
            relations: decoder.field(29, crate::TypeDescriptorRelations::<P>::decode)?,
            release_policy: decoder.field(30, crate::ReleasePolicy::decode)?,
        })
    }
}
