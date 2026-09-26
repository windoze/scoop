use super::*;
use scoop_wire::encode_canonical_temporary;

impl ExportsResolvedCrossConeLayoutAbiSectionV1 {
    pub fn validate<'a, E>(
        self,
        expected: &LayoutAbiExportConstituentsV1,
        dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.exports.provider() != expected.provider()
            || self.exports.target_profile() != expected.target_profile()
            || !same_bytes(self.exports.layouts(), expected.layouts())?
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        if !same_bytes(self.exports.callables(), expected.callables())? {
            return Err(LayoutAbiSectionError::CallableReplayChanged);
        }
        if !same_bytes(self.exports.dispatch(), expected.dispatch())? {
            return Err(LayoutAbiSectionError::DispatchReplayChanged);
        }
        let dependencies =
            dependencies::complete(expected.provider(), expected.target_profile(), dependencies)?;
        let physical_imports =
            crate::CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports)?;
        if !same_bytes(self.exports.descriptors(), expected.descriptors())? {
            return Err(LayoutAbiSectionError::DescriptorReplayChanged);
        }
        if !same_bytes(self.exports.shape_support(), expected.shape_support())? {
            return Err(LayoutAbiSectionError::ShapeSupport);
        }
        let resolved = self.resolve_dependencies(identities)?;
        let physical_imports = resolved.physical.validate_against(&physical_imports)?;
        build::complete(
            resolved.exports,
            dependencies,
            physical_imports,
            build::SelectionInput::Reader(resolved.semantic),
            source,
        )
    }
}

fn same_bytes(actual: &impl WireEncode, expected: &impl WireEncode) -> Result<bool, WireError> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary(actual, &path)?;
    let expected = encode_canonical_temporary(expected, &path)?;

    Ok(actual == expected)
}
