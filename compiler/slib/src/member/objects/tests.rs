use super::*;
use crate::{CanonicalSlibArchive, ManifestArchive, SlibMemberRecord};

fn object(index: usize, payload: Vec<u8>) -> UncompressedObjectMember {
    UncompressedObjectMember::new(
        ConeIdentity::CORE,
        MemberStableKey::LinkObject {
            verifier_capability: crate::scoop_lir_link_object_capability(),
            logical_key: LogicalMemberKey::new(index.to_le_bytes().to_vec()).unwrap(),
        },
        SlibMemberRole::LinkObject,
        payload,
    )
    .unwrap()
}

fn samples(family: &str) -> Vec<UncompressedObjectMember> {
    (0..96)
        .map(|index| {
            let payload = (0..80)
                .map(|row| format!("{family}/module_{index}/function_{row}: native runtime descriptor shared symbol relocation {row:08}\n"))
                .collect::<String>()
                .into_bytes();
            object(index, payload)
        })
        .collect()
}

fn directory(members: &[SlibMember]) -> Vec<SlibMemberRecord> {
    let mut records = members
        .iter()
        .map(|member| member.record().clone())
        .collect::<Vec<_>>();
    records.sort_unstable_by_key(SlibMemberRecord::id);
    records
}

#[test]
fn trained_batch_is_deterministic_and_preserves_every_object() {
    let mut objects = samples("first");
    // Cover attached dictionaries, copied match tables, and large inputs that
    // reload the dictionary with different compression parameters.
    let large = objects
        .iter()
        .flat_map(|object| object.payload.iter().copied())
        .collect::<Vec<_>>();
    objects.push(object(96, large[..192 * 1024].to_vec()));
    objects.push(object(97, large));
    let expected = objects
        .iter()
        .map(|object| (object.content.id(), object.payload.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let forward = encode_object_members(ConeIdentity::CORE, objects.clone()).unwrap();
    let mut reversed = objects;
    reversed.reverse();
    let reverse = encode_object_members(ConeIdentity::CORE, reversed).unwrap();
    assert_eq!(forward, reverse);
    let dictionary = forward
        .iter()
        .find(|member| member.record().role() == &SlibMemberRole::ObjectDictionary)
        .unwrap();
    assert!(dictionary.payload().len() <= DICTIONARY_LENGTH);
    assert_eq!(dictionary.record().sha256(), sha256(dictionary.payload()));
    assert!(dictionary.record().as_link_member().is_none());
    let records = directory(&forward);
    let archive = CanonicalSlibArchive::write(b"manifest", forward).unwrap();
    let decoded = ManifestArchive::open(archive.as_bytes())
        .unwrap()
        .validate_directory(&records)
        .unwrap();
    for (id, original) in expected {
        let bytes = decoded.member(id).unwrap().unwrap();
        assert_eq!(bytes, original);
        assert!(std::ptr::eq(bytes, decoded.member(id).unwrap().unwrap()));
        assert_eq!(
            records
                .iter()
                .find(|record| record.id() == id)
                .unwrap()
                .sha256(),
            sha256(&original)
        );
    }
}

#[test]
fn different_dictionaries_keep_native_content_and_code_projection_identical() {
    let first = train_dictionary(&samples("first"));
    let second = train_dictionary(&samples("second"));
    assert!(!first.is_empty() && !second.is_empty());
    assert_ne!(first, second);
    let raw = samples("first").remove(0);
    let left = raw
        .clone()
        .encode(&mut zstd::bulk::Compressor::with_dictionary(3, &first).unwrap())
        .unwrap();
    let right = raw
        .encode(&mut zstd::bulk::Compressor::with_dictionary(3, &second).unwrap())
        .unwrap();
    assert_ne!(left.payload(), right.payload());
    assert_eq!(left.record().content(), right.record().content());
    assert_eq!(
        left.record()
            .as_link_member()
            .unwrap()
            .fingerprint()
            .unwrap(),
        right
            .record()
            .as_link_member()
            .unwrap()
            .fingerprint()
            .unwrap()
    );
    let objects =
        crate::link_decode::tests::layout_link_support::verified_layout_code_link_object_members();
    let final_objects = objects.final_objects().clone();
    let left_code = crate::build_code_link_object_members_v2(
        final_objects.clone(),
        &[left.record().content().clone()],
    )
    .unwrap();
    let right_code = crate::build_code_link_object_members_v2(
        final_objects,
        &[right.record().content().clone()],
    )
    .unwrap();
    assert_eq!(left_code.projection(), right_code.projection());
    assert!(crate::compression::decompress(left.payload()).is_err());
    assert!(
        crate::compression::ObjectDecompressor::new(&second)
            .unwrap()
            .decompress(left.payload())
            .is_err()
    );
}

#[test]
fn small_batches_use_plain_frames_and_invalid_frames_report_errors() {
    let payload = b"small object".to_vec();
    let members =
        encode_object_members(ConeIdentity::CORE, vec![object(0, payload.clone())]).unwrap();
    assert_eq!(members.len(), 1);
    let stored = members[0].payload();
    assert_eq!(crate::compression::decompress(stored).unwrap(), payload);
    let decoder = crate::compression::ObjectDecompressor::new(&[]).unwrap();
    assert!(decoder.decompress(&stored[..stored.len() - 1]).is_err());
    let mut trailing = stored.to_vec();
    trailing.extend_from_slice(stored);
    assert!(decoder.decompress(&trailing).is_err());
    assert!(decoder.decompress(b"invalid frame").is_err());
}

#[test]
fn file_reader_loads_dictionary_on_object_use_and_shares_decoded_buffers() {
    let (original, mut members) = crate::link_decode::tests::archive::complete_artifact_parts();
    let raw = samples("file");
    let expected = raw[0].payload.clone();
    let id = raw[0].content.id();
    members.extend(encode_object_members(ConeIdentity::CORE, raw).unwrap());
    let manifest = crate::BootstrapManifest::new(
        crate::ProducerRecord::new("test").unwrap(),
        original.compatibility().clone(),
        original.cone().clone(),
        original.direct_dependencies().to_vec(),
        &members,
        original.semantic_fingerprints(),
        original.sections().to_vec(),
    )
    .unwrap();
    let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members.clone()).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("dictionary.slib");
    std::fs::write(&path, archive.as_bytes()).unwrap();
    let file =
        crate::ArtifactFile::open(&path, scoop_target_data::DARWIN_AARCH64_LLVM_22_1).unwrap();
    let saved = file.directory().unwrap();
    std::thread::scope(|scope| {
        let jobs = (0..4)
            .map(|_| scope.spawn(|| file.member(saved, id).unwrap().unwrap()))
            .collect::<Vec<_>>();
        let buffers = jobs
            .into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(buffers[0], expected);
        assert!(
            buffers
                .windows(2)
                .all(|pair| std::ptr::eq(pair[0], pair[1]))
        );
    });

    let dictionary = members
        .iter_mut()
        .find(|member| member.record().role() == &SlibMemberRole::ObjectDictionary)
        .unwrap();
    dictionary.payload.fill(0);
    let malformed = CanonicalSlibArchive::write_bootstrap(&manifest, members.clone()).unwrap();
    let bad_path = temporary.path().join("invalid-dictionary.slib");
    std::fs::write(&bad_path, malformed.as_bytes()).unwrap();
    let bad =
        crate::ArtifactFile::open(&bad_path, scoop_target_data::DARWIN_AARCH64_LLVM_22_1).unwrap();
    let saved = bad.directory().unwrap();
    let metadata = saved
        .manifest
        .members()
        .iter()
        .find(|record| record.role() == &SlibMemberRole::HirMetadata)
        .unwrap()
        .id();
    assert!(bad.member(saved, metadata).unwrap().is_some());
    assert!(bad.member(saved, id).is_err());

    members.retain(|member| member.record().role() != &SlibMemberRole::ObjectDictionary);
    let records = directory(&members);
    let missing = CanonicalSlibArchive::write(b"manifest", members).unwrap();
    let decoded = ManifestArchive::open(missing.as_bytes())
        .unwrap()
        .validate_directory(&records)
        .unwrap();
    assert!(decoded.member(metadata).unwrap().is_some());
    assert!(decoded.member(id).is_err());
}
