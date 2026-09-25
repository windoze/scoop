//! Every saved object set and coverage digest binds the actual final members.

use super::super::link_archive::{self, Rewrite};
use super::*;

mod mutation;
mod rejection;

#[derive(Clone, Copy, Debug)]
enum Surface {
    Identity,
    Ordinary,
    Shape,
}

#[derive(Clone, Copy, Debug)]
enum Change {
    Missing,
    Extra,
    Duplicate,
    Order,
    Fingerprint,
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Members(Surface, Change),
    Digest(Surface),
    ImageField(u64),
    EntryBranch,
}

pub(super) fn inspect(proof: &slib::ReplayedLayoutLinkSymbolUsesV1<'_>) -> String {
    let objects = proof.object_contents();
    let mut members = objects
        .objects()
        .objects()
        .iter()
        .map(|object| object.member())
        .chain(
            objects
                .generated_objects()
                .iter()
                .map(|object| object.member()),
        )
        .collect::<Vec<_>>();
    members.sort_unstable();
    assert_eq!(
        members,
        proof
            .link_objects()
            .members()
            .iter()
            .map(|member| member.member())
            .collect::<Vec<_>>()
    );
    format!(
        "scoop={} bridges={} final_members={}\n",
        objects.objects().objects().len(),
        objects.generated_objects().len(),
        members.len(),
    )
}

pub(super) fn check(
    symbols_path: &Path,
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    mut dump: String,
) {
    let cases = [Surface::Identity, Surface::Ordinary, Surface::Shape]
        .into_iter()
        .flat_map(|surface| {
            [
                Change::Missing,
                Change::Extra,
                Change::Duplicate,
                Change::Order,
                Change::Fingerprint,
            ]
            .map(|change| Case::Members(surface, change))
        })
        .chain([
            Case::Digest(Surface::Ordinary),
            Case::Digest(Surface::Shape),
        ])
        .chain((1..=6).map(Case::ImageField))
        .chain([Case::EntryBranch]);
    for case in cases {
        rejection::check(core, artifact, profile, case, false);
        dump.push_str(&format!("reject {case:?}\n"));
    }
    rejection::check(
        core,
        artifact,
        profile,
        Case::Members(Surface::Identity, Change::Fingerprint),
        true,
    );
    dump.push_str("reject dependency final fingerprint\n");
    let name = symbols_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_suffix(".symbols.snap")
        .unwrap();
    let path = symbols_path.with_file_name(format!("{name}.coverage.snap"));
    if std::env::var_os("SCOOP_UPDATE_LINK_COVERAGE").is_some() {
        std::fs::write(&path, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(path).unwrap());
}
