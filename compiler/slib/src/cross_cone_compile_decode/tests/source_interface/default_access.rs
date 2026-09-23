use super::*;
use scoop_hir::{
    ExportDefaultAccessWitnessV1, ExportDefaultPublicWitnessValidationError,
    ExportDefaultReferenceSetV1, ExportDefaultReferenceV1, ExportDefaultTemplateV1,
    PublicDefaultWitnessError, SourceAccessConstraintV1, SourceAccessDomainV1,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

#[test]
fn ordinary_reader_keeps_the_artifact_budget_when_resolving_source_domains() {
    let fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let bytes = fixture.artifact();
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let mut front = decoded.validate_foundation_structure(identities).unwrap();
    let meter = front.graph.envelope.meter_mut();
    let remaining = meter.limits().validation_work_units - meter.usage().validation_work_units;
    meter.charge_work(remaining, &WirePath::root()).unwrap();
    let Err(error) = front.resolve_hir_interface() else {
        panic!("source-domain resolution must retain the exhausted budget")
    };
    assert!(matches!(
        error,
        scoop_hir::CrossConeHirInterfaceResolutionError::DefaultTemplates(_)
    ));
    assert!(
        format!("{error:?}").contains("ValidationWorkUnits"),
        "{error:?}"
    );
}

#[test]
fn ordinary_reader_rejects_restricted_snapshots_on_public_defaults() {
    for case in 0..3 {
        let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
        let interface = &fixture.interface;
        let original = &interface.default_templates().records()[0];
        let reference = &original.references().types()[0];
        let restricted = SourceAccessDomainV1::from_constraints(
            vec![SourceAccessConstraintV1::Cone(fixture.cone.identity())],
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap();
        let universal = SourceAccessDomainV1::universal();
        let (direct, slot, target) = match case {
            0 => (restricted, None, universal),
            1 => (universal.clone(), None, restricted),
            2 => (universal.clone(), Some(restricted), universal),
            _ => unreachable!(),
        };
        let witness =
            ExportDefaultAccessWitnessV1::try_new(fixture.owner, direct, slot, target).unwrap();
        let references = ExportDefaultReferenceSetV1::try_new(
            vec![],
            vec![],
            vec![ExportDefaultReferenceV1::new(
                reference.target().clone(),
                reference.definition_origin().clone(),
                witness,
            )],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let template = ExportDefaultTemplateV1::try_new(
            original.key(),
            original.definition_root(),
            original.definition_path().clone(),
            original.locals().clone(),
            original.body().clone(),
            original.result().clone(),
            original.allows_suspend(),
            original.type_parameters().clone(),
            original.receiver().clone(),
            original.value_parameters().clone(),
            references,
            original.definition_origin().clone(),
        )
        .unwrap();
        fixture.interface = CrossConeHirInterfaceSectionV1::new(
            interface.public_bindings().clone(),
            interface.nominal_interfaces().clone(),
            interface.callable_interfaces().clone(),
            interface.property_interfaces().clone(),
            interface.type_aliases().clone(),
            interface.source_interfaces().clone(),
            CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap(),
            interface.constants().clone(),
            interface.definition_sources().clone(),
            interface.external_references().clone(),
        );
        let bytes = fixture.artifact();
        let result = validate_until_type_alias(&bytes).validate_source_interfaces(vec![]);
        let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract(
            crate::CrossConeHirDefaultProviderContractError::Template {
                index: 0, source, ..
            },
        )) = result
        else {
            panic!("public declaration must reject restricted access snapshot")
        };
        let crate::CrossConeHirDefaultProviderContractError::PublicWitness(error) = *source else {
            panic!("public witness validation")
        };
        let ExportDefaultPublicWitnessValidationError::Record {
            index: 0, source, ..
        } = *error
        else {
            panic!("reference witness")
        };
        if case == 1 {
            assert_eq!(source, PublicDefaultWitnessError::RestrictedTarget);
        } else {
            assert!(matches!(
                source,
                PublicDefaultWitnessError::CallDomain { actual: None, .. }
            ));
        }
    }
}
