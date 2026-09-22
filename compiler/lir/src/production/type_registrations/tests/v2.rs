//! These fixtures exercise registration constituents, not a completed artifact.

use super::*;
use crate::{
    DecodedStrongTypeRegistrationPlanV2, StrongTypeDescriptorRefV2 as Descriptor,
    StrongTypeDescriptorSemanticPlanSetV2, StrongTypeDescriptorSemanticPlanV2,
    StrongTypeDispatchCallableRefV2 as Callable, StrongTypeItableSemanticPlanV2,
    StrongTypeVtableSemanticPlanV2,
};

#[test]
fn legacy_relations_keep_the_complete_registration_bytes_in_v2() {
    let fixture = Fixture::new(Options {
        first_type_has_itable: true,
        ..Options::default()
    });
    let v1 = fixture.build().unwrap();
    let v2 = build(&fixture, None).unwrap();
    for (old, new) in v1.registrations().iter().zip(v2.registrations()) {
        let bytes = encode(new).unwrap();
        assert_eq!(bytes, encode(old).unwrap());
        let decoded: DecodedStrongTypeRegistrationPlanV2 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
    }
}

#[test]
fn dependency_parent_interface_and_dispatch_survive_the_registration_wire() {
    let fixture = Fixture::new(Options {
        first_type_has_itable: true,
        ..Options::default()
    });
    let plans = build(&fixture, Some(ConeIdentity::CORE)).unwrap();
    for plan in plans.registrations() {
        let bytes = encode(plan).unwrap();
        let decoded: DecodedStrongTypeRegistrationPlanV2 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let shared: DecodedStrongTypeRegistrationPlanV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&shared).unwrap(), bytes);
        assert!(
            matches!(plan.semantic().parent(), Some(Descriptor::DependencyExternal { provider, .. })
            if provider == ConeIdentity::CORE)
        );
        assert!(
            matches!(plan.semantic().vtable().slots(), [Callable::DependencyExternal { provider, .. }]
            if *provider == ConeIdentity::CORE)
        );
        for table in plan.semantic().itables() {
            assert!(
                matches!(table.interface(), Descriptor::DependencyExternal { provider, .. }
                if provider == ConeIdentity::CORE)
            );
        }
        let mut extra_field = bytes.clone();
        assert_eq!(&extra_field[..2], &[0xb8, 28]);
        extra_field[1] = 29;
        extra_field.extend_from_slice(&[0x18, 29, 0]);
        assert!(
            decode_canonical::<DecodedStrongTypeRegistrationPlanV2>(
                &extra_field,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    let changed_provider = build(&fixture, Some(ConeIdentity::SINGLE_FILE)).unwrap();
    for (original, changed) in plans
        .registrations()
        .iter()
        .zip(changed_provider.registrations())
    {
        assert_ne!(encode(original).unwrap(), encode(changed).unwrap());
    }
}

#[test]
fn v2_uses_the_same_complete_local_definition_checks() {
    for options in [
        Options {
            omit_first_layout: true,
            ..Options::default()
        },
        Options {
            omit_descriptor_symbol: true,
            ..Options::default()
        },
        Options {
            omit_descriptor_diagnostic: true,
            ..Options::default()
        },
        Options {
            omit_layout_patch: true,
            ..Options::default()
        },
    ] {
        let fixture = Fixture::new(options);
        assert_eq!(
            build(&fixture, None).unwrap_err(),
            fixture.build().unwrap_err()
        );
    }
}

fn build(
    fixture: &Fixture,
    provider: Option<ConeIdentity>,
) -> Result<StrongTypeRegistrationPlanSetV2, StrongTypeRegistrationPlanBuildError> {
    StrongTypeRegistrationPlanSetV2::new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        &semantics(fixture, provider),
        &fixture.digests,
    )
}

fn semantics(
    fixture: &Fixture,
    provider: Option<ConeIdentity>,
) -> StrongTypeDescriptorSemanticPlanSetV2 {
    let body = foreign_body();
    let descriptors = fixture
        .semantics
        .descriptors()
        .iter()
        .map(|old| {
            let reference = |exact| match provider {
                Some(provider) => Descriptor::DependencyExternal { provider, exact },
                None => Descriptor::Local(exact),
            };
            let slots = || {
                provider
                    .map(|provider| Callable::DependencyExternal { provider, body })
                    .into_iter()
                    .collect()
            };
            StrongTypeDescriptorSemanticPlanV2::from_artifact(
                old.exact_type(),
                old.diagnostic_name().to_owned(),
                old.instance_layout(),
                old.instance_scan(),
                old.instance_shape().clone(),
                old.inline_scan(),
                provider.map(|_| reference(foreign_exact("Parent"))),
                StrongTypeVtableSemanticPlanV2::from_artifact(old.vtable().table(), slots()),
                old.itables()
                    .iter()
                    .map(|table| {
                        StrongTypeItableSemanticPlanV2::from_artifact(
                            table.table(),
                            reference(table.interface().exact_type()),
                            slots(),
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    StrongTypeDescriptorSemanticPlanSetV2::from_artifact(
        fixture.semantics.producer(),
        fixture.semantics.target().clone(),
        descriptors,
    )
}

fn foreign_body() -> scoop_identity::PersistentCallableBodyId {
    scoop_identity::PersistentCallableBodyId::from_key(&scoop_identity::CallableBodyKey::strong(
        foreign_owner(),
    ))
    .unwrap()
}

fn foreign_owner() -> scoop_identity::StrongCallableDefinitionOwner {
    use scoop_identity::{PersistentFunctionId, StrongCallableDefinitionOwner};
    let declaration =
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("dispatch_target").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    StrongCallableDefinitionOwner::Function(declaration)
}

#[test]
fn complete_registration_surface_preserves_the_new_reference_sums() {
    let fixture = Fixture::new(Options {
        first_type_has_itable: true,
        ..Options::default()
    });
    let producer = fixture.foundation.producer();
    let surface = crate::StrongRegistrationProductionSurfaceV2::from_semantics(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.digests,
        fixture.identities.clone(),
        crate::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(
            &fixture.foundation,
        )
        .unwrap(),
        semantics(&fixture, Some(ConeIdentity::CORE)),
        crate::StrongSafepointSemanticPlanSetV1::from_artifact(producer, Vec::new()),
        crate::StrongImmortalObjectSemanticPlanSetV1::from_artifact(producer, Vec::new()),
        crate::StrongInitializationUnitSemanticPlanSetV2::from_artifact(
            crate::StrongStaticStorageSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            Vec::new(),
        ),
    )
    .unwrap();
    let bytes = encode(&surface).unwrap();
    assert_eq!(bytes[0], 0xa8);
    let decoded: crate::DecodedStrongRegistrationProductionSurfaceV2 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let shared: crate::DecodedStrongRegistrationProductionSurfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&shared).unwrap(), bytes);
    assert!(
        shared
            .validate(
                crate::LirTargetProfile::DARWIN_AARCH64,
                &fixture.foundation,
                &fixture.digests,
                &crate::StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
            )
            .is_err()
    );
    for field_count in [0xa7, 0xa9] {
        let mut malformed = bytes.clone();
        malformed[0] = field_count;
        assert!(
            decode_canonical::<crate::DecodedStrongRegistrationProductionSurfaceV2>(
                &malformed,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
}

fn foreign_exact(name: &str) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source).unwrap(),
    ))
    .unwrap()
}

mod reader;
