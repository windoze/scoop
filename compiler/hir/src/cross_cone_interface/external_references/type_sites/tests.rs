use scoop_wire::{BudgetMeter, DecodeLimits, WireErrorKind, WirePath, decode_canonical, encode};

use super::super::call_sites::Fixture;
use super::*;
use crate::{
    CanonicalExternalHirReferenceRolesV1, DecodedExternalHirReferenceV1,
    ExternalHirReferenceBuildError, ExternalHirReferenceRoleV1, ExternalHirReferenceV1,
    ExternalHirTargetV1,
};

mod declarations;
mod nominals;
mod relations;
mod storage;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn site(fixture: &Fixture, index: u32, role: HirExpressionTypeRoleV1) -> HirDependencyTypeSiteV1 {
    let call = fixture.site(index, vec![0]).unwrap();
    HirDependencyTypeSiteV1::new(call.position(), call.origin().clone(), role, fixture.unit)
}

#[test]
fn expression_type_occurrences_preserve_typed_roots_origins_and_roles() {
    let fixture = Fixture::new();
    for (index, role) in [
        HirExpressionTypeRoleV1::Value,
        HirExpressionTypeRoleV1::SizeOf,
        HirExpressionTypeRoleV1::AlignOf,
        HirExpressionTypeRoleV1::TypeTest,
        HirExpressionTypeRoleV1::ArrayElement,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(encode(&role).unwrap(), vec![index as u8 + 1]);
        let original = site(&fixture, 7, role);
        let bytes = encode(&original).unwrap();
        assert_eq!(&bytes[..3], &[0xa6, 0, 1]);
        let mut retired = vec![0xa5];
        retired.extend_from_slice(&bytes[3..]);
        assert!(
            decode_canonical::<DecodedHirDependencyTypeSiteV1>(&retired, DecodeLimits::default())
                .is_err()
        );
        let decoded: DecodedHirDependencyTypeSiteV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(
            decoded
                .resolve(&mut fixture.graph(), &mut meter(), &WirePath::root())
                .unwrap(),
            original
        );
    }
    assert!(decode_canonical::<HirExpressionTypeRoleV1>(&[6], DecodeLimits::default()).is_err());
}

#[test]
fn type_site_order_is_position_then_role_and_never_deduplicates_actual_uses() {
    let fixture = Fixture::new();
    let value = site(&fixture, 1, HirExpressionTypeRoleV1::Value);
    let operand = site(&fixture, 1, HirExpressionTypeRoleV1::SizeOf);
    let table = CanonicalHirDependencyTypeSitesV1::try_new(
        vec![operand.clone(), value.clone()],
        &mut meter(),
    )
    .unwrap();
    assert_eq!(table.records(), &[value.clone(), operand.clone()]);
    for pair in [
        [value.clone(), value],
        [
            operand.clone(),
            site(&fixture, 0, HirExpressionTypeRoleV1::Value),
        ],
    ] {
        let bytes = [
            vec![0x82],
            encode(&pair[0]).unwrap(),
            encode(&pair[1]).unwrap(),
        ]
        .concat();
        let raw: DecodedCanonicalHirDependencyTypeSitesV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert!(matches!(
            raw.resolve(&mut fixture.graph(), &mut meter(), &WirePath::root()),
            Err(HirDependencyTypeSiteResolutionError::Shape(_))
        ));
    }
    assert!(
        CanonicalHirDependencyTypeSitesV1::try_new(vec![operand.clone(), operand], &mut meter())
            .is_err()
    );
}

#[test]
fn type_reference_roles_require_actual_nominal_sites_and_reject_the_old_wire() {
    let fixture = Fixture::new();
    let nominal = ExternalHirTargetV1::Nominal(scoop_identity::NominalDeclarationOwner::Concrete(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ));
    let build = |target, role, sites| {
        ExternalHirReferenceV1::try_new(
            fixture.provider,
            target,
            CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
            crate::CanonicalDependencyBindingWitnessesV1::try_new(vec![]).unwrap(),
            Default::default(),
            sites,
        )
    };
    let records = || {
        CanonicalHirDependencyTypeSitesV1::try_new(
            vec![site(&fixture, 0, HirExpressionTypeRoleV1::Value)],
            &mut meter(),
        )
        .unwrap()
    };
    assert_eq!(
        build(
            nominal,
            ExternalHirReferenceRoleV1::ExecutableTypeDependency,
            Default::default()
        ),
        Err(ExternalHirReferenceBuildError::MissingTypeSites)
    );
    assert_eq!(
        build(
            nominal,
            ExternalHirReferenceRoleV1::SignatureDependency,
            records()
        ),
        Err(ExternalHirReferenceBuildError::UnexpectedTypeSites)
    );
    assert_eq!(
        build(
            fixture.target(),
            ExternalHirReferenceRoleV1::ExecutableTypeDependency,
            records()
        ),
        Err(ExternalHirReferenceBuildError::UnexpectedTypeSites)
    );
    let original = build(
        nominal,
        ExternalHirReferenceRoleV1::ExecutableTypeDependency,
        records(),
    )
    .unwrap();
    let bytes = encode(&original).unwrap();
    assert_eq!(bytes[0], 0xa6);
    let raw: DecodedExternalHirReferenceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&raw).unwrap(), bytes);
    for length in [5, 7] {
        let mut altered = bytes.clone();
        altered[0] = 0xa0 + length;
        assert!(matches!(
            decode_canonical::<DecodedExternalHirReferenceV1>(&altered, DecodeLimits::default())
                .unwrap_err()
                .kind(),
            WireErrorKind::InvalidLength { expected: 6, .. }
        ));
    }
}

#[test]
fn type_site_resolution_shares_cumulative_work_and_allocation_limits() {
    let fixture = Fixture::new();
    let bytes = encode(&site(&fixture, 0, HirExpressionTypeRoleV1::Value)).unwrap();
    let raw: DecodedHirDependencyTypeSiteV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut baseline = meter();
    raw.clone()
        .resolve(&mut fixture.graph(), &mut baseline, &WirePath::root())
        .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: baseline.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    raw.clone()
        .resolve(&mut fixture.graph(), &mut shared, &WirePath::root())
        .unwrap();
    assert!(
        raw.clone()
            .resolve(&mut fixture.graph(), &mut shared, &WirePath::root())
            .is_err()
    );
    let mut exhausted = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    });
    assert!(
        raw.resolve(&mut fixture.graph(), &mut exhausted, &WirePath::root())
            .is_err()
    );
}
