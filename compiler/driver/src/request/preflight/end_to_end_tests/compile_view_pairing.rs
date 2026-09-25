use super::*;
use scoop_slib::{PublishViewMismatchError, StrongLinkArtifactValidationError};

#[test]
fn link_shape_validation_requires_the_same_artifact_compile_view() {
    let target = resolved_target().expect("Compile/Link pairing requires the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let root = sysroot.path().join("library");
    let mut archives = Vec::new();
    for (index, source) in [
        "public struct Shape()",
        "public struct Shape(val number: Int)",
    ]
    .into_iter()
    .enumerate()
    {
        write_manifest_cone(&root, "dev.example", "shape-pairing", "library", source);
        let library = build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/shape-{index}.slib")),
        );
        archives.push(std::fs::read(library.artifact().path()).unwrap());
    }
    let identity = ConeCoordinate::new("dev.example", "shape-pairing", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let mut session = scoop_identity::SemanticIdentitySession::new();
    let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
        identity,
        target.lir_target_selection(),
        vec![ConeIdentity::CORE],
        vec![&core_bytes],
        &archives[0],
        target.c_bridge_toolchain().profile(),
        &mut session,
    )
    .unwrap();
    for (bytes, expected) in [
        (&core_bytes, PublishViewMismatchError::Cone),
        (&archives[1], PublishViewMismatchError::ArtifactFingerprint),
    ] {
        let graph = DecodedSlibEnvelope::open(bytes, target.lir_target_selection())
            .unwrap()
            .validate_graph()
            .unwrap();
        assert!(matches!(
            scoop_slib::validate_self_describing_cross_cone_strong_link_artifact(
                graph,
                closure.current_compile(),
                &[],
                target.c_bridge_toolchain().profile(),
            ),
            Err(StrongLinkArtifactValidationError::CompileView(actual)) if actual == expected
        ));
    }
}
