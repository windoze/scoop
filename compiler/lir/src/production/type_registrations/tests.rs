use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeKey, DispatchTableKey, ExactTypeKey,
    LayoutKey, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentLayoutId, PersistentScanId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId,
    RepresentationRole, ScanKey, ScanRole, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{
    CanonicalLirFoundation, DecodedStrongTypeRegistrationPlanV1, DigestNodeV1, RefScan,
    RuntimeTypeMappingRecord, TypeInstanceShapeV1,
};
use scoop_wire::{decode_canonical, encode};

#[test]
fn joins_every_type_to_its_descriptor_layout_and_content_nodes() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(
        plans.target(),
        &crate::target_fixtures::DARWIN_AARCH64.wire_id()
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
    }
}

#[test]
fn wire_reader_rejects_type_registrations_without_runtime_relations() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];
    let mut encoded = encode(plan).unwrap();
    assert_eq!(&encoded[..2], &[0xb8, 25]);
    assert_eq!(&encoded[encoded.len() - 5..], &[0x18, 30, 0xa1, 0, 1]);
    encoded[1] = 24;
    encoded.truncate(encoded.len() - 5);
    assert!(decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(&encoded).is_err());
    assert_eq!(&encoded[..2], &[0xb8, 24]);
    assert_eq!(&encoded[encoded.len() - 5..], &[0x18, 29, 0xa1, 0, 0]);
    encoded.splice(0..2, [0xb7]);
    encoded.truncate(encoded.len() - 5);
    assert!(decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(&encoded).is_err());
    assert_eq!(
        &encoded[encoded.len() - 7..],
        &[0x18, 0x1c, 0xa2, 0, 1, 1, 0]
    );
    encoded[0] = 0xb6;
    encoded.truncate(encoded.len() - 7);

    assert!(decode_canonical::<DecodedStrongTypeRegistrationPlanV1>(&encoded,).is_err());
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

mod fixture;
use fixture::*;
mod artifacts;
use artifacts::*;
mod digests;
use digests::*;

mod v2;
