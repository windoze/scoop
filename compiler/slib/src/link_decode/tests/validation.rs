use super::*;

#[test]
fn strong_graph_rejects_final_object_bytes_that_do_not_reconstruct() {
    let bytes = complete_artifact(true);

    let dependency_owners = Vec::new();
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations()
            .unwrap()
            .validate_c_bridge_envelopes(&c_bridge_profile())
            .unwrap()
            .validate_builtin_objects()
            .unwrap()
            .validate_digest_patch_sites()
            .unwrap()
            .validate_registration_objects()
            .unwrap()
            .fingerprint_registration_leaves()
            .unwrap()
            .validate_link_symbol_requirements(&dependency_owners, &c_bridge_profile())
            .unwrap()
            .fingerprint_registration_dependencies()
            .unwrap()
            .finalize_strong_objects(),
        Err(StrongLinkObjectFinalizationError::FinalObjectMismatch(
            ReconstructedScoopObjectError::ByteMismatch(_)
        ))
    ));
}

#[test]
fn link_materialization_requires_the_complete_planned_object_directory() {
    let bytes = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        true,
        true,
        false,
        false,
    );

    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations(),
        Err(StrongLinkMaterializationError::MissingObjectMember(_))
    ));
}

#[test]
fn link_digest_patch_rejects_a_stale_definition_index_projection() {
    let fixture = link_object_fixture();
    let stale_closure = MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_without_object_projection_for_test(
            &fixture.plan,
            digest_patch_intent(),
            fixture.plan.scoop_lir_members()[0].member_id(),
            fixture.checked_offset,
        ),
    )
    .unwrap();
    let bytes = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), stale_closure],
    );

    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations()
            .unwrap()
            .validate_c_bridge_envelopes(&c_bridge_profile())
            .unwrap()
            .validate_builtin_objects()
            .unwrap()
            .validate_digest_patch_sites(),
        Err(StrongLinkDigestPatchError::ClosureProjection(
            LinkObjectProjectionValidationError::ProjectionMismatch
        ))
    ));
}

#[test]
fn link_symbol_validation_rejects_a_stale_defined_owner_projection() {
    let fixture = link_object_fixture();
    let stale_closure = MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_without_symbol_projection_for_test(
            &fixture.plan,
            &fixture.builtins,
            digest_patch_intent(),
            fixture.plan.scoop_lir_members()[0].member_id(),
            fixture.checked_offset,
        ),
    )
    .unwrap();
    let bytes = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), stale_closure],
    );

    let dependency_owners = Vec::new();
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations()
            .unwrap()
            .validate_c_bridge_envelopes(&c_bridge_profile())
            .unwrap()
            .validate_builtin_objects()
            .unwrap()
            .validate_digest_patch_sites()
            .unwrap()
            .validate_registration_objects()
            .unwrap()
            .fingerprint_registration_leaves()
            .unwrap()
            .validate_link_symbol_requirements(&dependency_owners, &c_bridge_profile()),
        Err(StrongLinkSymbolRequirementError::ClosureProjection(
            LinkSymbolProjectionValidationError::DefinedSymbols(
                crate::DefinedLinkSymbolOwnerValidationError::ProjectionMismatch
            )
        ))
    ));
}

#[test]
fn link_section_decode_requires_foundations_and_matching_semantic_fingerprints() {
    let missing = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        true,
        false,
        false,
        true,
    );
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            crate::ArtifactProfileInventoryError::MissingRequiredCapability {
                view: crate::ArtifactProfileView::Link,
                location: crate::SectionLocation::Lir,
                ..
            }
        ))
    ));

    let missing_hir_production = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        false,
        true,
        false,
        true,
    );
    assert!(matches!(
        open_graph(&missing_hir_production).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::MissingSection {
            location: Some(MetadataLocation::Hir),
            ..
        })
    ));

    let stale = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        true,
        true,
        true,
        true,
    );
    assert!(matches!(
        open_graph(&stale).decode_single_cone_link_sections(),
        Err(
            SingleConeLinkSectionDecodeError::SemanticFingerprintMismatch {
                location: MetadataLocation::Hir,
                ..
            }
        )
    ));
}

#[test]
fn link_section_decode_rejects_the_wrong_profile_before_payloads() {
    let (hir, mir, lir) = crate::strong_compile_decode::tests::required_sections();
    let bytes = crate::strong_compile_decode::tests::build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        hir,
        mir,
        lir,
        false,
    );
    assert!(matches!(
        open_graph(&bytes).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::WrongProfile { .. })
    ));
}

#[test]
fn link_section_decode_closes_manifest_inventory() {
    let missing = artifact(Vec::new(), vec![strong_section(), closure_section()]);
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                location: crate::SectionLocation::Manifest,
                ..
            }
        ))
    ));

    let unknown = ManifestSection::new(
        CapabilityId::new("org.scoop-lang.test", "link", 1).unwrap(),
        MemberPurposeSet::LINK,
        Vec::new(),
    )
    .unwrap();
    let unsupported = artifact(
        vec![production_manifest_section(), unknown],
        vec![strong_section(), closure_section()],
    );
    assert!(matches!(
        open_graph(&unsupported).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                location: crate::SectionLocation::Manifest,
                ..
            }
        ))
    ));
}

#[test]
fn link_section_decode_closes_lir_inventory_but_keeps_optional_opaque() {
    let missing = artifact(vec![production_manifest_section()], vec![strong_section()]);
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                location: crate::SectionLocation::Lir,
                ..
            }
        ))
    ));

    let unknown = MetadataSection::new(
        MetadataLocation::Lir,
        CapabilityId::new("org.scoop-lang.test", "link-metadata", 1).unwrap(),
        MemberPurposeSet::LINK,
        Vec::new(),
    )
    .unwrap();
    let unsupported = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section(), unknown],
    );
    assert!(matches!(
        open_graph(&unsupported).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                location: crate::SectionLocation::Lir,
                ..
            }
        ))
    ));

    let optional = MetadataSection::new(
        MetadataLocation::Lir,
        CapabilityId::new("org.scoop-lang.test", "optional-metadata", 1).unwrap(),
        MemberPurposeSet::NONE,
        Vec::new(),
    )
    .unwrap();
    let accepted = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section(), optional],
    );
    assert!(
        open_graph(&accepted)
            .decode_single_cone_link_sections()
            .is_ok()
    );
}
