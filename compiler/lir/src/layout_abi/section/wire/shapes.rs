use super::*;

/// All local exports have been replayed. Dependency uses and physical imports
/// retain their original transport until the complete selection is checked.
#[derive(Debug)]
pub struct ExportsResolvedCrossConeLayoutAbiSectionV1 {
    pub(super) exports: LayoutAbiExportConstituentsV1,
    pub(super) selected: DecodedSelectedDependencyLayoutAbiSetV1,
}

impl DescriptorsResolvedCrossConeLayoutAbiSectionV1 {
    pub const fn descriptors(&self) -> &crate::CanonicalExactDescriptorExportsV1 {
        &self.descriptors
    }

    pub fn validate_shape_support<E>(
        self,
        expected: &crate::CanonicalParamFreeShapeSupportExportsV1,
    ) -> Result<ExportsResolvedCrossConeLayoutAbiSectionV1, LayoutAbiSectionError<E>> {
        if self.layouts.provider() != expected.provider() {
            return Err(LayoutAbiExportConstituentsError::Provider.into());
        }
        if self.layouts.target() != expected.target() {
            return Err(LayoutAbiExportConstituentsError::Target.into());
        }
        let shapes = self.shape_support.validate_against(expected)?;
        let exports = LayoutAbiExportConstituentsV1::try_new(
            self.layouts,
            self.descriptors,
            self.dispatch,
            self.callables,
            shapes,
        )?;
        Ok(ExportsResolvedCrossConeLayoutAbiSectionV1 {
            exports,
            selected: self.selected,
        })
    }

    pub fn validate<'a, E>(
        self,
        expected: &LayoutAbiExportConstituentsV1,
        dependencies: &[&'a LayoutAbiExportConstituentsV1],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CrossConeLayoutAbiSectionV1<'a>, LayoutAbiSectionError<E>> {
        if self.layouts.provider() != expected.provider()
            || self.layouts.target() != expected.target_profile()
        {
            return Err(LayoutAbiSectionError::LayoutReplayChanged);
        }
        self.validate_shape_support(expected.shape_support())?
            .validate(expected, dependencies, physical_imports, source, identities)
    }
}

impl ExportsResolvedCrossConeLayoutAbiSectionV1 {
    pub const fn exports(&self) -> &LayoutAbiExportConstituentsV1 {
        &self.exports
    }
}

impl WireEncode for ExportsResolvedCrossConeLayoutAbiSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        field(encoder, 1, self.exports.layouts())?;
        field(encoder, 2, self.exports.descriptors())?;
        field(encoder, 3, self.exports.dispatch())?;
        field(encoder, 4, self.exports.callables())?;
        field(encoder, 5, self.exports.shape_support())?;
        field(encoder, 6, &self.selected)
    }
}
