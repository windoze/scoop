use scoop_wire::{BudgetMeter, DecodeLimits, WireErrorKind, WirePath, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalExternalHirReferenceRolesV1, CanonicalExternalHirReferencesV1,
    DecodedExternalHirReferenceV1, ExternalHirReferenceBuildError, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSetBuildError, ExternalHirReferenceV1,
};

pub(super) mod support;
use support::Fixture;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn six_field_call_sites_round_trip_without_erasing_repeated_unit_arguments() {
    let fixture = Fixture::new();
    let site = fixture.site(7, vec![0, 3]).unwrap();
    let bytes = encode(&site).unwrap();
    assert_eq!(bytes[0], 0xa6);
    assert_eq!(site.arguments(), &[fixture.unit, fixture.unit]);
    assert_eq!(site.witness_indices(), &[0, 3]);
    let decoded: DecodedHirDependencyCallSiteV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded
            .resolve(&mut fixture.graph(), &mut meter(), &WirePath::root())
            .unwrap(),
        site
    );
}

#[test]
fn call_routes_are_nonempty_ordered_and_unique_on_both_sides_of_the_codec() {
    let fixture = Fixture::new();
    for (indices, expected) in [
        (vec![], HirDependencyCallSiteBuildError::EmptyWitnessIndices),
        (
            vec![1, 0],
            HirDependencyCallSiteBuildError::WitnessIndexOrder { index: 1 },
        ),
        (
            vec![0, 0],
            HirDependencyCallSiteBuildError::WitnessIndexOrder { index: 1 },
        ),
    ] {
        assert_eq!(fixture.site(0, indices), Err(expected));
    }
    let site = fixture.site(0, vec![0, 1]).unwrap();
    for suffix in [vec![0x80], vec![0x82, 1, 0], vec![0x82, 0, 0]] {
        let mut bytes = encode(&site).unwrap();
        bytes.truncate(bytes.len() - 3);
        bytes.extend(suffix);
        let decoded: DecodedHirDependencyCallSiteV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture.graph(), &mut meter(), &WirePath::root()),
            Err(HirDependencyCallSiteResolutionError::Shape(_))
        ));
    }
}

#[test]
fn producer_orders_positions_while_reader_rejects_duplicates_and_reordering() {
    let fixture = Fixture::new();
    let first = fixture.site(1, vec![0]).unwrap();
    let second = fixture.site(2, vec![0]).unwrap();
    assert_eq!(
        CanonicalHirDependencyCallSitesV1::try_new(vec![second.clone(), first.clone()])
            .unwrap()
            .records(),
        &[first.clone(), second.clone()]
    );
    for sites in [[first.clone(), first.clone()], [second, first]] {
        let bytes = [
            vec![0x82],
            encode(&sites[0]).unwrap(),
            encode(&sites[1]).unwrap(),
        ]
        .concat();
        let decoded: DecodedCanonicalHirDependencyCallSitesV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture.graph(), &mut meter(), &WirePath::root()),
            Err(HirDependencyCallSiteResolutionError::Shape(_))
        ));
    }
}

#[test]
fn call_metadata_shares_work_and_memory_budgets() {
    let fixture = Fixture::new();
    let decoded: DecodedHirDependencyCallSiteV1 = decode_canonical(
        &encode(&fixture.site(0, vec![0]).unwrap()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut first = meter();
    decoded
        .clone()
        .resolve(&mut fixture.graph(), &mut first, &WirePath::root())
        .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: first.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    decoded
        .clone()
        .resolve(&mut fixture.graph(), &mut shared, &WirePath::root())
        .unwrap();
    assert!(matches!(
        decoded
            .clone()
            .resolve(&mut fixture.graph(), &mut shared, &WirePath::root()),
        Err(HirDependencyCallSiteResolutionError::Resource(_))
    ));
    let mut exhausted = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        decoded.resolve(&mut fixture.graph(), &mut exhausted, &WirePath::root()),
        Err(HirDependencyCallSiteResolutionError::Resource(_))
    ));
}

#[test]
fn concrete_function_roles_require_actual_sites_and_valid_witness_indices() {
    let fixture = Fixture::new();
    let build = |role, sites| {
        ExternalHirReferenceV1::try_new(
            fixture.provider,
            fixture.target(),
            CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
            fixture.witnesses(),
            sites,
            Default::default(),
        )
    };
    assert_eq!(
        build(
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            Default::default()
        ),
        Err(ExternalHirReferenceBuildError::MissingCallSites)
    );
    let sites = |indices| {
        CanonicalHirDependencyCallSitesV1::try_new(vec![fixture.site(0, indices).unwrap()]).unwrap()
    };
    assert_eq!(
        build(
            ExternalHirReferenceRoleV1::DefaultDependency,
            sites(vec![0])
        ),
        Err(ExternalHirReferenceBuildError::UnexpectedCallSites)
    );
    assert_eq!(
        build(
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            sites(vec![1])
        ),
        Err(ExternalHirReferenceBuildError::CallWitnessIndex { site: 0, index: 1 })
    );
    let record = build(
        ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        sites(vec![0]),
    )
    .unwrap();
    let decoded: DecodedExternalHirReferenceV1 =
        decode_canonical(&encode(&record).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.graph()).unwrap(), record);
}

#[test]
fn old_four_field_references_are_rejected_even_without_calls() {
    let fixture = Fixture::new();
    let record = ExternalHirReferenceV1::try_new(
        fixture.provider,
        fixture.target(),
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::DefaultDependency,
        ])
        .unwrap(),
        fixture.witnesses(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut bytes = encode(&record).unwrap();
    bytes[0] = 0xa4;
    bytes.truncate(bytes.len() - 2);
    assert!(matches!(
        decode_canonical::<DecodedExternalHirReferenceV1>(&bytes, DecodeLimits::default())
            .unwrap_err()
            .kind(),
        WireErrorKind::InvalidLength {
            expected: 6,
            actual: 4
        }
    ));
}

#[test]
fn one_executable_position_cannot_claim_two_distinct_targets() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    let second_key = scoop_identity::SourceDeclarationKey::function(
        scoop_identity::SourceDeclarationSite::new(
            fixture.provider,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap(),
        scoop_identity::CanonicalIdentifier::new("other").unwrap(),
        0,
        None,
        vec![],
    );
    let second =
        crate::ExternalHirTargetV1::Callable(scoop_identity::CallableTemplateOrigin::Function(
            scoop_identity::PersistentFunctionId::from_source_declaration(&second_key).unwrap(),
        ));
    let records = [fixture.target(), second]
        .into_iter()
        .map(|target| {
            ExternalHirReferenceV1::try_new(
                fixture.provider,
                target,
                CanonicalExternalHirReferenceRolesV1::try_new(vec![
                    ExternalHirReferenceRoleV1::ConcreteSelectedUse,
                ])
                .unwrap(),
                fixture.witnesses(),
                CanonicalHirDependencyCallSitesV1::try_new(vec![other.site(0, vec![0]).unwrap()])
                    .unwrap(),
                Default::default(),
            )
            .unwrap()
        })
        .collect();
    assert!(matches!(
        CanonicalExternalHirReferencesV1::try_new(records),
        Err(ExternalHirReferenceSetBuildError::DuplicateCallPosition(_))
    ));
}
