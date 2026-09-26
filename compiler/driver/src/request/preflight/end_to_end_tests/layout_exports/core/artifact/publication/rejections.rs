use super::*;
use slib::CrossConeLayoutArtifactValidationError as Error;

pub(super) fn check(
    destination: &Path,
    public: &hir::CrossConeHirInterfaceSectionV1,
    provider: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    dump: &mut String,
) {
    let current = reader::open(artifact).identity();
    let direct = [reader::open(provider).identity()];
    let dependencies = [provider.as_bytes()];
    let reject = |bytes: &[u8], providers: &[&[u8]]| {
        let result = slib::read_cross_cone_layout_artifact_summary(
            bytes,
            current,
            &direct,
            providers,
            artifact.target_selection(),
            profile,
        );
        result.unwrap_err()
    };
    let (bytes, expected) = source_calls::publication_mutation(public, artifact);
    let Error::Semantic { source } = reject(&bytes, &dependencies) else {
        panic!("expected source signature rejection")
    };
    let slib::CrossConeLayoutSemanticClosureError::HirDeclarations(error) = *source else {
        panic!("expected source call validation: {source:?}")
    };
    assert_eq!(error.provider, current);
    let slib::CrossConeHirDeclarationValidationError::References(
        slib::CrossConeHirReferenceSurfaceError::CallSites(error),
    ) = *error.source
    else {
        panic!("expected actual call-site relation")
    };
    assert!(
        matches!(*error, slib::CrossConeHirCallSiteOriginError::Signature { position, .. }
        if position == expected)
    );
    dump.push_str("reader SourceSignature\n");

    let Error::Semantic { source } = reject(artifact.as_bytes(), &[]) else {
        panic!("expected missing dependency rejection")
    };
    assert!(
        matches!(*source, slib::CrossConeLayoutSemanticClosureError::Graph(error)
        if matches!(*error, slib::CrossConeClosureGraphError::MissingDirectArtifact { identity }
            if identity == direct[0]))
    );
    dump.push_str("reader MissingDependency\n");

    for (owner, source_artifact) in [(current, artifact), (direct[0], provider)] {
        let bytes = corrupt_code(source_artifact);
        let error = if owner == current {
            reject(&bytes, &dependencies)
        } else {
            reject(artifact.as_bytes(), &[&bytes])
        };
        let Error::Physical { source } = error else {
            panic!("expected final Link Code projection rejection: {error:?}")
        };
        assert_eq!(source.provider, owner);
        assert!(
            matches!(*source.source, slib::SharedLirPhysicalError::LinkSymbolUses(error)
            if matches!(*error, slib::LayoutLinkSymbolUseError::CodeProjection(
                slib::CodeProductionProjectionError::FieldMismatch { field: 7 })))
        );
        dump.push_str(&format!(
            "reader {}Code\n",
            if owner == current {
                "Current"
            } else {
                "Dependency"
            }
        ));
    }
    let mut removed = false;
    let bytes = link_archive::rewrite(artifact, |record, _| {
        if !removed && matches!(record.role(), slib::SlibMemberRole::LinkObject { .. }) {
            removed = true;
            link_archive::Rewrite::Remove
        } else {
            link_archive::Rewrite::Keep
        }
    });
    let Error::Physical { source } = reject(&bytes, &dependencies) else {
        panic!("expected actual Link object rejection")
    };
    assert_eq!(source.provider, current);
    dump.push_str("reader MissingObject\n");

    let blocked = destination.parent().unwrap().join("blocked.slib");
    std::fs::create_dir(&blocked).unwrap();
    let result = artifact.publish(&blocked);
    assert!(matches!(
        result,
        Err(slib::CrossConeArtifactPublishError::Io {
            operation: slib::CrossConePublishIoOperation::RenameTemporary,
            ..
        })
    ));
    assert!(std::fs::read_dir(&blocked).unwrap().next().is_none());
    std::fs::remove_dir(&blocked).unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), artifact.as_bytes());
    assert_no_temporary(destination.parent().unwrap(), destination);
    dump.push_str("reject RenameDirectory; destination-preserved=true\n");
}

fn corrupt_code(artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1) -> Vec<u8> {
    let slib::FingerprintAvailability::Available(code) =
        reader::open(artifact).semantic_fingerprints().code()
    else {
        panic!("real layout artifact contains Code")
    };
    link_archive::rewrite_production(artifact, |bytes| {
        let positions = bytes
            .windows(32)
            .enumerate()
            .filter_map(|(index, value)| (value == code.as_array()).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        let mut changed = bytes.to_vec();
        changed[positions[0] + 31] ^= 1;
        changed
    })
}
