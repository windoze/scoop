//! Dependency TD/dispatch references traverse the actual ten-field wire.

use super::*;
use crate::{
    DecodedStrongProductionSectionV2, EntryProductionSourceV1, ReplayedStrongProductionSectionV2,
    StrongProductionSectionV2, StrongProductionSectionValidationError as SectionError,
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
    meter: &mut BudgetMeter,
) -> Result<ReplayedStrongProductionSectionV2, SectionError> {
    let decoded: DecodedStrongProductionSectionV2 =
        decode_canonical(bytes, DecodeLimits::default()).unwrap();
    decoded.replay(
        ConeCoordinate::reserved_single_file(),
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::SINGLE_FILE, Vec::new()).unwrap(),
        fixture.digests.clone(),
        EntryProductionSourceV1::Library,
        &[],
        None,
        definitions,
        &StrongInitializationDefinitionCatalogV2::new(
            ConeIdentity::SINGLE_FILE,
            &[],
            &mut super::meter(),
        )
        .unwrap(),
        meter,
    )
}

#[test]
fn ten_field_section_replays_foreign_parent_interface_and_dispatch() {
    let fixture = complete_fixture();
    let original = section(&fixture);
    let bytes = encode(&original).unwrap();
    assert_eq!(bytes[0], 0xaa);
    assert!(
        decode_canonical::<crate::DecodedStrongProductionSectionV1>(
            &bytes,
            DecodeLimits::default()
        )
        .is_err()
    );
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let replayed = replay_section(&fixture, &bytes, &definitions, &mut meter()).unwrap();
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
    let empty = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[], &mut meter())
        .unwrap();
    assert!(matches!(
        replay_section(&fixture, &bytes, &empty, &mut meter()),
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
        replay_section(&fixture, &altered, &definitions, &mut meter()),
        Err(SectionError::SectionMismatch)
    ));
}

#[test]
fn ten_field_section_replay_uses_one_cumulative_budget() {
    let fixture = complete_fixture();
    let bytes = encode(&section(&fixture)).unwrap();
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let mut baseline = meter();
    replay_section(&fixture, &bytes, &definitions, &mut baseline).unwrap();
    let usage = baseline.usage();
    let mut shared = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: usage.logical_heap_bytes,
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    replay_section(&fixture, &bytes, &definitions, &mut shared).unwrap();
    assert!(matches!(
        replay_section(&fixture, &bytes, &definitions, &mut shared),
        Err(SectionError::Resource(_))
    ));
}

#[test]
fn final_layout_join_rejects_missing_local_descriptor_exports() {
    let fixture = complete_fixture();
    let bytes = encode(&section(&fixture)).unwrap();
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let replayed = replay_section(&fixture, &bytes, &definitions, &mut meter()).unwrap();
    let missing = replayed.type_registrations().registrations()[0].exact_type();
    let layout = empty_layout_section();
    assert!(matches!(
        replayed.validate_layout_abi(&layout, &mut meter()),
        Err(crate::StrongProductionLayoutJoinError::MissingDescriptorExport(actual))
            if actual == missing
    ));
}

struct EmptyLayoutSource;

impl crate::LayoutAbiSectionSourceAuthorityV1<()> for EmptyLayoutSource {
    fn validate_local_exports(
        &self,
        _: &crate::LayoutAbiExportConstituentsV1,
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[crate::LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }

    fn validate_physical_imports(
        &self,
        imports: &[crate::ExternalShapeLinkImportV1<'_>],
        _: &mut BudgetMeter,
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
        &mut meter(),
    )
    .unwrap();
    let descriptors = crate::CanonicalExactDescriptorExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
        &mut meter(),
    )
    .unwrap();
    let exports = crate::LayoutAbiExportConstituentsV1::try_new(
        layouts.clone(),
        descriptors.clone(),
        crate::CanonicalExactDispatchExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
            &mut meter(),
        )
        .unwrap(),
        crate::CanonicalExactCallableAbiExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
            &mut meter(),
        )
        .unwrap(),
        crate::CanonicalParamFreeShapeSupportExportsV1::from_sources(
            &[],
            &layouts,
            &descriptors,
            &foundation,
            &mut meter(),
        )
        .unwrap(),
    )
    .unwrap();
    crate::CrossConeLayoutAbiSectionV1::try_new(
        exports,
        &[],
        Vec::new(),
        &EmptyLayoutSource,
        &mut meter(),
    )
    .unwrap()
}
