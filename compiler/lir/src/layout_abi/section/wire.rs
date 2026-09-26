use super::*;

mod descriptors;
mod physical;
mod resolved_dependencies;
pub use physical::PhysicalImportsReplayedLayoutAbiSectionV1;
mod shapes;
mod stages;

pub use resolved_dependencies::DependencyResolvedCrossConeLayoutAbiSectionV1;
pub use shapes::ExportsResolvedCrossConeLayoutAbiSectionV1;

pub type DecodedCrossConeLayoutAbiSectionV1 = UnselectedCrossConeLayoutAbiSectionV1<
    crate::DecodedCanonicalExactLayoutExportsV1,
    crate::DecodedCanonicalExactCallableAbiExportsV1,
    crate::DecodedCanonicalExactDispatchExportsV1,
    crate::DecodedCanonicalExactDescriptorExportsV1,
>;
pub type LayoutsResolvedCrossConeLayoutAbiSectionV1 = UnselectedCrossConeLayoutAbiSectionV1<
    crate::CanonicalExactLayoutExportsV1,
    crate::DecodedCanonicalExactCallableAbiExportsV1,
    crate::DecodedCanonicalExactDispatchExportsV1,
    crate::DecodedCanonicalExactDescriptorExportsV1,
>;
pub type CallablesResolvedCrossConeLayoutAbiSectionV1 = UnselectedCrossConeLayoutAbiSectionV1<
    crate::CanonicalExactLayoutExportsV1,
    crate::CanonicalExactCallableAbiExportsV1,
    crate::DecodedCanonicalExactDispatchExportsV1,
    crate::DecodedCanonicalExactDescriptorExportsV1,
>;
pub type DispatchResolvedCrossConeLayoutAbiSectionV1 = UnselectedCrossConeLayoutAbiSectionV1<
    crate::CanonicalExactLayoutExportsV1,
    crate::CanonicalExactCallableAbiExportsV1,
    crate::CanonicalExactDispatchExportsV1,
    crate::DecodedCanonicalExactDescriptorExportsV1,
>;

pub type DescriptorsResolvedCrossConeLayoutAbiSectionV1 = UnselectedCrossConeLayoutAbiSectionV1<
    crate::CanonicalExactLayoutExportsV1,
    crate::CanonicalExactCallableAbiExportsV1,
    crate::CanonicalExactDispatchExportsV1,
    crate::CanonicalExactDescriptorExportsV1,
>;

/// Each type parameter records a checked constituent. Remaining exports,
/// selected uses and physical imports always retain their untrusted wire.
#[derive(Debug)]
pub struct UnselectedCrossConeLayoutAbiSectionV1<L, C, D, T> {
    layouts: L,
    descriptors: T,
    dispatch: D,
    callables: C,
    shape_support: crate::DecodedCanonicalParamFreeShapeSupportExportsV1,
    selected: DecodedSelectedDependencyLayoutAbiSetV1,
}

#[derive(Debug)]
struct DecodedSelectedDependencyLayoutAbiSetV1 {
    semantic: Vec<DecodedLayoutAbiDependencyV1>,
    physical: crate::DecodedCanonicalExternalShapeLinkImportsV1,
}

impl WireDecode for DecodedCrossConeLayoutAbiSectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            layouts: decoder.field(1, crate::DecodedCanonicalExactLayoutExportsV1::decode)?,
            descriptors: decoder
                .field(2, crate::DecodedCanonicalExactDescriptorExportsV1::decode)?,
            dispatch: decoder.field(3, crate::DecodedCanonicalExactDispatchExportsV1::decode)?,
            callables: decoder
                .field(4, crate::DecodedCanonicalExactCallableAbiExportsV1::decode)?,
            shape_support: decoder.field(
                5,
                crate::DecodedCanonicalParamFreeShapeSupportExportsV1::decode,
            )?,
            selected: decoder.field(6, DecodedSelectedDependencyLayoutAbiSetV1::decode)?,
        })
    }
}

impl<L: WireEncode, C: WireEncode, D: WireEncode, T: WireEncode> WireEncode
    for UnselectedCrossConeLayoutAbiSectionV1<L, C, D, T>
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        field(encoder, 1, &self.layouts)?;
        field(encoder, 2, &self.descriptors)?;
        field(encoder, 3, &self.dispatch)?;
        field(encoder, 4, &self.callables)?;
        field(encoder, 5, &self.shape_support)?;
        field(encoder, 6, &self.selected)
    }
}

impl WireEncode for CrossConeLayoutAbiSectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        field(encoder, 1, self.layouts())?;
        field(encoder, 2, self.descriptors())?;
        field(encoder, 3, self.dispatch())?;
        field(encoder, 4, self.callables())?;
        field(encoder, 5, self.shape_support())?;
        field(encoder, 6, self.selected())
    }
}

impl WireDecode for DecodedSelectedDependencyLayoutAbiSetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            semantic: decoder.field(1, |decoder| {
                decoder.decode_array(|decoder, _| DecodedLayoutAbiDependencyV1::decode(decoder))
            })?,
            physical: decoder
                .field(2, crate::DecodedCanonicalExternalShapeLinkImportsV1::decode)?,
        })
    }
}

impl WireEncode for DecodedSelectedDependencyLayoutAbiSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.semantic.len() as u64)?;
        for relation in &self.semantic {
            relation.encode(encoder)?;
        }
        encoder.field(2)?;
        self.physical.encode(encoder)
    }
}

fn field(
    encoder: &mut Encoder,
    index: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(index)?;
    value.encode(encoder)
}
