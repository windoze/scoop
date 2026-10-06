use super::*;

#[test]
fn static_class_shapes_keep_properties_primary_mapping_and_base_separate() {
    with_shapes(
        r#"
            package shapes
            public annotation class Label(val text: String)
            public open class Base<T> public constructor(public val base: T)
            public class Row<T> public constructor(seed: Int, @Label("value") public val value: T, private var count: Int = seed) : Base<T>(value) {
                public val computed: Int get() = count
            }
            public class Empty()
            public class Secondary { public constructor(value: Int) {} }
        "#,
        r#"
            import shapes.Row
            import shapes.Empty
            import shapes.Secondary
            public fun row(value: Row<String>): Row<String> = value
            public fun empty(value: Empty): Empty = value
            public fun secondary(value: Secondary): Secondary = value
        "#,
        |module, world, imported| {
            let shape = nominal(module, "row");
            assert_eq!(shape.kind(), hir::StaticNominalKind::Class);
            let SignatureTypeKey::NominalApplication { arguments, .. } = shape
                .base_class(world)
                .unwrap()
                .signature(module, &[])
                .unwrap()
            else {
                panic!("generic base")
            };
            assert_eq!(
                arguments.as_slice(),
                &[signature(module, shape.arguments()[0])]
            );
            let fields = shape.fields().collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                2,
                "base and computed property have no direct field"
            );
            assert!(
                fields
                    .iter()
                    .all(|field| field.annotations(world).is_empty())
            );
            let properties = shape.properties(world);
            assert_eq!(
                properties
                    .iter()
                    .map(|property| property.name())
                    .collect::<Vec<_>>(),
                ["value", "count", "computed"]
            );
            assert_eq!(
                annotation_text(properties[0].annotations(world), module, world),
                "value"
            );
            assert_eq!(
                properties[0].value_type(&[]).unwrap(),
                signature(module, shape.arguments()[0])
            );
            assert!(properties[0].field_storage().is_some());
            assert!(properties[1].field_storage().is_some());
            assert!(properties[2].field_storage().is_none());
            let parameters = shape
                .primary_constructor(world)
                .unwrap()
                .parameters()
                .collect::<Vec<_>>();
            assert_eq!(parameters[0].target(), hir::StaticParameterTarget::Plain);
            assert_eq!(
                parameters[1].target(),
                hir::StaticParameterTarget::Property(properties[0].identity())
            );
            assert_eq!(
                parameters[2].target(),
                hir::StaticParameterTarget::Property(properties[1].identity())
            );
            assert_eq!(
                matches!(
                    parameters[2].default().unwrap(),
                    hir::StaticDefaultReference::Dependency { .. }
                ),
                imported
            );
            assert_eq!(
                nominal(module, "empty")
                    .primary_constructor(world)
                    .unwrap()
                    .parameters()
                    .len(),
                0
            );
            assert!(
                nominal(module, "secondary")
                    .primary_constructor(world)
                    .is_none()
            );
        },
    );
}

#[test]
fn static_companion_shapes_preserve_host_arguments_and_property_storage() {
    with_shapes(
        r#"
            package shapes
            public struct Box<T>(val value: T) {
                public companion object Factory {
                    public var count: Int = 0
                    public val current: Option<T> = None
                }
            }
        "#,
        r#"
            import shapes.Box
            public fun factory(): Box<String>.Factory = Box<String>.Factory
        "#,
        |module, world, _| {
            let shape = nominal(module, "factory");
            assert_eq!(shape.kind(), hir::StaticNominalKind::Object);
            assert_eq!(shape.arguments().len(), 1);
            assert!(matches!(
                module.types[shape.arguments()[0]],
                hir::Type::String
            ));
            assert!(shape.primary_constructor(world).is_none());
            let fields = shape.fields().collect::<Vec<_>>();
            let properties = shape.properties(world);
            assert_eq!(properties.len(), 2);
            for (field, property) in fields.iter().zip(properties) {
                assert_eq!(field.name(), property.name());
                assert_eq!(
                    field.storage(),
                    hir::NominalFieldStorage::PropertyBacking(property.identity())
                );
                assert_eq!(
                    field.value_type().signature(module, &[]).unwrap(),
                    property.value_type(&[]).unwrap()
                );
            }
        },
    );
}
