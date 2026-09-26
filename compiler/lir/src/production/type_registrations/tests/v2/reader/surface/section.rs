//! Dependency TD/dispatch references traverse the actual ten-field wire.

use super::*;
use crate::{
    DecodedStrongProductionSectionV2, EntryProductionSourceV1, StrongProductionSectionV2,
    StrongProductionSectionValidationError as SectionError,
};
use scoop_identity::ConeCoordinate;

fn complete_fixture() -> Fixture {
    let mut fixture = foreign_fixture();
    crate::production::strong_section::tests::attach_image(
        &ConeCoordinate::reserved_single_file(),
        &mut fixture.foundation,
        &mut fixture.digests,
    );
    fixture
}

fn section(fixture: &Fixture) -> StrongProductionSectionV2 {
    StrongProductionSectionV2::from_parts(
        ConeCoordinate::reserved_single_file(),
        &[],
        &fixture.foundation,
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap(),
        fixture.digests.clone(),
        surface(fixture, true),
        EntryProductionSourceV1::Library,
        &[],
        None,
    )
    .unwrap()
}

fn replay_section(
    fixture: &Fixture,
    bytes: &[u8],
    definitions: &StrongTypeReferenceDefinitionsV2,
) -> Result<StrongProductionSectionV2, SectionError> {
    let decoded: DecodedStrongProductionSectionV2 = decode_canonical(bytes).unwrap();
    decoded.replay(
        ConeCoordinate::reserved_single_file(),
        &[],
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap(),
        EntryProductionSourceV1::Library,
        &[],
        None,
        definitions,
        &StrongInitializationDefinitionCatalogV2::new(ConeIdentity::SINGLE_FILE, &[]).unwrap(),
    )
}

#[test]
fn ten_field_section_replays_foreign_parent_interface_and_dispatch() {
    let fixture = complete_fixture();
    let original = section(&fixture);
    let bytes = encode(&original).unwrap();
    assert_eq!(bytes[0], 0xaa);
    let shared: crate::DecodedStrongProductionSectionV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&shared).unwrap(), bytes);
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let replayed = replay_section(&fixture, &bytes, &definitions).unwrap();
    assert_eq!(replayed.external_bridges(), original.external_bridges());
    assert_eq!(
        replayed.canonical_definitions(),
        original.canonical_definitions()
    );
    assert_eq!(
        replayed.object_definition_plans(),
        original.object_definition_plans()
    );
    assert_eq!(
        replayed.digest_finalization_plan(),
        original.digest_finalization_plan()
    );
    assert_eq!(
        replayed.registration_identities(),
        original.registration_production().identities()
    );
    assert_eq!(
        replayed.type_registrations(),
        original.registration_production().types()
    );
    assert_eq!(replayed.image_plan(), original.image_plan());
    assert_eq!(replayed.entry_plan(), original.entry_plan());
    assert_eq!(replayed.shape_support_plan(), original.shape_support_plan());
    assert_eq!(
        replayed.generated_bridge_plan(),
        original.generated_bridge_plan()
    );
    assert_eq!(
        replayed.initialization_cycle_abi(),
        original.initialization_cycle_abi()
    );
}

#[test]
fn ten_field_section_rejects_missing_foreign_definitions_and_changed_definition_atoms() {
    let fixture = complete_fixture();
    let original = section(&fixture);
    let bytes = encode(&original).unwrap();
    let empty = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[]).unwrap();
    assert!(matches!(
        replay_section(&fixture, &bytes, &empty),
        Err(SectionError::Registrations(Error::TypeReference(_)))
    ));
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let mut altered = bytes;
    let atom = fixture.foundation.definition_atoms()[0].id();
    let position = altered
        .windows(32)
        .position(|bytes| bytes == atom.as_array())
        .unwrap();
    altered[position] ^= 1;
    assert!(matches!(
        replay_section(&fixture, &altered, &definitions),
        Err(SectionError::SectionMismatch)
    ));
}

#[test]
fn ten_field_section_rejects_a_valid_digest_graph_missing_a_registration_input() {
    let fixture = complete_fixture();
    let bytes = crate::production::strong_section::tests::without_image_input(
        &section(&fixture),
        &fixture.foundation,
    );
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    assert!(matches!(
        replay_section(&fixture, &bytes, &definitions),
        Err(SectionError::DigestMismatch)
    ));
}

#[test]
fn final_layout_join_keeps_dependency_checks_for_unexported_local_types() {
    let fixture = complete_fixture();
    let bytes = encode(&section(&fixture)).unwrap();
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let replayed = replay_section(&fixture, &bytes, &definitions).unwrap();
    let layout = empty_layout_section();
    assert!(matches!(
        replayed.validate_layout_abi(&layout),
        Err(crate::StrongProductionLayoutJoinError::MissingSelectedDescriptor { provider, .. })
            if provider == ConeIdentity::CORE
    ));
}

#[test]
fn final_layout_join_preserves_complete_private_type_registrations() {
    let mut fixture = Fixture::new(Options::default());
    let coordinate = ConeCoordinate::reserved_single_file();
    crate::production::strong_section::tests::attach_image(
        &coordinate,
        &mut fixture.foundation,
        &mut fixture.digests,
    );
    let production = StrongProductionSectionV2::from_parts(
        coordinate,
        &[],
        &fixture.foundation,
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap(),
        fixture.digests.clone(),
        surface(&fixture, false),
        EntryProductionSourceV1::Library,
        &[],
        None,
    )
    .unwrap();
    let expected = production.registration_production().types().clone();
    assert!(!expected.registrations().is_empty());
    let joined = production
        .validate_layout_abi(&empty_layout_section())
        .unwrap();
    assert_eq!(joined.type_registrations(), &expected);
}

struct EmptyLayoutSource;

impl crate::LayoutAbiSectionSourceAuthorityV1<()> for EmptyLayoutSource {
    fn validate_local_exports(&self, _: &crate::LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[crate::LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }

    fn validate_physical_imports(
        &self,
        imports: &[crate::ExternalShapeLinkImportV1],
    ) -> Result<(), ()> {
        imports.is_empty().then_some(()).ok_or(())
    }
}

fn empty_layout_section() -> crate::CrossConeLayoutAbiSectionV1<'static> {
    let foundation = crate::OdrFreeLirFoundation::try_new(
        ConeIdentity::SINGLE_FILE,
        crate::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    let layouts = crate::CanonicalExactLayoutExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let descriptors = crate::CanonicalExactDescriptorExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let exports = crate::LayoutAbiExportConstituentsV1::try_new(
        layouts.clone(),
        descriptors.clone(),
        crate::CanonicalExactDispatchExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalExactCallableAbiExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalParamFreeShapeSupportExportsV1::from_sources(
            &[],
            &layouts,
            &descriptors,
            &foundation,
        )
        .unwrap(),
    )
    .unwrap();
    crate::CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &EmptyLayoutSource)
        .unwrap()
}
