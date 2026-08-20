//! Opaque members use the current manifest and object bytes, never frozen headers.

use super::*;
use crate::{DecodedSlibEnvelope, ExtensionRequirement, LogicalMemberKey};

#[test]
fn optional_members_round_trip_without_changing_link_objects_or_semantic_fingerprints() {
    let baseline_bytes = complete_artifact(false);
    let baseline = DecodedSlibEnvelope::open(&baseline_bytes, selection()).unwrap();
    let extended_bytes = extend(&baseline, ExtensionRequirement::Optional);
    let extended = DecodedSlibEnvelope::open(&extended_bytes, selection()).unwrap();
    assert_ne!(baseline_bytes, extended_bytes);
    assert_ne!(
        baseline.manifest().artifact_fingerprint(),
        extended.manifest().artifact_fingerprint()
    );
    assert_eq!(
        baseline.manifest().semantic_fingerprints(),
        extended.manifest().semantic_fingerprints()
    );
    let baseline_objects = crate::verify_code_link_object_members_v1(
        finalized_link_object_fixture().1,
        baseline.manifest().members(),
    )
    .unwrap();
    let extended_objects = crate::verify_code_link_object_members_v1(
        finalized_link_object_fixture().1,
        extended.manifest().members(),
    )
    .unwrap();
    assert_eq!(
        encode(baseline_objects.projection()).unwrap(),
        encode(extended_objects.projection()).unwrap()
    );
    let materialized = open_graph(&extended_bytes)
        .decode_single_cone_link_sections()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .validate_production()
        .unwrap()
        .validate_materializations()
        .unwrap();
    assert_eq!(materialized.scoop_objects().len(), 1);
    assert!(materialized.generated_bridge_objects().is_empty());
}

#[test]
fn link_required_member_is_rejected_after_valid_archive_readback() {
    let bytes = complete_artifact(false);
    let baseline = DecodedSlibEnvelope::open(&bytes, selection()).unwrap();
    let extended_bytes = extend(&baseline, ExtensionRequirement::Link);
    let extended = DecodedSlibEnvelope::open(&extended_bytes, selection()).unwrap();
    let extension = extended
        .manifest()
        .members()
        .iter()
        .find(|member| matches!(member.role(), SlibMemberRole::ExtensionBlob { .. }))
        .unwrap();
    assert!(matches!(
        crate::verify_code_link_object_members_v1(
            finalized_link_object_fixture().1,
            extended.manifest().members(),
        ),
        Err(crate::CodeLinkObjectMemberValidationError::UnsupportedLinkExtension {
            member, capability
        }) if member == extension.id() && capability == extension_capability()
    ));
}

fn extension_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.test", "opaque-member", 1).unwrap()
}

fn extend(baseline: &DecodedSlibEnvelope<'_>, requirement: ExtensionRequirement) -> Vec<u8> {
    let original = baseline.manifest();
    let mut members: Vec<_> = original
        .members()
        .iter()
        .map(|record| {
            SlibMember::new(
                original.cone().identity(),
                record.stable_key().clone(),
                record.role().clone(),
                baseline.member(record.id()).unwrap().to_vec(),
            )
            .unwrap()
        })
        .collect();
    members.push(
        SlibMember::new(
            original.cone().identity(),
            MemberStableKey::ExtensionBlob {
                capability: extension_capability(),
                logical_key: LogicalMemberKey::new(b"opaque".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability: extension_capability(),
                requirement,
            },
            b"opaque payload".to_vec(),
        )
        .unwrap(),
    );
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        original.compatibility().clone(),
        original.cone().clone(),
        original.direct_dependencies().to_vec(),
        &members,
        original.semantic_fingerprints(),
        original.sections().to_vec(),
    )
    .unwrap();
    CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .as_bytes()
        .to_vec()
}
