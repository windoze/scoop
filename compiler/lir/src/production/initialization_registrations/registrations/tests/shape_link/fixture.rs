use super::*;

pub(super) struct ProviderFixture {
    pub source: Fixture,
    pub section: StrongProductionSectionV2,
    pub ordinary: CrossConeLirBridgeSectionV1,
    pub layouts: CanonicalExactLayoutExportsV1,
    pub callables: CanonicalExactCallableAbiExportsV1,
    pub descriptors: CanonicalExactDescriptorExportsV1,
    pub dispatch: CanonicalExactDispatchExportsV1,
}

impl ProviderFixture {
    pub fn new(lazy: bool) -> Self {
        let mut source = Fixture::complete_surface(Options {
            lazy,
            ..Options::default()
        });
        let coordinate = scoop_identity::ConeCoordinate::reserved_single_file();
        crate::production::strong_section::tests::attach_image(
            &coordinate,
            &mut source.foundation,
            &mut source.digests,
        );
        let producer = source.foundation.producer();
        let unit = &source.semantics.units()[0];
        let units = StrongInitializationUnitSemanticPlanSetV2::from_artifact(
            source.semantics.static_storages().clone(),
            vec![StrongInitializationUnitSemanticPlanV2::from_artifact(
                unit.unit(),
                unit.diagnostic_path().to_owned(),
                unit.schedule(),
                unit.storage(),
                unit.failure_root(),
                unit.initializer(),
                unit.ensure(),
                Vec::new(),
            )],
        );
        let registrations = StrongRegistrationProductionSurfaceV2::from_semantics(
            TARGET,
            &source.foundation,
            &source.digests,
            source.identities.clone(),
            StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(&source.foundation)
                .unwrap(),
            StrongTypeDescriptorSemanticPlanSetV2::from_artifact(
                producer,
                TARGET.wire_id(),
                Vec::new(),
            ),
            StrongSafepointSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            StrongImmortalObjectSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            units,
        )
        .unwrap();
        let section = StrongProductionSectionV2::from_parts(
            coordinate,
            &[],
            &source.foundation,
            StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
            source.digests.clone(),
            registrations,
            EntryProductionSourceV1::Library,
            &[],
            None,
        )
        .unwrap();
        let layouts =
            CanonicalExactLayoutExportsV1::try_new(TARGET, &source.foundation, Vec::new()).unwrap();
        let descriptors =
            CanonicalExactDescriptorExportsV1::try_new(TARGET, &source.foundation, Vec::new())
                .unwrap();
        let dispatch =
            CanonicalExactDispatchExportsV1::try_new(TARGET, &source.foundation, Vec::new())
                .unwrap();
        let ordinary =
            CrossConeLirBridgeSectionV1::try_new(&source.foundation, Vec::new(), Vec::new())
                .unwrap();
        let unit_layout: ExactLayoutExportV1 = crate::exact_layout::tests::unit().into();
        let records = [
            InitializationCallableRole::Initializer,
            InitializationCallableRole::Ensure,
        ]
        .into_iter()
        .map(|role| {
            let target = StrongCallableDefinitionOwner::GeneratedCallable(
                PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
                    unit: source.unit,
                    role,
                })
                .unwrap(),
            );
            ExactCallableAbiExportV1::replay(
                TARGET,
                target,
                scoop_identity::ExactCallableSignature::new(
                    scoop_identity::Effect::Ordinary,
                    None,
                    Vec::new(),
                    unit_layout.identity().exact(),
                ),
                ExactCallableProtocolV1::OrdinaryManaged,
                CallableAbiLayoutInputsV1 {
                    receiver: CallableAbiReceiverInputV1::NoReceiver,
                    parameters: &[],
                    result: &unit_layout,
                },
                &source.foundation,
            )
            .unwrap()
        })
        .collect();
        let callables =
            CanonicalExactCallableAbiExportsV1::try_new(TARGET, &source.foundation, records)
                .unwrap();
        Self {
            source,
            section,
            ordinary,
            layouts,
            callables,
            descriptors,
            dispatch,
        }
    }

    pub fn provider(&self) -> ShapeLinkProviderV1<'_> {
        ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
            foundation: &self.source.foundation,
            production: &self.section,
            ordinary: &self.ordinary,
            layouts: &self.layouts,
            callables: &self.callables,
            descriptors: &self.descriptors,
            dispatch: &self.dispatch,
        })
        .unwrap()
    }

    pub fn unit(&self) -> &StrongInitializationUnitSemanticPlanV2 {
        self.section
            .registration_production()
            .initialization_units()
            .registrations()[0]
            .semantic()
    }
}
