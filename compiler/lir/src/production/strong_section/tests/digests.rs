use scoop_identity::{DigestNodeKey, DigestOwnerAndRoleKey};
use scoop_wire::WireEncode;

use super::*;

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

pub(in crate::production) fn without_image_input(
    section: &StrongProductionSectionV2,
    foundation: &ConeLirFoundation,
) -> Vec<u8> {
    let mut nodes = section.digest_finalization_plan().nodes().to_vec();
    let image = nodes
        .iter_mut()
        .find(|node| node.key() == &DigestNodeKey::runtime_image(foundation.producer()))
        .unwrap();
    assert!(!image.direct_inputs().is_empty());
    *image = DigestNodeV1::new(
        *image.key(),
        image.direct_inputs()[1..].to_vec(),
        image
            .patch_intents()
            .iter()
            .map(|patch| *patch.key())
            .collect(),
    )
    .unwrap();
    let graph = StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap();
    let mut raw: DecodedStrongProductionSectionV2 = decoded(section);
    raw.digest_finalization_plan = decoded(&graph);
    encode(&raw).unwrap()
}

pub(super) fn check(section: &StrongProductionSectionV2, foundation: &ConeLirFoundation) {
    let projection = || {
        crate::replay_strong_digest_finalization_plan_v2(
            foundation,
            section.registration_production(),
            &EntryProductionSourceV1::Library,
        )
    };
    assert_eq!(&projection().unwrap(), section.digest_finalization_plan());
    let replay = |raw: DecodedStrongProductionSectionV2| {
        raw.replay(
            ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap(),
            &[],
            LirTargetProfile::DARWIN_AARCH64,
            foundation,
            EntryProductionSourceV1::Library,
            &[],
            &crate::StrongTypeReferenceDefinitionsV2::new(foundation.producer(), &[], &[]).unwrap(),
            &crate::StrongInitializationDefinitionCatalogV2::new(foundation.producer(), &[])
                .unwrap(),
        )
    };
    replay(decoded(section)).unwrap();
    let image = &section.digest_finalization_plan().nodes()[0];
    let image_key = DigestNodeKey::runtime_image(foundation.producer());
    let no_patch = DigestNodeV1::new(image_key, vec![], vec![]).unwrap();
    let atom = foundation
        .definition_atoms()
        .iter()
        .find(|record| record.key().role() == DefinitionAtomRole::Primary)
        .unwrap()
        .id();
    let extra = DigestNodeV1::new(DigestNodeKey::object_definition(atom), vec![], vec![]).unwrap();
    for nodes in [vec![no_patch], vec![image.clone(), extra]] {
        let graph = StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap();
        let mut raw: DecodedStrongProductionSectionV2 = decoded(section);
        raw.digest_finalization_plan = decoded(&graph);
        assert!(matches!(
            replay(raw),
            Err(StrongProductionSectionValidationError::DigestMismatch)
        ));
    }
    let foreign = ConeCoordinate::new("test", "foreign-digests", "1.0.0").unwrap();
    let (_, graph) = super::fixture(&foreign);
    let mut raw: DecodedStrongProductionSectionV2 = decoded(section);
    raw.digest_finalization_plan = decoded(&graph);
    assert!(matches!(
        replay(raw),
        Err(StrongProductionSectionValidationError::DigestReplay(_))
    ));
    assert!(matches!(
        image.key().owner_and_role(),
        DigestOwnerAndRoleKey::RuntimeImage(_)
    ));

    projection().unwrap();

    projection().unwrap();
}
