use super::*;
use scoop_identity::{CallableTemplateOrigin, PropertyOwner, SignatureTypeKey};

#[test]
fn class_shape_keeps_primary_properties_and_original_defaults_across_cones() {
    super::m23_generic_body_consumption::with_provider_consumer(
        r#"
            package shapes
            public class Row<T> public constructor(prefix: Int, public val value: T, private var counter: Int = prefix) {
                public val computed: Int get() = counter
            }
            public class Empty()
            public class Secondary { public constructor(value: Int) {} }
        "#,
        r#"
            import shapes.Row
            public fun row(): Row<String> = Row<String>(7, "value")
        "#,
        |output, _, _, interface, _| {
            let row = interface.nominal_interfaces().all_records().find(|nominal| {
                nominal.type_parameters().len_u32() == 1
            }).unwrap();
            let primary = row.declaration_details().class_primary_constructor().unwrap();
            let properties = primary.properties();
            assert_eq!(properties.len(), 3);
            assert_eq!(properties[0], None);
            let value = interface.property_interfaces().declaration(PropertyOwner::Property(properties[1].unwrap())).unwrap();
            assert_eq!(value.value_type(), &SignatureTypeKey::Binder { depth: 0, index: 0 });
            let counter = interface.property_interfaces().declaration(PropertyOwner::Property(properties[2].unwrap())).unwrap();
            assert_eq!(counter.declared_visibility(), hir::DeclaredVisibilityV1::Private);
            let owner = CallableTemplateOrigin::Constructor(primary.constructor());
            let source = interface.source_interfaces().get(owner).unwrap();
            let parameters = source.parameters().parameters();
            assert_eq!(parameters[2].name().as_str(), "counter");
            assert!(parameters[2].calling().template().is_some());
            let imported = output.output().export.loaded_class_definitions.values()
                .find(|definition| definition.declaration.name() == "Row").unwrap();
            assert_eq!(imported.declaration.interface.declaration_details().class_primary_constructor(), Some(primary));
            let other_classes = interface.nominal_interfaces().all_records()
                .filter(|nominal| nominal.kind() == hir::PublicNominalKindV1::Class && nominal.type_parameters().is_empty())
                .collect::<Vec<_>>();
            assert_eq!(other_classes.len(), 2);
            assert_eq!(other_classes.iter().filter(|nominal| nominal.declaration_details().class_primary_constructor().is_none()).count(), 1);
            assert!(other_classes.iter().filter_map(|nominal| nominal.declaration_details().class_primary_constructor()).all(|primary| primary.properties().is_empty()));
        },
    ).unwrap();
}
