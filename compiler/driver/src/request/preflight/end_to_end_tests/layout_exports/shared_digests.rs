use std::collections::BTreeMap;

use super::*;

pub(super) fn check(
    name: &str,
    foundation: &lir::OdrFreeLirFoundation,
    production: &lir::StrongProductionSectionV2,
) {
    let replay = || {
        lir::replay_strong_digest_finalization_plan_v2(
            foundation,
            production.registration_production(),
            &lir::EntryProductionSourceV1::Library,
        )
    };
    let expected = replay().unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(&expected, production.digest_finalization_plan());
    let raw: lir::DecodedStrongDigestFinalizationPlanV1 = decoded(&expected);
    assert_eq!(
        raw.resolve_foundation(
            foundation,
            &scoop_lir::StrongTypeReferenceDefinitionsV2::new(foundation.producer(), &[], &[])
                .unwrap()
        )
        .unwrap(),
        expected
    );
    if name.starts_with("shared-digests-") {
        let mut counts = BTreeMap::<_, (usize, usize, usize)>::new();
        for node in expected.nodes() {
            let count = counts
                .entry((node.kind().tag(), format!("{:?}", node.kind())))
                .or_default();
            count.0 += 1;
            count.1 += node.direct_inputs().len();
            count.2 += node.patch_intents().len();
        }
        let dump: String = counts
            .into_iter()
            .map(|((_, kind), (nodes, edges, patches))| {
                format!("{kind}: nodes={nodes} inputs={edges} patches={patches}\n")
            })
            .collect();
        let path = crate::workspace_root()
            .join("tests/fixtures/m23-core-layout-exports")
            .join(format!("{name}.digests.snap"));
        if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
            std::fs::write(&path, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(path).unwrap());

        replay().unwrap();
    }
}
