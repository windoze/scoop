//! Saved symbol ownership and publication data survive ordinary dependency reads.

use super::lir_dependencies::reader;
use scoop_slib as slib;

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
) {
    let complete = reader::read_link(core, artifact)
        .read_definition_data()
        .unwrap();
    assert_eq!(complete.dependency_first().len(), 2);
    for (physical, definitions) in complete.dependency_first() {
        let source = if physical.identity() == core.summary().identity() {
            core
        } else {
            artifact
        };
        assert_eq!(physical.identity(), definitions.provider());
        assert_eq!(definitions.summary(), source.summary());
        assert!(!definitions.defined_symbols().owners().is_empty());
        for owner in definitions.defined_symbols().owners() {
            assert!(
                physical
                    .manifest()
                    .members()
                    .iter()
                    .any(|record| record.id() == owner.member())
            );
        }
    }
}
