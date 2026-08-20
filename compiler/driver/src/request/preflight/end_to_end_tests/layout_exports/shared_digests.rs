use super::*;

pub(super) fn check(
    name: &str,
    foundation: &lir::ConeLirFoundation,
    production: &lir::ConeProductionSectionV2,
) {
    let replay = || {
        lir::replay_digest_finalization_plan_v2(
            foundation,
            production.registration_production(),
            &lir::EntryProductionSourceV1::Library,
        )
    };
    let expected = replay().unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(&expected, production.digest_finalization_plan());
    let raw: lir::DecodedDigestFinalizationPlanV1 = decoded(&expected);
    assert_eq!(
        raw.resolve_foundation(
            foundation,
            &scoop_lir::StrongTypeReferenceDefinitionsV2::new(foundation.producer(), &[], &[])
                .unwrap()
        )
        .unwrap(),
        expected
    );
}
