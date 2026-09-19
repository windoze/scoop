use super::*;

struct Candidate<'a>(ShapeLinkSupportSourceV1<'a>);
impl crate::shape_link::support::sealed::Sealed for Candidate<'_> {}
impl<'a> ShapeLinkSupportAuthorityV1<'a> for Candidate<'a> {
    fn support_source(
        &self,
        _: ConeIdentity,
        _: Subject,
        _: &mut BudgetMeter,
    ) -> Result<Option<ShapeLinkSupportSourceV1<'a>>, ShapeLinkError> {
        Ok(Some(self.0))
    }
}

#[test]
fn shape_link_support_candidate_cannot_replace_provider_unit_semantics() {
    let fixture = ProviderFixture::new(false);
    let unit = fixture.unit();
    let altered = StrongInitializationUnitSemanticPlanV2::from_artifact(
        unit.unit(),
        "another-object".to_owned(),
        unit.schedule(),
        unit.storage(),
        unit.failure_root(),
        unit.initializer(),
        unit.ensure(),
        unit.dependencies().to_vec(),
    );
    let candidate = Candidate(ShapeLinkSupportSourceV1::Initialization { unit: &altered });
    let subject = Subject::InitializationCell(unit.unit());
    assert!(
        matches!(ExternalShapeLinkImportV1::replay(&fixture.provider(), subject, ConeIdentity::CORE, &consumer(), &candidate, &mut meter()), Err(ShapeLinkError::SupportRelation(actual)) if actual == subject)
    );
    let storage = fixture
        .section
        .registration_production()
        .static_storages()
        .registrations()
        .iter()
        .find(|record| record.semantic().storage() == unit.failure_root())
        .unwrap()
        .semantic();
    let candidate = Candidate(ShapeLinkSupportSourceV1::StaticStorage { unit, storage });
    let subject = Subject::StaticStorage(unit.storage());
    assert!(
        matches!(ExternalShapeLinkImportV1::replay(&fixture.provider(), subject, ConeIdentity::CORE, &consumer(), &candidate, &mut meter()), Err(ShapeLinkError::SupportRelation(actual)) if actual == subject)
    );
}

#[test]
fn shape_link_terminal_rebind_rejects_same_body_with_changed_gc_protocol() {
    let fixture = ProviderFixture::new(false);
    let target = fixture.callables.records()[0].target();
    let import = ExternalShapeLinkImportV1::replay(
        &fixture.provider(),
        Subject::Callable(target),
        ConeIdentity::CORE,
        &consumer(),
        &fixture.support(false),
        &mut meter(),
    )
    .unwrap();
    let result: ExactLayoutExportV1 = crate::exact_layout::tests::unit().into();
    let changed = ExactCallableAbiExportV1::replay(
        TARGET,
        target,
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            result.identity().exact(),
        ),
        ExactCallableProtocolV1::OrdinaryNoGc,
        CallableAbiLayoutInputsV1 {
            receiver: CallableAbiReceiverInputV1::NoReceiver,
            parameters: &[],
            result: &result,
        },
        &fixture.source.foundation,
        &mut meter(),
    )
    .unwrap();
    let callables = CanonicalExactCallableAbiExportsV1::try_new(
        TARGET,
        &fixture.source.foundation,
        vec![changed],
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        import.validate_semantic_against(
            &fixture.layouts,
            &callables,
            &fixture.descriptors,
            &fixture.dispatch,
            &mut meter()
        ),
        Err(ShapeLinkError::Contract)
    ));
}
