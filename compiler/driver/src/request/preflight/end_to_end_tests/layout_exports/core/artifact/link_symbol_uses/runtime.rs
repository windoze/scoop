//! Finalized runtime bytes and metadata are independently rebuilt on read.

use super::super::link_archive::{self, Rewrite};
use super::*;
use object::{Object as _, ObjectSection as _};
use std::collections::BTreeMap;

mod rejection;

#[derive(Clone, Copy, Debug)]
pub(super) enum RuntimeMutation {
    Field(u32),
    Body(slib::SlibMemberId, u64),
    Patch(
        scoop_identity::DigestSemanticFieldRole,
        slib::SlibMemberId,
        u64,
    ),
}

pub(super) fn inspect(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    proof: &slib::ReplayedLayoutLinkSymbolUsesV1<'_>,
) -> String {
    let payloads = link_archive::payloads(artifact);
    let finalized = proof.final_objects();
    for object in finalized.objects() {
        assert_eq!(object.bytes(), payloads[&object.member()]);
    }
    let registrations = finalized.runtime_images().fingerprint().registrations();
    assert_eq!(registrations.producer(), proof.provider());
    let counts = [
        registrations.safepoints().fingerprints().len(),
        registrations.callables().fingerprints().len(),
        registrations.types().fingerprints().len(),
        registrations.immortal_objects().fingerprints().len(),
        registrations.static_storages().fingerprints().len(),
        registrations.initializations().fingerprints().len(),
    ];
    format!(
        "objects={} registrations={counts:?}\n",
        finalized.objects().len()
    )
}

pub(super) fn cases(proof: &slib::ReplayedLayoutLinkSymbolUsesV1<'_>) -> Vec<RuntimeMutation> {
    let mut by_role = BTreeMap::new();
    for patch in proof.object_contents().patch_sites().sites() {
        by_role.entry(patch.semantic_field_role()).or_insert(*patch);
    }
    let mut cases = (3..=6)
        .map(RuntimeMutation::Field)
        .chain(by_role.into_iter().map(|(role, patch)| {
            RuntimeMutation::Patch(role, patch.member(), patch.checked_offset())
        }))
        .collect::<Vec<_>>();
    cases.push(body_mutation(proof));
    cases
}

fn body_mutation(proof: &slib::ReplayedLayoutLinkSymbolUsesV1<'_>) -> RuntimeMutation {
    for candidate in proof.object_contents().objects().objects() {
        let file = object::File::parse(candidate.final_bytes()).unwrap();
        for section in file
            .sections()
            .filter(|section| section.kind() == object::SectionKind::Text)
        {
            // A RET has no relocation bits. Changing the instruction must change
            // the actual callable-body and runtime-image fingerprints.
            if let Some(index) = section
                .data()
                .unwrap()
                .chunks_exact(4)
                .position(|instruction| instruction == [0xc0, 0x03, 0x5f, 0xd6])
            {
                return RuntimeMutation::Body(
                    candidate.member(),
                    section.file_range().unwrap().0 + (index as u64) * 4,
                );
            }
        }
    }
    panic!("native callable fixture contains an AArch64 return");
}

pub(super) fn check(
    symbols_path: &Path,
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    cases: Vec<RuntimeMutation>,
    mut dump: String,
) {
    for mutation in cases {
        rejection::check(core, artifact, profile, mutation);
        match mutation {
            RuntimeMutation::Field(field) => dump.push_str(&format!("reject field {field}\n")),
            RuntimeMutation::Patch(role, _, _) => {
                dump.push_str(&format!("reject {role:?} patch\n"))
            }
            RuntimeMutation::Body(_, _) => {
                dump.push_str("reject changed body runtime fingerprint\n")
            }
        }
    }
    let name = symbols_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_suffix(".symbols.snap")
        .unwrap();
    let path = symbols_path.with_file_name(format!("{name}.runtime.snap"));
    if std::env::var_os("SCOOP_UPDATE_LINK_RUNTIME").is_some() {
        std::fs::write(&path, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(path).unwrap());
}
