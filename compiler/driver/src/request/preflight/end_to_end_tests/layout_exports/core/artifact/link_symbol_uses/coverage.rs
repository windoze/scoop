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

pub(super) fn inspect(proof: &slib::ReplayedLayoutLinkSymbolUsesV1) {
    let objects = proof.object_contents();
    let mut members = objects
        .objects()
        .objects()
        .iter()
        .map(|object| object.member())
        .chain(objects.generated_objects().map(|object| object.member()))
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
}

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
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
    }
    rejection::check(
        core,
        artifact,
        profile,
        Case::Members(Surface::Identity, Change::Fingerprint),
        true,
    );
}
