use super::*;
use scoop_identity::SignatureTypeKey;

impl Projection<'_> {
    pub(super) fn parameters(
        &mut self,
        owner: ExportParameterOwner,
        binders: &[HirSignatureBinder],
        expected: &[SignatureTypeKey],
    ) -> Result<CanonicalSourceParameterShapesV1, Error> {
        super::super::source_parameter_shapes::project(
            self.export,
            &self.signatures,
            owner,
            binders,
            expected,
        )
    }
}
