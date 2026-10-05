use super::*;

#[test]
fn dependency_companions_keep_the_original_declaration_and_materialize_both_applications() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m29-companions/artifacts");
    super::m23_generic_body_consumption::with_provider_consumer(
        &std::fs::read_to_string(root.join("provider/src/box.scoop")).unwrap(),
        r#"
            import shared.Box
            import shared.integerCompanion
            fun main() {
                val first = Box<Int>.Factory
                val same = integerCompanion()
                val second = Box<String>.Companion
                val value = second.create("text")
            }
        "#,
        |output, _, _, _, _| {
            let export = output.output().export.module();
            assert!(
                export
                    .objects
                    .values()
                    .all(|object| object.name != "Factory")
            );
            let module = output.output().local.module();
            let values = module
                .objects
                .values()
                .filter(|object| object.name == "Factory")
                .map(|object| &module.singleton_values[object.singleton_value])
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 2);
            assert_eq!(values[0].identity, values[1].identity);
            assert_ne!(values[0].published_root, values[1].published_root);
            assert_ne!(values[0].initialization, values[1].initialization);
            for value in values {
                assert_eq!(
                    module.initialization_units[value.initialization]
                        .dependencies
                        .len(),
                    1
                );
                assert!(matches!(
                    module.initialization_units[value.initialization]
                        .identity
                        .key(),
                    scoop_identity::InitializationUnitKey::GenericCompanionApplication { .. }
                ));
            }
        },
    )
    .unwrap();
}

#[test]
fn companion_host_arguments_select_distinct_types_and_initialization_units() {
    let source = r#"
        public struct Box<T>(val value: T) {
            public companion object Factory {
                public var visits: Int = 0
                public fun make(value: T): Box<T> {
                    visits += 1
                    return Box<T>(value)
                }
            }
        }
        typealias IntBox = Box<Int>
        fun main() {
            val first = Box<Int>.make(7)
            val second = Box<String>.Factory.make("text")
            val again = IntBox.Companion.make(8)
        }
    "#;
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
        .expect("complete host applications must resolve and materialize");
    let objects = output
        .local
        .objects
        .iter()
        .filter(|(_, object)| object.name == "Factory")
        .map(|(_, object)| object)
        .collect::<Vec<_>>();
    assert_eq!(objects.len(), 2, "the alias reuses the Int application");
    assert_ne!(objects[0].object_type, objects[1].object_type);
    let values = objects
        .iter()
        .map(|object| &output.local.singleton_values[object.singleton_value])
        .collect::<Vec<_>>();
    assert_ne!(values[0].published_root, values[1].published_root);
    assert_ne!(values[0].initialization, values[1].initialization);
    let first = &output.local.initialization_units[values[0].initialization];
    let second = &output.local.initialization_units[values[1].initialization];
    assert_ne!(first.identity.id(), second.identity.id());
    assert_ne!(first.failure_root, second.failure_root);
    assert!(matches!(
        first.identity.key(),
        scoop_identity::InitializationUnitKey::GenericCompanionApplication { .. }
    ));
}
