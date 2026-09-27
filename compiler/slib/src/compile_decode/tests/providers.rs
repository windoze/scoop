use crate::strong_compile_decode::tests::{
    build_artifact_for_profile_with_dependencies, open_graph,
};
use crate::{
    ArtifactCapabilityProfile, ConeKind, ConeRecord, ConeSourceForm, DependencyRecord,
    HirFingerprint, LirFingerprint, MirFingerprint,
};
use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, DeclarationScope,
    DefinitionOwnerChain, IdentityValidationError, PackagePath, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_lir::{CanonicalLirFoundation, DecodedLirFoundation};
use scoop_mir::{CanonicalMirFoundation, DecodedMirFoundation};
use scoop_wire::{decode_canonical, encode};

#[test]
fn foreign_declarations_require_their_actual_cone_dependency() {
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
            let bytes = build_artifact_for_profile_with_dependencies(
                ConeRecord::new(
                    ConeCoordinate::reserved_single_file(),
                    ConeKind::Executable,
                    ConeSourceForm::SingleFile,
                )
                .unwrap(),
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
                dependencies,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                false,
            );
            let result = crate::compile_decode::validate_foundation_identity_graph(
                &mut open_graph(&bytes),
                &decode_canonical::<DecodedHirFoundation>(&encode(&hir).unwrap()).unwrap(),
                &decode_canonical::<DecodedMirFoundation>(&encode(&mir).unwrap()).unwrap(),
                &decode_canonical::<DecodedLirFoundation>(&encode(&lir).unwrap()).unwrap(),
            );
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
