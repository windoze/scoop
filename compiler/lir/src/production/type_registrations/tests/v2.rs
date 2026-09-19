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
        assert!(
            decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err(),
            "the old reader must reject the new relation tags"
        );
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
                provider.map(|_| reference(exact_type("Parent"))),
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
    let semantics = StrongTypeDescriptorSemanticPlanSetV2::from_artifact(
        fixture.semantics.producer(),
        fixture.semantics.target().clone(),
        descriptors,
    );
    StrongTypeRegistrationPlanSetV2::new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        &semantics,
        &fixture.digests,
    )
}

fn foreign_body() -> scoop_identity::PersistentCallableBodyId {
    use scoop_identity::{
        CallableBodyKey, PersistentCallableBodyId, PersistentFunctionId,
        StrongCallableDefinitionOwner,
    };
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
    PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(declaration),
    ))
    .unwrap()
}
