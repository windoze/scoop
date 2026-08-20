use scoop_identity::{CapabilityId, ConeIdentity, ObjectFormatId, TargetProfileWireId};

use super::*;

fn capability(name: &str) -> CapabilityId {
    CapabilityId::new("org.scoop-lang.test", name, 1).unwrap()
}

fn logical_key(bytes: &[u8]) -> LogicalMemberKey {
    LogicalMemberKey::new(bytes.to_vec()).unwrap()
}

#[test]
fn logical_keys_require_nonempty_bytes() {
    assert_eq!(
        LogicalMemberKey::new(Vec::new()),
        Err(LogicalMemberKeyError::Empty)
    );
    let key = vec![0; 8_192];
    assert_eq!(LogicalMemberKey::new(key.clone()).unwrap().as_bytes(), key);
}

#[test]
fn key_role_matrix_rejects_variant_and_capability_mismatches() {
    let diagnostic = capability("diagnostic");
    let other = capability("other");
    let key = MemberStableKey::DiagnosticAttachment {
        capability: diagnostic.clone(),
        logical_key: logical_key(b"json"),
    };

    assert_eq!(
        SlibMemberRecord::new(
            ConeIdentity::CORE,
            key.clone(),
            SlibMemberRole::HirMetadata,
            b"payload",
        ),
        Err(SlibMemberRecordError::KeyRoleMismatch)
    );
    assert_eq!(
        SlibMemberRecord::new(
            ConeIdentity::CORE,
            key,
            SlibMemberRole::DiagnosticAttachment { capability: other },
            b"payload",
        ),
        Err(SlibMemberRecordError::CapabilityMismatch)
    );
}

#[test]
fn metadata_record_has_fixed_wire_id_and_fingerprint() {
    let record = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        b"hir",
    )
    .unwrap();

    assert_eq!(
        hex(&scoop_wire::encode(&record).unwrap()),
        "a5015820d2be1585afd5800d1d66522762f35b9d89eaa15cc43dcf9e0e299503e6d292b802a1000103a20001010104030558205e308bbe57debf9cc46cf2588f41a55fd820d26f1bf98c7a8c0dcf17b8e9262f"
    );
    assert_eq!(
        record.id().to_string(),
        "d2be1585afd5800d1d66522762f35b9d89eaa15cc43dcf9e0e299503e6d292b8"
    );
    assert_eq!(
        record.fingerprint().unwrap().to_string(),
        "ff5ae48ebf8612348229d20cdca9084c45d477daaff04cc5b23ca2e9ac83b3f3"
    );
    assert_eq!(record.byte_length(), 3);
    assert_eq!(record.sha256(), scoop_wire::sha256(b"hir"));
    assert!(record.as_link_member().is_none());
}

#[test]
fn only_link_inputs_can_form_link_fingerprints() {
    let verifier = capability("object");
    let link = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::LinkObject {
            verifier_capability: verifier.clone(),
            logical_key: logical_key(b"unit"),
        },
        SlibMemberRole::LinkObject {
            target_profile: TargetProfileWireId::darwin_aarch64(),
            object_format: ObjectFormatId::macho_relocatable(),
            verifier_capability: verifier,
        },
        b"object",
    )
    .unwrap();
    let optional = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::ExtensionBlob {
            capability: capability("optional"),
            logical_key: logical_key(b"blob"),
        },
        SlibMemberRole::ExtensionBlob {
            capability: capability("optional"),
            requirement: ExtensionRequirement::Optional,
        },
        b"optional",
    )
    .unwrap();
    let required = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::ExtensionBlob {
            capability: capability("required"),
            logical_key: logical_key(b"library"),
        },
        SlibMemberRole::ExtensionBlob {
            capability: capability("required"),
            requirement: ExtensionRequirement::Link,
        },
        b"required",
    )
    .unwrap();

    assert_eq!(
        link.as_link_member()
            .unwrap()
            .fingerprint()
            .unwrap()
            .to_string(),
        "e5d1dcce52fcffc04dfd840fcef111454a7820509c5f04b8ed3f52ba3b31a502"
    );
    assert_eq!(
        required
            .as_link_member()
            .unwrap()
            .fingerprint()
            .unwrap()
            .to_string(),
        "427bde8532ebf7f4aec5abae2dbce525e394655342b78d52686de016c5850de0"
    );
    assert!(optional.as_link_member().is_none());
    assert!(required.as_link_member().is_some());
    assert_eq!(link.role().purpose_set(), MemberPurposeSet::LINK);
}

#[test]
fn metadata_roles_keep_the_initial_wire_schema_field() {
    assert_eq!(
        hex(&scoop_wire::encode(&SlibMemberRole::HirMetadata).unwrap()),
        "a200010101"
    );
    assert_eq!(
        hex(&scoop_wire::encode(&SlibMemberRole::MirMetadata).unwrap()),
        "a200020101"
    );
    assert_eq!(
        hex(&scoop_wire::encode(&SlibMemberRole::LirMetadata).unwrap()),
        "a200030101"
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
