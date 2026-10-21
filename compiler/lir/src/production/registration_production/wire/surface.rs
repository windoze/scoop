//! Registration data consumed by downstream compilation and linking.

use super::*;

pub type DecodedStrongRegistrationProductionSurfaceV1 = DecodedStrongRegistrationProductionSurface;
pub type DecodedStrongRegistrationProductionSurfaceV2 = DecodedStrongRegistrationProductionSurface;

#[derive(Debug)]
pub struct DecodedStrongRegistrationProductionSurface {
    pub(in super::super) identities: DecodedRegistrationIdentitySurfaceV1,
    pub(in super::super) static_storages: Vec<DecodedStrongStaticStorageRegistrationPlanV1>,
    pub(in super::super) initialization_units:
        Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
}

impl DecodedStrongRegistrationProductionSurface {
    pub(crate) fn link_registrations(
        self,
        foundation: &crate::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<
        [Vec<(
            scoop_identity::ObjectDefinitionPlanId,
            scoop_identity::PersistentSymbolRequest,
        )>; 6],
        crate::LinkDataError,
    > {
        self.identities.read_link(foundation, identities)
    }
}

impl WireEncode for DecodedStrongRegistrationProductionSurface {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encode_field(encoder, 1, &self.identities)?;
        encode_array_field(encoder, 6, &self.static_storages)?;
        encode_array_field(encoder, 7, &self.initialization_units)
    }
}

impl WireDecode for DecodedStrongRegistrationProductionSurface {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            identities: decoder.field(1, DecodedRegistrationIdentitySurfaceV1::decode)?,
            static_storages: decode_array_field(
                decoder,
                6,
                DecodedStrongStaticStorageRegistrationPlanV1::decode,
            )?,
            initialization_units: decode_array_field(
                decoder,
                7,
                DecodedStrongInitializationUnitRegistrationPlanV1::decode,
            )?,
        })
    }
}
