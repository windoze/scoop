use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, DispatchTableKey, ExactTypeKey, LayoutKey, LinkageClass,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentLayoutId, PersistentScanId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, RepresentationRole,
    ScanKey, ScanRole, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{
    CanonicalLirFoundation, DecodedStrongTypeRegistrationPlanV1, DigestNodeV1, RefScan,
    RuntimeTypeMappingRecord, StrongExternalLirBridgeSurfaceV1, TypeInstanceShapeV1,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn joins_every_type_to_its_descriptor_layout_and_digest_writers() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(
        plans.target(),
        &crate::LirTargetProfile::DARWIN_AARCH64.wire_id()
    );
    assert_eq!(plans.registrations().len(), 2);
    assert!(
        plans
            .registrations()
            .windows(2)
            .all(|pair| pair[0].exact_type() < pair[1].exact_type())
    );
    for plan in plans.registrations() {
        assert_eq!(
            plan.symbol().key(),
            PersistentSymbolKey::TypeRegistration(plan.exact_type())
        );
        assert_eq!(
            plan.descriptor_symbol().key(),
            PersistentSymbolKey::TypeDescriptor(plan.exact_type())
        );
        assert_eq!(
            plan.layout_symbol().key(),
            PersistentSymbolKey::Layout(plan.layout())
        );
        assert_eq!(
            plan.runtime_type(),
            RuntimeTypeMappingRecord::new(plan.exact_type())
                .unwrap()
                .runtime_type()
        );
        let registration = node(&fixture.digests, plan.registration_fingerprint_node());
        assert_eq!(registration.direct_inputs().len(), 3);
        assert_eq!(
            registration.patch_intents()[0].id(),
            plan.registration_definition_patch()
        );
        assert!(
            node(&fixture.digests, plan.descriptor_definition_node())
                .patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.descriptor_definition_patch())
        );
        assert!(
            node(&fixture.digests, plan.layout_fingerprint_node())
                .patch_intents()
                .iter()
                .any(|patch| patch.id() == plan.layout_fingerprint_patch())
        );
    }
}

#[test]
fn wire_reader_reconstructs_the_complete_type_descriptor_semantics() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let decoded = plans
        .registrations()
        .iter()
        .map(|plan| {
            decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(
                &encode(plan).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap()
        })
        .collect();
    let external_bridges =
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap();

    let validated = crate::validate_types(
        decoded,
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        &external_bridges,
        &fixture.digests,
    )
    .unwrap();

    assert_eq!(validated, fixture.semantics);
}

#[test]
fn wire_reader_rejects_the_old_27_field_type_registration_plan() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];
    let mut encoded = encode(plan).unwrap();
    assert_eq!(&encoded[..2], &[0xb8, 0x1c]);
    assert_eq!(
        &encoded[encoded.len() - 7..],
        &[0x18, 0x1c, 0xa2, 0, 1, 1, 0]
    );
    encoded[1] = 0x1b;
    encoded.truncate(encoded.len() - 7);

    assert!(
        decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(&encoded, DecodeLimits::default(),)
            .is_err()
    );
}

#[test]
fn records_and_round_trips_the_exact_typed_itable_directory() {
    let fixture = Fixture::new(Options {
        first_type_has_itable: true,
        ..Options::default()
    });
    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];
    let expected = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.descriptor_definition_plan(),
        DefinitionAtomRole::RuntimeRecord,
        DefinitionAtomSubkey::ExactType(plan.exact_type()),
    ))
    .unwrap();
    assert_eq!(
        plan.itable_directory(),
        TypeDescriptorITableDirectoryV1::Defined(expected.id())
    );

    let decoded = plans
        .registrations()
        .iter()
        .map(|plan| {
            decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(
                &encode(plan).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap()
        })
        .collect();
    let external_bridges =
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap();
    assert_eq!(
        crate::validate_types(
            decoded,
            crate::LirTargetProfile::DARWIN_AARCH64,
            &fixture.foundation,
            &fixture.identities,
            &external_bridges,
            &fixture.digests,
        )
        .unwrap(),
        fixture.semantics
    );
}

#[test]
fn requires_complete_type_registration_coverage() {
    let fixture = Fixture::new(Options {
        omit_last_registration: true,
        ..Options::default()
    });

    assert!(matches!(
        fixture.build(),
        Err(StrongTypeRegistrationPlanBuildError::TypeSet { expected, actual })
            if expected.len() == 2 && actual.len() == 1
    ));
}

#[test]
fn requires_runtime_type_and_managed_object_layout_relations() {
    assert!(matches!(
        Fixture::new(Options {
            omit_first_runtime_type: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::RuntimeTypeSet { actual, .. })
            if actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_first_layout: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::MissingLayout(_))
    ));
}

#[test]
fn requires_registration_descriptor_and_layout_symbols() {
    for options in [
        Options {
            omit_registration_symbol: true,
            ..Options::default()
        },
        Options {
            omit_descriptor_symbol: true,
            ..Options::default()
        },
        Options {
            omit_layout_symbol: true,
            ..Options::default()
        },
    ] {
        assert!(matches!(
            Fixture::new(options).build(),
            Err(StrongTypeRegistrationPlanBuildError::MissingSymbol(_))
        ));
    }
}

#[test]
fn requires_the_exact_descriptor_diagnostic_atom() {
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_diagnostic: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::DescriptorAssociatedAtomSet {
            actual,
            ..
        }) if actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            extra_descriptor_atom: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::DescriptorAssociatedAtomSet {
            actual,
            ..
        }) if actual.len() == 2
    ));
}

#[test]
fn requires_the_exact_itable_directory_atom_for_nonempty_itables() {
    assert!(matches!(
        Fixture::new(Options {
            first_type_has_itable: true,
            omit_itable_directory: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::DescriptorAssociatedAtomSet {
            expected,
            actual,
            ..
        }) if expected.len() == 2 && actual.len() == 1
    ));
}

#[test]
fn requires_a_leaf_registration_object_definition() {
    assert!(matches!(
        Fixture::new(Options {
            registration_object_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::RegistrationObjectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            registration_object_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::RegistrationObjectPatches { .. })
    ));
}

#[test]
fn requires_exact_registration_inputs_and_digest_writers() {
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::DirectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::PatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::MissingPatch { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_layout_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongTypeRegistrationPlanBuildError::MissingPatch { .. })
    ));
}

mod fixture;
use fixture::*;
mod artifacts;
use artifacts::*;
mod digests;
use digests::*;

fn node(plan: &StrongDigestFinalizationPlanV1, id: DigestNodeId) -> &DigestNodeV1 {
    plan.nodes().iter().find(|node| node.id() == id).unwrap()
}

mod v2;
