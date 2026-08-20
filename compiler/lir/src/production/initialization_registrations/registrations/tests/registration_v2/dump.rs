use super::*;
use crate::dump_initialization_dependencies;

#[test]
fn production_dump_distinguishes_external_and_local_initialization_edges() {
    for lazy in [false, true] {
        let options = Options {
            lazy,
            ..Options::default()
        };
        let consumer = Fixture::new(options);
        let local = Fixture::with_source(options, ConeIdentity::SINGLE_FILE, "local");
        let external = Fixture::with_source(options, ConeIdentity::CORE, "external");
        let reference =
            |provider| dependency(consumer.foundation.producer(), consumer.unit, provider);
        assert!(
            dump_initialization_dependencies(&build(&consumer, Vec::new()).unwrap()).is_empty()
        );
        assert!(
            dump_initialization_dependencies(&build(&consumer, vec![reference(&local)]).unwrap())
                .is_empty()
        );
        let mut references = vec![reference(&local), reference(&external)];
        references.sort_by_key(Dependency::unit);
        let plans = build(&consumer, references.clone()).unwrap();
        let expected = references
            .iter()
            .map(|value| {
                if value.unit() == local.unit {
                    format!(
                        "local(provider={},unit={})",
                        ConeIdentity::SINGLE_FILE,
                        local.unit
                    )
                } else {
                    format!(
                        "external(provider={},unit={})",
                        ConeIdentity::CORE,
                        external.unit
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(
            dump_initialization_dependencies(&plans),
            format!("  init-dependencies {} [{expected}]\n", consumer.unit)
        );
    }
}
