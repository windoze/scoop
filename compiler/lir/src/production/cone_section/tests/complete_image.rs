//! Add the same complete image contract to nonempty registration fixtures.

use super::*;

pub(in crate::production) fn attach_image(
    coordinate: &ConeCoordinate,
    foundation: &mut ConeLirFoundation,
    digests: &mut DigestFinalizationPlanV1,
) {
    let (image_foundation, image_digests) = fixture(coordinate);
    assert_eq!(foundation.producer(), image_foundation.producer());
    let mut canonical = foundation.as_canonical().clone();
    canonical
        .set_definition_plans(
            foundation
                .definition_plans()
                .iter()
                .chain(image_foundation.definition_plans())
                .cloned()
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            foundation
                .definition_atoms()
                .iter()
                .chain(image_foundation.definition_atoms())
                .cloned()
                .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            foundation
                .symbol_requests()
                .iter()
                .chain(image_foundation.symbol_requests())
                .copied()
                .collect(),
        )
        .unwrap(),
    );
    let producer = foundation.producer();
    *foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();
    let mut nodes = digests.nodes().to_vec();
    let image = &image_digests.nodes()[0];
    let current = nodes
        .iter_mut()
        .find(|node| node.id() == image.id())
        .unwrap();
    *current = DigestNodeV1::new(
        *image.key(),
        current.direct_inputs().to_vec(),
        image
            .patch_intents()
            .iter()
            .map(|patch| *patch.key())
            .collect(),
    )
    .unwrap();
    *digests = DigestFinalizationPlanV1::new(nodes, foundation).unwrap();
}
