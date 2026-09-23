use super::*;

impl<'input> OdrCheckedSingleConeLinkFoundations<'input> {
    pub(super) fn validate_shared_production(
        self,
        interface: &scoop_hir::CrossConeHirInterfaceSectionV1,
        expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<ProductionValidatedSingleConeLinkSections<'input>, StrongProfileProductionError>
    {
        let Self {
            mut graph,
            mut identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        } = self;
        let production = crate::strong_compile_decode::validate_shared_strong_profile_production(
            &mut graph,
            &mut identities,
            &foundations,
            production,
            interface,
            expected_external_bridges,
        )?;
        Ok(ProductionValidatedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        })
    }
}
