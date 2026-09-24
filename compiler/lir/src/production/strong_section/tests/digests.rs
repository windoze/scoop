use scoop_identity::{DigestNodeKey, DigestOwnerAndRoleKey};
use scoop_wire::{BudgetMeter, WireEncode};

use super::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

pub(in crate::production) fn without_image_input(
    section: &StrongProductionSectionV2,
    foundation: &OdrFreeLirFoundation,
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

pub(super) fn check(section: &StrongProductionSectionV2, foundation: &OdrFreeLirFoundation) {
    let projection = |meter: &mut BudgetMeter| {
        crate::replay_strong_digest_finalization_plan_v2(
            foundation,
            section.registration_production(),
            &EntryProductionSourceV1::Library,
            meter,
        )
    };
    assert_eq!(
        &projection(&mut meter()).unwrap(),
        section.digest_finalization_plan()
    );
    let replay = |raw: DecodedStrongProductionSectionV2, meter: &mut BudgetMeter| {
        raw.validate_initialization_abi(None, meter)
            .unwrap()
            .replay(
                ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap(),
                &[],
                LirTargetProfile::DARWIN_AARCH64,
                foundation,
                section.external_bridges().clone(),
                EntryProductionSourceV1::Library,
                &[],
                None,
                &crate::StrongTypeReferenceDefinitionsV2::new(
                    foundation.producer(),
                    &[],
                    &mut self::meter(),
                )
                .unwrap(),
                &crate::StrongInitializationDefinitionCatalogV2::new(
                    foundation.producer(),
                    &[],
                    &mut self::meter(),
                )
                .unwrap(),
                meter,
            )
    };
    replay(decoded(section), &mut meter()).unwrap();
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
            replay(raw, &mut meter()),
            Err(StrongProductionSectionValidationError::DigestMismatch)
        ));
    }
    let foreign = ConeCoordinate::new("test", "foreign-digests", "1.0.0").unwrap();
    let (_, graph) = super::fixture(&foreign);
    let mut raw: DecodedStrongProductionSectionV2 = decoded(section);
    raw.digest_finalization_plan = decoded(&graph);
    assert!(matches!(
        replay(raw, &mut meter()),
        Err(StrongProductionSectionValidationError::DigestReplay(_))
    ));
    assert!(matches!(
        image.key().owner_and_role(),
        DigestOwnerAndRoleKey::RuntimeImage(_)
    ));
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(projection(&mut BudgetMeter::new(limits)).is_err());
        let raw: crate::DecodedStrongDigestFinalizationPlanV1 =
            decoded(section.digest_finalization_plan());
        assert!(
            raw.resolve_foundation(foundation, &mut BudgetMeter::new(limits))
                .is_err()
        );
    }
    let mut measured = meter();
    projection(&mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    projection(&mut shared).unwrap();
    assert!(projection(&mut shared).is_err());
}
