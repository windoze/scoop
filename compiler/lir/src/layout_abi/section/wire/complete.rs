use super::*;
use scoop_wire::encode_canonical_temporary_with_meter;

impl ExportsResolvedCrossConeLayoutAbiSectionV1 {
    pub fn validate<'a, E>(
        self,
        expected: &LayoutAbiExportConstituentsV1,
        dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1<'a>>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        identities: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.exports.provider() != expected.provider()
            || self.exports.target_profile() != expected.target_profile()
            || !same_bytes(self.exports.layouts(), expected.layouts(), meter)?
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        if !same_bytes(self.exports.callables(), expected.callables(), meter)? {
            return Err(LayoutAbiSectionError::CallableReplayChanged);
        }
        if !same_bytes(self.exports.dispatch(), expected.dispatch(), meter)? {
            return Err(LayoutAbiSectionError::DispatchReplayChanged);
        }
        let dependencies = dependencies::complete(
            expected.provider(),
            expected.target_profile(),
            dependencies,
            meter,
        )?;
        let physical_imports =
            crate::CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports, meter)?;
        if !same_bytes(self.exports.descriptors(), expected.descriptors(), meter)? {
            return Err(LayoutAbiSectionError::DescriptorReplayChanged);
        }
        if !same_bytes(
            self.exports.shape_support(),
            expected.shape_support(),
            meter,
        )? {
            return Err(LayoutAbiSectionError::ShapeSupport);
        }
        let mut semantic = reserve(self.selected.semantic.len(), meter)?;
        for relation in self.selected.semantic {
            semantic.push(relation.resolve(identities, meter)?);
        }
        let physical_imports = self
            .selected
            .physical
            .validate_against(&physical_imports, meter)?;
        build::complete(
            self.exports,
            dependencies,
            physical_imports,
            build::SelectionInput::Reader(semantic),
            source,
            meter,
        )
    }
}

fn same_bytes(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<bool, WireError> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len() as u64, &path)?;
    Ok(actual == expected)
}
