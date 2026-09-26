use super::*;
use scoop_identity::{PersistentTypeId, SourceNominalKind};

#[test]
fn foreign_declarations_require_their_actual_cone_dependency() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    for provider in [
        ConeCoordinate::reserved_core(),
        ConeCoordinate::new("tests", "provider", "1.0.0").unwrap(),
    ] {
        let source =
            CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
                SourceDeclarationSite::new(
                    provider.identity().unwrap(),
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new("Value").unwrap(),
                SourceNominalKind::Struct,
                0,
            ))
            .unwrap();
        let mut hir = CanonicalHirFoundation::empty();
        hir.set_types(vec![source.clone()]).unwrap();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        for include_dependency in [false, true] {
            let dependencies = include_dependency
                .then(|| {
                    DependencyRecord::new(
                        provider.clone(),
                        HirFingerprint::from_array([1; 32]),
                        MirFingerprint::from_array([2; 32]),
                        LirFingerprint::from_array([3; 32]),
                    )
                    .unwrap()
                })
                .into_iter()
                .collect();
            let artifact = IdentityFoundationArtifact::write(
                IdentityFoundationArtifactInput::new(
                    ProducerRecord::new("test").unwrap(),
                    ConeRecord::new(
                        ConeCoordinate::reserved_single_file(),
                        ConeKind::Executable,
                        ConeSourceForm::SingleFile,
                    )
                    .unwrap(),
                    selection,
                    &hir,
                    &mir,
                    &lir,
                )
                .with_direct_dependencies(dependencies),
            )
            .unwrap();
            let result = crate::DecodedSlibEnvelope::open(artifact.as_bytes(), selection)
                .unwrap()
                .validate_graph()
                .unwrap()
                .decode_identity_foundations()
                .unwrap()
                .validate_identities();
            if include_dependency {
                result.expect("the declared provider resolves the foreign declaration");
            } else {
                assert!(matches!(result,
                    Err(IdentityValidationError::InvalidRecord { id, .. })
                        if id == *source.id().as_array()
                ));
            }
        }
    }
}
