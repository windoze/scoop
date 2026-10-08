use super::*;
use scoop_identity::{CallableTemplateOwner, GeneratedCallableKey, PersistentGeneratedCallableId};

#[test]
fn unit_equality_calls_the_ordinary_core_implementation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-cli-unit-equality/cross-cone/provider/src/main.scoop"
    ));
    with_source(source, |output, mir| {
        let unit = scoop_mir::core_unit_exact_type();
        let dependencies = output.executable_dependency_callables().unwrap();
        assert_eq!(dependencies.len(), 1);
        let equality = dependencies[0].callable();
        assert_eq!(equality.provider(), scoop_identity::ConeIdentity::CORE);
        assert_eq!(equality.capability().signature().parameters(), &[unit]);
        assert_eq!(mir.meta.external_callables.len(), 1);
        let generated =
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::DerivedEquality {
                exact_owner: unit,
            })
            .unwrap();
        assert!(
            mir.meta
                .source_callable_materializations
                .iter()
                .all(|entry| {
                    entry.materialization().template()
                        != CallableTemplateOwner::Generated(generated)
                })
        );
    });
}
