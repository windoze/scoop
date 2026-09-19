use super::*;
use crate::cross_cone_type_semantics::inheritance::inheritance_interface_fixture;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::*;
use scoop_wire::DecodeLimits;

mod authority;
mod cases;
mod source;
mod template;
use authority::Authority;

#[derive(Clone, Copy, Default)]
enum Case {
    #[default]
    Valid,
    Metadata,
    RejectMetadata,
    WrongTarget,
    Origin,
    DomainLie,
    DirectCoverage,
    Generic,
    GenericReject,
    GenericLie,
    EmptyReferences,
}
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn run(
    case: Case,
) -> Result<(bool, usize, usize), ProtectedDefaultReferenceSemanticError<&'static str>> {
    run_with_limits(case, DecodeLimits::default())
}
fn run_with_limits(
    case: Case,
    limits: DecodeLimits,
) -> Result<(bool, usize, usize), ProtectedDefaultReferenceSemanticError<&'static str>> {
    let mut bundle = inheritance_interface_fixture();
    let generic = matches!(case, Case::Generic | Case::GenericReject);
    let owner = source::owner(&mut bundle.fixture, bundle.derived.source, generic);
    let target = if matches!(case, Case::DirectCoverage) {
        let hidden = bundle
            .fixture
            .graph
            .add("Hidden", SourceNominalKind::Struct, &[]);
        bundle
            .fixture
            .graph
            .visibility(hidden, DeclaredVisibilityV1::Internal);
        hidden.source
    } else {
        bundle.fixture.unit.source
    };
    let record = source::callable(&mut bundle.fixture, owner, target);
    let graph_source = bundle.fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        graph_source.records.values(),
        graph_source.keys.keys().copied(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let mut source_foundation = bundle.fixture.clone();
    let source = record.check(&graph, &mut source_foundation);
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
    let key = ProtectedDefaultTemplateKeyV1::try_new(record.declaration(), 0).unwrap();
    let origin = bundle.fixture.graph.origins[&owner].clone();
    let profile = if generic || matches!(case, Case::GenericLie | Case::EmptyReferences) {
        ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata
    } else {
        ProtectedDefaultWitnessSourceProfileV1::ParamFree
    };
    let target_domain = graph.replay_nominal_access(target, &mut meter()).unwrap();
    let witness = if generic || matches!(case, Case::GenericLie) {
        ProtectedDefaultAccessWitnessV1::generic_source_metadata(key.owner()).unwrap()
    } else {
        ProtectedDefaultAccessWitnessV1::param_free(
            key.owner(),
            PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal()),
            CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
            PersistentLookupDomainV1::new(if matches!(case, Case::DomainLie) {
                PersistentAccessDomainV1::empty()
            } else {
                target_domain.lookup().domain().clone()
            }),
        )
        .unwrap()
    };
    let template = template::build(key, target, &origin, witness, case);
    let mut authority = Authority {
        key,
        target,
        origin,
        profile,
        roots: CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        case,
        concrete: 0,
        metadata: 0,
    };
    let checked = template.validate_reference_access_semantics(
        source,
        &graph,
        inheritance,
        &mut authority,
        &mut BudgetMeter::new(limits),
        &WirePath::root(),
    )?;
    assert_eq!(checked.template().key(), key);
    assert_eq!(checked.owner().key(), key);
    assert_eq!(checked.body().references(), template.references());
    Ok((
        matches!(
            checked,
            CheckedProtectedDefaultReferenceReplayV1::GenericSourceMetadata(_)
        ),
        authority.concrete,
        authority.metadata,
    ))
}
