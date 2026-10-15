use super::*;

/// All local exports are available. Dependency uses and physical imports
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

    pub fn read_shape_support(
        self,
        ordinary: &crate::CrossConeLirBridgeSectionV1,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<ExportsResolvedCrossConeLayoutAbiSectionV1, crate::LinkDataError> {
        let shapes =
            self.shape_support
                .read(self.layouts.target(), self.layouts.provider(), identities)?;
        let exports = LayoutAbiExportConstituentsV1::try_new(
            self.layouts,
            self.descriptors,
            self.dispatch,
            self.callables,
            shapes,
            ordinary.clone(),
        )
        .map_err(crate::link_data::link_error)?;
        Ok(ExportsResolvedCrossConeLayoutAbiSectionV1 {
            exports,
            selected: self.selected,
        })
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
