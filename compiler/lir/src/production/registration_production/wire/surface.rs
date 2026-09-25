//! Closed versioned registration carriers; only type-reference sums change.

use super::*;

pub type DecodedStrongRegistrationProductionSurfaceV1 =
    DecodedStrongRegistrationProductionSurface<DecodedStrongTypeRegistrationPlanV1>;
pub type DecodedStrongRegistrationProductionSurfaceV2 =
    DecodedStrongRegistrationProductionSurface<DecodedStrongTypeRegistrationPlanV2>;

#[derive(Debug)]
pub struct DecodedStrongRegistrationProductionSurface<T> {
    pub(in super::super) identities: DecodedStrongRegistrationIdentitySurfaceV1,
    pub(in super::super) safepoints: Vec<DecodedStrongSafepointRegistrationPlanV1>,
    pub(in super::super) callables: Vec<DecodedStrongCallableRegistrationPlanV1>,
    pub(in super::super) types: Vec<T>,
    pub(in super::super) immortal_objects: Vec<DecodedStrongImmortalObjectRegistrationPlanV1>,
    pub(in super::super) static_storages: Vec<DecodedStrongStaticStorageRegistrationPlanV1>,
    pub(in super::super) initialization_units:
        Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    pub(in super::super) callable_runtime_scans: Vec<DecodedStrongCallableRuntimeScanPlanV1>,
}

impl<T: WireEncode> WireEncode for DecodedStrongRegistrationProductionSurface<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encode_field(encoder, 1, &self.identities)?;
        encode_array_field(encoder, 2, &self.safepoints)?;
        encode_array_field(encoder, 3, &self.callables)?;
        encode_array_field(encoder, 4, &self.types)?;
        encode_array_field(encoder, 5, &self.immortal_objects)?;
        encode_array_field(encoder, 6, &self.static_storages)?;
        encode_array_field(encoder, 7, &self.initialization_units)?;
        encode_array_field(encoder, 8, &self.callable_runtime_scans)
    }
}

impl<T: WireDecode> WireDecode for DecodedStrongRegistrationProductionSurface<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            identities: decoder.field(1, DecodedStrongRegistrationIdentitySurfaceV1::decode)?,
            safepoints: decode_array_field(
                decoder,
                2,
                DecodedStrongSafepointRegistrationPlanV1::decode,
            )?,
            callables: decode_array_field(
                decoder,
                3,
                DecodedStrongCallableRegistrationPlanV1::decode,
            )?,
            types: decode_array_field(decoder, 4, T::decode)?,
            immortal_objects: decode_array_field(
                decoder,
                5,
                DecodedStrongImmortalObjectRegistrationPlanV1::decode,
            )?,
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
            callable_runtime_scans: decode_array_field(
                decoder,
                8,
                DecodedStrongCallableRuntimeScanPlanV1::decode,
            )?,
        })
    }
}
