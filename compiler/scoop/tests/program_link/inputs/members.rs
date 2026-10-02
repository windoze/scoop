use super::*;
use scoop_identity::CapabilityId;
use scoop_slib::{
    ExtensionRequirement, LogicalMemberKey, ManifestSection, MemberPurposeSet, MemberStableKey,
    SlibMember, SlibMemberRole,
};

#[test]
fn optional_machine_bytes_and_compile_only_payloads_remain_opaque() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "root",
        "executable",
        &fixture("read-println.scoop"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let output = directory.path().join("program");
    let original_plan = environment.link(&root, &[], &output);
    let (manifest, mut members) = archive::read(&root);
    let object = members
        .iter()
        .find(|member| matches!(member.record().role(), SlibMemberRole::LinkObject { .. }))
        .unwrap()
        .payload()
        .to_vec();
    assert!(object::File::parse(object.as_slice()).is_ok());
    let cone = manifest.cone().identity();
    let optional = CapabilityId::new("dev.programlink", "opaque-object", 1).unwrap();
    members.push(
        SlibMember::new(
            cone,
            MemberStableKey::ExtensionBlob {
                capability: optional.clone(),
                logical_key: LogicalMemberKey::new(b"object".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability: optional.clone(),
                requirement: ExtensionRequirement::Optional,
            },
            object.clone(),
        )
        .unwrap(),
    );
    members.push(
        SlibMember::new(
            cone,
            MemberStableKey::DiagnosticAttachment {
                capability: optional.clone(),
                logical_key: LogicalMemberKey::new(b"diagnostic".to_vec()).unwrap(),
            },
            SlibMemberRole::DiagnosticAttachment {
                capability: optional,
            },
            object.clone(),
        )
        .unwrap(),
    );
    let mut sections = manifest.sections().to_vec();
    sections.push(
        ManifestSection::new(
            CapabilityId::new("dev.programlink", "compile-only", 1).unwrap(),
            MemberPurposeSet::COMPILE,
            vec![0xff],
        )
        .unwrap(),
    );
    let repacked = directory.path().join("repacked.slib");
    archive::write(&repacked, &manifest, members.clone(), sections.clone());
    assert_ne!(
        std::fs::read(&root).unwrap(),
        std::fs::read(&repacked).unwrap()
    );
    assert_eq!(environment.link(&repacked, &[], &output), original_plan);
    assert_eq!(run(&output, false), "42\n");
    assert_eq!(run(&output, true), "42\n");

    let required = CapabilityId::new("dev.programlink", "unknown-link-object", 1).unwrap();
    let member = SlibMember::new(
        cone,
        MemberStableKey::ExtensionBlob {
            capability: required.clone(),
            logical_key: LogicalMemberKey::new(b"object".to_vec()).unwrap(),
        },
        SlibMemberRole::ExtensionBlob {
            capability: required,
            requirement: ExtensionRequirement::Link,
        },
        object,
    )
    .unwrap();
    let id = member.record().id().to_string();
    members.push(member);
    archive::write(&repacked, &manifest, members, sections);
    let error = assert_error(environment, &repacked, &[], &output, "unknown-link-object");
    assert!(error.contains(&id), "{error}");
}
