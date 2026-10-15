//! Source-produced archives expose their saved publication metadata.

use super::lir_dependencies::reader;
use super::*;
use scoop_slib as slib;

mod rejections;

pub(super) fn check_provider(provider: &slib::AssembledCrossConeLayoutArtifactV1) {
    let input = reader::open(provider);
    let publication = slib::read_cross_cone_layout_artifact_summary(
        provider.as_bytes(),
        input.identity(),
        &[],
        &[],
        provider.target_selection(),
    )
    .unwrap();
    verify(&publication, provider);
    assert!(publication.direct_dependencies().is_empty());
}

pub(super) fn check(
    name: &str,
    public: &hir::CrossConeHirInterfaceSectionV1,
    provider: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
) {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("published.slib");
    let direct = [reader::open(provider).identity()];
    let current = reader::open(artifact).identity();
    let dependencies = [provider.as_bytes()];
    if name == "combined" {
        std::fs::write(&destination, b"previous validated publication").unwrap();
    }
    let published = artifact.publish(&destination).unwrap();
    assert_eq!(published.path(), destination);
    assert_eq!(std::fs::read(&destination).unwrap(), artifact.as_bytes());
    verify(published.summary(), artifact);
    assert_eq!(published.summary().direct_dependencies().len(), 1);
    assert_eq!(
        published.summary().direct_dependencies()[0],
        reader::open(provider).dependency_record()
    );
    let bytes = std::fs::read(&destination).unwrap();
    let reread = slib::read_cross_cone_layout_artifact_summary(
        &bytes,
        current,
        &direct,
        &dependencies,
        artifact.target_selection(),
    )
    .unwrap();
    assert_eq!(&reread, published.summary());
    assert_no_temporary(root.path(), &destination);

    rejections::check(&destination, public, provider, artifact);
}

fn verify(
    publication: &slib::CrossConeArtifactSummary,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
) {
    let source = reader::open(artifact);
    assert_eq!(
        publication.artifact_fingerprint(),
        source.artifact_fingerprint()
    );
    assert_eq!(publication.coordinate(), source.coordinate());
    assert_eq!(publication.identity(), source.identity());
    assert_eq!(publication.kind(), source.kind());
    assert_eq!(publication.source_form(), source.source_form());
    assert_eq!(publication.target_selection(), artifact.target_selection());
    assert_eq!(
        *publication.profile(),
        scoop_identity::ArtifactCapabilityProfileId::cross_cone_generic()
    );
    assert_eq!(publication.dependency_record(), source.dependency_record());
    assert_eq!(
        publication.manifest_summary().semantic_fingerprints(),
        source.semantic_fingerprints()
    );
}

fn assert_no_temporary(root: &Path, destination: &Path) {
    let entries = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(entries, [destination]);
}
