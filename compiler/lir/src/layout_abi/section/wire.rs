use super::*;
use scoop_wire::encode_canonical_temporary_with_meter;

#[derive(Debug)]
pub struct DecodedCrossConeLayoutAbiSectionV1 {
    layouts: crate::DecodedCanonicalExactLayoutExportsV1,
    descriptors: crate::DecodedCanonicalExactDescriptorExportsV1,
    dispatch: crate::DecodedCanonicalExactDispatchExportsV1,
    callables: crate::DecodedCanonicalExactCallableAbiExportsV1,
    shape_support: crate::DecodedCanonicalParamFreeShapeSupportExportsV1,
    selected: DecodedSelectedDependencyLayoutAbiSetV1,
}

/// The layout table has been compared with canonical replay. Every remaining
/// constituent, selected use and physical import still requires validation.
#[derive(Debug)]
pub struct LayoutsResolvedCrossConeLayoutAbiSectionV1 {
    layouts: crate::CanonicalExactLayoutExportsV1,
    descriptors: crate::DecodedCanonicalExactDescriptorExportsV1,
    dispatch: crate::DecodedCanonicalExactDispatchExportsV1,
    callables: crate::DecodedCanonicalExactCallableAbiExportsV1,
    shape_support: crate::DecodedCanonicalParamFreeShapeSupportExportsV1,
    selected: DecodedSelectedDependencyLayoutAbiSetV1,
}

#[derive(Debug)]
struct DecodedSelectedDependencyLayoutAbiSetV1 {
    semantic: Vec<DecodedLayoutAbiDependencyV1>,
    physical: crate::DecodedCanonicalExternalShapeLinkImportsV1,
}

impl DecodedCrossConeLayoutAbiSectionV1 {
    pub fn validate_layouts(
        self,
        expected: &crate::CanonicalExactLayoutExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<LayoutsResolvedCrossConeLayoutAbiSectionV1, crate::ExactLayoutTableError> {
        Ok(LayoutsResolvedCrossConeLayoutAbiSectionV1 {
            layouts: self.layouts.validate_against(expected, meter)?,
            descriptors: self.descriptors,
            dispatch: self.dispatch,
            callables: self.callables,
            shape_support: self.shape_support,
            selected: self.selected,
        })
    }

    pub fn validate<'a, E>(
        self,
        expected: &LayoutAbiExportConstituentsV1,
        dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1<'a>>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        identities: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        self.validate_layouts(expected.layouts(), meter)?.validate(
            expected,
            dependencies,
            physical_imports,
            source,
            identities,
            meter,
        )
    }
}

impl LayoutsResolvedCrossConeLayoutAbiSectionV1 {
    pub const fn layouts(&self) -> &crate::CanonicalExactLayoutExportsV1 {
        &self.layouts
    }

    pub fn validate<'a, E>(
        self,
        expected: &LayoutAbiExportConstituentsV1,
        dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1<'a>>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        identities: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.layouts.provider() != expected.provider()
            || self.layouts.target() != expected.target_profile()
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        let path = WirePath::root();
        let checked_layouts = encode_canonical_temporary_with_meter(&self.layouts, meter, &path)?;
        let expected_layouts =
            encode_canonical_temporary_with_meter(expected.layouts(), meter, &path)?;
        meter.charge_work(checked_layouts.len() as u64, &path)?;
        if checked_layouts != expected_layouts {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        let dependencies = dependencies::complete(
            expected.provider(),
            expected.target_profile(),
            dependencies,
            meter,
        )?;
        let physical_imports =
            crate::CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports, meter)?;
        let descriptors = self
            .descriptors
            .validate_against(expected.descriptors(), meter)?;
        let dispatch = self.dispatch.validate_against(expected.dispatch(), meter)?;
        let callables = self
            .callables
            .validate_against(expected.callables(), meter)?;
        validate_shape_support(self.shape_support, expected.shape_support(), meter)?;
        let exports = LayoutAbiExportConstituentsV1::try_new(
            self.layouts,
            descriptors,
            dispatch,
            callables,
            expected.shape_support().clone(),
        )?;
        let mut semantic = reserve(self.selected.semantic.len(), meter)?;
        for relation in self.selected.semantic {
            semantic.push(relation.resolve(identities, meter)?);
        }
        let physical_imports = self
            .selected
            .physical
            .validate_against(&physical_imports, meter)?;
        build::complete(
            exports,
            dependencies,
            physical_imports,
            build::SelectionInput::Reader(semantic),
            source,
            meter,
        )
    }
}

fn validate_shape_support<E>(
    actual: crate::DecodedCanonicalParamFreeShapeSupportExportsV1,
    expected: &crate::CanonicalParamFreeShapeSupportExportsV1,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSectionError<E>> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary_with_meter(&actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len() as u64, &path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(LayoutAbiSectionError::ShapeSupport)
    }
}

impl WireDecode for DecodedCrossConeLayoutAbiSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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

impl WireEncode for DecodedCrossConeLayoutAbiSectionV1 {
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

impl WireEncode for LayoutsResolvedCrossConeLayoutAbiSectionV1 {
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
