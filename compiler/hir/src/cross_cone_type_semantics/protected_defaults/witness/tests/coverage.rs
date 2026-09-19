use super::*;
use crate::cross_cone_type_semantics::inheritance::inheritance_interface_fixture;
use scoop_identity::ConeIdentity;

struct Roots(CanonicalProtectedSlotRefsV1);
impl ProtectedDefaultRootSlotSemanticAuthority<&'static str> for Roots {
    fn default_root_slots(
        &self,
        _: CallableTemplateOrigin,
    ) -> Result<&CanonicalProtectedSlotRefsV1, &'static str> {
        Ok(&self.0)
    }
}
fn check(case: u8) -> Result<(), ProtectedDefaultDomainCoverageError<&'static str>> {
    let mut bundle = inheritance_interface_fixture();
    bundle
        .fixture
        .graph
        .visibility(bundle.derived, DeclaredVisibilityV1::Internal);
    let graph_source = bundle.fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let domains = graph
        .replay_nominal_domains(bundle.derived.exact, &mut meter())
        .unwrap()
        .to_record();
    bundle.change(bundle.derived, |record| {
        *record = NominalInheritanceInterfaceV1::try_new(
            record.edges().clone(),
            domains,
            record.constructors().clone(),
            record.slots().clone(),
            record.protected_members().clone(),
            record.slot_schemas().clone(),
        )
        .unwrap();
    });
    let protected = bundle
        .protected
        .validate_sources(
            &graph,
            &CanonicalNominalRepresentationSupportV1::try_new(vec![]).unwrap(),
            &mut bundle.fixture,
            &mut meter(),
        )
        .unwrap();
    let inheritance = bundle
        .table
        .validate_interfaces(&graph, protected, &mut bundle.fixture, &mut meter())
        .unwrap();
    let InheritanceCallableDeclarationV1::Function(function) = bundle.target else {
        panic!("fixture has a function slot")
    };
    let declaration = CallableTemplateOrigin::Function(function);
    let original = bundle.fixture.payload(
        bundle.derived,
        declaration,
        vec![],
        SignatureTypeKey::Nominal(nominal(bundle.fixture.unit)),
    );
    let payload = NominalSourceCallablePayloadV1::try_new(
        declaration,
        bundle.derived.source,
        original.type_parameters().clone(),
        original.parameters().clone(),
        original.result().clone(),
        original.effects(),
        CallableModalityV1::Final,
        CanonicalProtectedSlotRefsV1::try_new(vec![bundle.slot]).unwrap(),
    )
    .unwrap();
    let record = NominalSupportCallableInterfaceV1::try_new(
        declaration,
        bundle
            .fixture
            .access(bundle.derived, DeclaredVisibilityV1::Public),
        payload,
    )
    .unwrap();
    let source = ProtectedDefaultOwnerSourceV1::NominalSupport(
        record
            .validate_source(&graph, &mut bundle.fixture, &mut meter())
            .unwrap(),
    );
    let direct =
        PersistentAccessDomainV1::try_from_constraints(vec![PersistentAccessConstraintV1::Cone(
            ConeIdentity::CORE,
        )])
        .unwrap();
    let target = match case {
        1 => PersistentAccessDomainV1::empty(),
        2 => direct.clone(),
        _ => PersistentAccessDomainV1::universal(),
    };
    let slot_domain = if case == 4 {
        direct.clone()
    } else {
        PersistentAccessDomainV1::universal()
    };
    let slots = if case == 3 {
        vec![]
    } else {
        vec![ProtectedDefaultSlotCallDomainV1::new(
            bundle.slot,
            PersistentSlotContractDomainV1::new(slot_domain),
        )]
    };
    let witness = ProtectedDefaultAccessWitnessV1::param_free(
        declaration,
        lookup(direct),
        CanonicalProtectedDefaultSlotCallDomainsV1::try_new(slots).unwrap(),
        lookup(target.clone()),
    )
    .unwrap();
    let key = ProtectedDefaultTemplateKeyV1::try_new(declaration, 0).unwrap();
    let profile = ExpectedProfile {
        key,
        profile: ProtectedDefaultWitnessSourceProfileV1::ParamFree,
    };
    let CheckedProtectedDefaultWitnessSourceV1::ParamFree(checked) = witness
        .validate_source_profile(key, source, &profile, &mut meter())
        .unwrap()
    else {
        panic!("param-free owner expected")
    };
    let target = graph.validate_access_domain(&target, &mut meter()).unwrap();
    let roots = Roots(
        CanonicalProtectedSlotRefsV1::try_new(if case == 5 { vec![] } else { vec![bundle.slot] })
            .unwrap(),
    );
    checked
        .validate_domains(&graph, inheritance, &target, &roots, &mut meter())
        .map(|_| ())
}

#[test]
fn default_coverage_joins_public_slot_and_internal_owner_direct_domain() {
    check(0).unwrap();
}
#[test]
fn direct_and_slot_call_domains_are_each_checked_in_full() {
    assert!(matches!(
        check(1),
        Err(ProtectedDefaultDomainCoverageError::DirectCoverage)
    ));
    assert!(matches!(
        check(2),
        Err(ProtectedDefaultDomainCoverageError::SlotCoverage(_))
    ));
}
#[test]
fn missing_roots_and_claimed_slot_domains_cannot_shrink_coverage() {
    for case in [3, 5] {
        assert!(matches!(
            check(case),
            Err(ProtectedDefaultDomainCoverageError::SlotInventory)
        ));
    }
    assert!(matches!(
        check(4),
        Err(ProtectedDefaultDomainCoverageError::DomainMismatch)
    ));
}
