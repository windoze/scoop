use super::*;

pub(super) struct DecodedTypes {
    provider: ConeIdentity,
    identities: ValidatedIdentityGraph,
    foundation: hir::OdrFreeHirFoundation,
    public: hir::CrossConeHirInterfaceSectionV1,
    types: CrossConeTypeSemanticsSectionV1,
}

impl DecodedTypes {
    pub(super) fn resolve_dispatch_order(
        &mut self,
        order: &hir::NominalDispatchOrderV1,
    ) -> Result<
        hir::NominalDispatchOrderV1,
        hir::NominalDispatchOrderResolutionError<scoop_identity::IdentityReferenceError>,
    > {
        let decoded: hir::DecodedNominalDispatchOrderV1 = decoded(order);
        decoded.resolve(&mut self.identities)
    }
    pub(super) fn read(
        mut input: current_hir::CurrentConeHirArtifacts,
        coordinate: &ConeCoordinate,
        dependencies: &[CheckedSharedTypeFoundationV1<'_>],
    ) -> Self {
        let provider = input.hir.output().export.module().cone;
        let mut foundation =
            hir::CanonicalHirFoundation::from_type_semantics_output(&input.hir).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                input.hir.output().export.module(),
                &input.cross_cone_section,
            )
            .unwrap();
        let foundation: hir::DecodedHirFoundation = decoded(&foundation);
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(provider).unwrap();
        foundation.register_identities(&mut pending).unwrap();
        for dependency in dependencies {
            pending
                .register_external_graph_authorities(dependency.metadata().identities)
                .unwrap();
        }
        foundation.resolve_identities(&mut pending).unwrap();
        let mut identities = pending.finish().unwrap();
        let foundation = hir::OdrFreeHirFoundation::from_validated(
            foundation
                .validate_with_dependency_sources(coordinate, &mut identities)
                .unwrap(),
        )
        .unwrap();
        let public: hir::DecodedCrossConeHirInterfaceSectionV1 =
            decoded(&input.cross_cone_section.index_for_wire().unwrap());
        let public = public.resolve(&mut identities).unwrap();
        let source = scoop_hir_lower::produce_cross_cone_type_semantics(
            &input.hir,
            hir::SharedTypeMetadataV1 {
                provider,
                identities: &identities,
                foundation: &foundation,
                public: &public,
            },
            &dependencies
                .iter()
                .map(|dependency| dependency.metadata())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let types: hir::DecodedCrossConeTypeSemanticsSectionV1 =
            decoded(&source.section().index_for_wire().unwrap());
        let types = types
            .resolve(&mut identities, &scoop_wire::WirePath::root())
            .unwrap();
        Self {
            provider,
            identities,
            foundation,
            public,
            types,
        }
    }

    pub(super) fn metadata(&self) -> hir::SharedTypeMetadataV1<'_> {
        hir::SharedTypeMetadataV1 {
            provider: self.provider,
            identities: &self.identities,
            foundation: &self.foundation,
            public: &self.public,
        }
    }

    pub(super) fn check<'a>(
        &'a self,
        dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    ) -> Result<CheckedSharedTypeFoundationV1<'a>, Error> {
        self.types
            .validate_shared_foundation(self.metadata(), dependencies)
    }
}
