use scoop_lir::{DecodedCanonicalExternalShapeLinkImportsV1, SelectedDependencyLayoutAbiSetV1};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath,
    encode_canonical_temporary_with_meter,
};

use super::{
    CrossConeLayoutLinkClosureSectionV1, EncodeResult, ExternalShapeRelocationUseSetDigestV1,
    LayoutLinkClosureError, sequence,
};
use crate::link_object::{
    DecodedCanonicalUndefinedRelocationUseV1, DecodedCodeLinkObjectMemberSetV1, DecodedFixedBytesV1,
};

#[derive(Debug)]
struct DecodedUse {
    use_site: DecodedCanonicalUndefinedRelocationUseV1,
    import_index: u32,
}
impl WireEncode for DecodedUse {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.import_index))
    }
}
impl WireDecode for DecodedUse {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            use_site: decoder.field(1, DecodedCanonicalUndefinedRelocationUseV1::decode)?,
            import_index: decoder.field(2, Decoder::u32)?,
        })
    }
}

#[derive(Debug)]
struct DecodedCoverage {
    objects: DecodedCodeLinkObjectMemberSetV1,
    digest: DecodedFixedBytesV1<ExternalShapeRelocationUseSetDigestV1>,
}
impl WireEncode for DecodedCoverage {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(2)?;
        encoder.field(1)?;
        self.objects.encode(encoder)?;
        encoder.field(2)?;
        self.digest.encode(encoder)
    }
}
impl WireDecode for DecodedCoverage {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            objects: decoder.field(1, DecodedCodeLinkObjectMemberSetV1::decode)?,
            digest: decoder.field(2, DecodedFixedBytesV1::decode)?,
        })
    }
}

/// Untrusted bytes, with no selected-import or object-coverage authority.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutLinkClosureSectionV1 {
    semantic_imports: DecodedCanonicalExternalShapeLinkImportsV1,
    requirements: Vec<DecodedUse>,
    object_coverage: DecodedCoverage,
}

impl DecodedCrossConeLayoutLinkClosureSectionV1 {
    /// Checks the Compile projection without claiming relocation coverage.
    pub fn validate_semantic_imports_against(
        &self,
        selected: &SelectedDependencyLayoutAbiSetV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<(), LayoutLinkClosureError> {
        if !same_bytes(&self.semantic_imports, selected.physical_imports(), meter)? {
            return Err(LayoutLinkClosureError::SemanticImports(
                scoop_lir::ShapeLinkError::Contract,
            ));
        }
        Ok(())
    }

    /// Only returns the independently rebuilt proof. No wire field is promoted
    /// to a trusted identity, relocation use, or directory record.
    pub fn validate_against<'a>(
        self,
        expected: &CrossConeLayoutLinkClosureSectionV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutLinkClosureSectionV1<'a>, LayoutLinkClosureError> {
        self.semantic_imports
            .validate_against(expected.semantic_imports(), meter)?;
        validate_requirements(&self.requirements, expected.requirements(), meter)?;
        if !same_bytes(
            &self.object_coverage.objects,
            expected.object_coverage().verified_link_objects(),
            meter,
        )? || !self.object_coverage.digest.matches(
            expected
                .object_coverage()
                .relocation_use_set_digest()
                .as_array(),
        ) {
            return Err(LayoutLinkClosureError::ObjectCoverageMismatch);
        }
        Ok(expected.clone())
    }
}

fn validate_requirements(
    actual: &[DecodedUse],
    expected: &[super::ExternalShapeUndefinedUseV1],
    meter: &mut BudgetMeter,
) -> Result<(), LayoutLinkClosureError> {
    meter.charge_nodes(actual.len() as u64, &WirePath::root())?;
    if actual.len() != expected.len() {
        return Err(LayoutLinkClosureError::RequirementsMismatch);
    }
    for (actual, expected) in actual.iter().zip(expected) {
        if !same_bytes(actual, expected, meter)? {
            return Err(LayoutLinkClosureError::RequirementsMismatch);
        }
    }
    Ok(())
}

fn same_bytes(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<bool, WireError> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
    Ok(actual == expected)
}

impl WireEncode for DecodedCrossConeLayoutLinkClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_imports.encode(encoder)?;
        encoder.field(2)?;
        sequence(encoder, &self.requirements)?;
        encoder.field(3)?;
        self.object_coverage.encode(encoder)
    }
}
impl WireDecode for DecodedCrossConeLayoutLinkClosureSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            semantic_imports: decoder
                .field(1, DecodedCanonicalExternalShapeLinkImportsV1::decode)?,
            requirements: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedUse::decode(decoder))
            })?,
            object_coverage: decoder.field(3, DecodedCoverage::decode)?,
        })
    }
}

#[cfg(test)]
#[path = "tests/wire.rs"]
mod tests;
