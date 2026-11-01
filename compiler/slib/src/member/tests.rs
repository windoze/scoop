use scoop_identity::{CapabilityId, ConeIdentity};

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
        SlibMember::new(
            ConeIdentity::CORE,
            key.clone(),
            SlibMemberRole::HirMetadata,
            b"payload".to_vec(),
        ),
        Err(SlibMemberRecordError::KeyRoleMismatch)
    );
    assert_eq!(
        SlibMember::new(
            ConeIdentity::CORE,
            key,
            SlibMemberRole::DiagnosticAttachment { capability: other },
            b"payload".to_vec(),
        ),
        Err(SlibMemberRecordError::CapabilityMismatch)
    );
}

#[test]
fn metadata_record_has_fixed_wire_id_and_fingerprint() {
    let record = SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        b"hir".to_vec(),
    )
    .unwrap()
    .record()
    .clone();

    assert_eq!(
        hex(&scoop_wire::encode(&record).unwrap()),
        "a4015820d2be1585afd5800d1d66522762f35b9d89eaa15cc43dcf9e0e299503e6d292b803a200010101040c0558205febd6c21b3c6bd69dfe95d1ccc3642ee63e7f9faebb0bde10f5b37af1f199f6"
    );
    assert_eq!(
        record.id().to_string(),
        "d2be1585afd5800d1d66522762f35b9d89eaa15cc43dcf9e0e299503e6d292b8"
    );
    assert_eq!(
        record.fingerprint().unwrap().to_string(),
        "81e09433e25f702d5c2b32f37ee54d27d215e6f7a4cb971ddee7bd12af245c12"
    );
    assert_eq!(record.byte_length(), 12);
    assert_eq!(
        hex(record.sha256().as_array()),
        "5febd6c21b3c6bd69dfe95d1ccc3642ee63e7f9faebb0bde10f5b37af1f199f6"
    );
    assert!(record.as_link_member().is_none());
}

#[test]
fn only_link_inputs_can_form_link_fingerprints() {
    let verifier = capability("object");
    let link = SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::LinkObject {
            verifier_capability: verifier.clone(),
            logical_key: logical_key(b"unit"),
        },
        SlibMemberRole::LinkObject,
        b"object".to_vec(),
    )
    .unwrap()
    .record()
    .clone();
    let optional = SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::ExtensionBlob {
            capability: capability("optional"),
            logical_key: logical_key(b"blob"),
        },
        SlibMemberRole::ExtensionBlob {
            capability: capability("optional"),
            requirement: ExtensionRequirement::Optional,
        },
        b"optional".to_vec(),
    )
    .unwrap()
    .record()
    .clone();
    let required = SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::ExtensionBlob {
            capability: capability("required"),
            logical_key: logical_key(b"library"),
        },
        SlibMemberRole::ExtensionBlob {
            capability: capability("required"),
            requirement: ExtensionRequirement::Link,
        },
        b"required".to_vec(),
    )
    .unwrap()
    .record()
    .clone();

    assert_eq!(
        link.as_link_member()
            .unwrap()
            .fingerprint()
            .unwrap()
            .to_string(),
        "d29223da4529c43eaf9a59d39d5bb361e91686122ccf3cad3c6085da63cc2669"
    );
    assert_eq!(
        required
            .as_link_member()
            .unwrap()
            .fingerprint()
            .unwrap()
            .to_string(),
        "f509a5fcabb56118b17ba92c28fa6acd256a7b1644387fa2cacce8d92dc0a0cd"
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
