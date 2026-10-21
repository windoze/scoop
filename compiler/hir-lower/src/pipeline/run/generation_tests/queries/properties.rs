use super::types::resolve;
use super::*;

#[test]
fn property_queries_keep_logical_capabilities_bodies_annotations_and_cached_identities() {
    let source = r#"
        annotation class Mark
        annotation class Label(val text: String)
        class Box<T>(val value: T) {
            @get:Label("read") @set:Label("write")
            var mirror: T get() = value
                private set(next) {}
            private val hidden: Int = 1
            val computed: T get() = value
        }
        interface View<T> { val item: T }
        object Values { const val Tag: Int = 1 }
        @Mark fun inspect() {}
    "#;
    let mut accessors = Vec::new();
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let ty = resolve(queries, context, "Box<String>");
        let string = resolve(queries, context, "String");
        let properties = queries.nominal_properties(context, ty).unwrap().unwrap();
        assert_eq!(
            properties
                .iter()
                .map(|property| property.name.as_str())
                .collect::<Vec<_>>(),
            ["value", "mirror", "hidden", "computed"]
        );
        let mirror = &properties[1];
        assert_eq!(mirror.value_type.applied, string);
        assert_ne!(mirror.value_type.declaration, string);
        assert!(mirror.getter.access.accessible);
        let setter = mirror.setter.as_ref().unwrap();
        assert!(!setter.access.accessible);
        assert_eq!(setter.access.visibility, hir::DeclaredVisibilityV1::Private);
        assert!(!properties[2].getter.access.accessible);
        assert!(properties[3].setter.is_none());
        for accessor in [&mirror.getter, setter] {
            assert!(matches!(
                queries.body(accessor.callable),
                Some(Availability::Available(Body::Current(_)))
            ));
            assert!(
                matches!(queries.describe_annotations(accessor.annotations), hir::SemanticAnnotations::Current(annotations) if annotations.len() == 1)
            );
            accessors.push((accessor.callable, accessor.identity));
        }
        let fields = queries.nominal_fields(context, ty).unwrap().unwrap();
        assert!(fields.fields.iter().any(|field| field.storage
            == hir::NominalFieldStorage::PropertyBacking(properties[0].identity)));
        assert!(!fields.fields.iter().any(|field| matches!(field.storage, hir::NominalFieldStorage::PropertyBacking(id) if id == mirror.identity)));
        let view = resolve(queries, context, "View<String>");
        let properties = queries.nominal_properties(context, view).unwrap().unwrap();
        assert!(matches!(
            queries.body(properties[0].getter.callable),
            Some(Availability::NoBody)
        ));
        let constants = resolve(queries, context, "Values");
        let properties = queries
            .nominal_properties(context, constants)
            .unwrap()
            .unwrap();
        assert!(matches!(
            queries.body(properties[0].getter.callable),
            Some(Availability::NoBody)
        ));
        Ok(Vec::new())
    });
    let module = compile(source, &mut driver).unwrap();
    for (callable, identity) in accessors {
        let actual = match callable {
            hir::SemanticCallable::CurrentGetter(id) => {
                module.property_accessor_identities[id].id()
            }
            hir::SemanticCallable::CurrentSetter(id) => {
                module.property_accessor_identities[id].id()
            }
            _ => panic!("queried source accessor"),
        };
        assert_eq!(actual, identity);
    }
}

#[test]
fn property_queries_apply_protected_receiver_rules_and_actual_override_relations() {
    let source = r#"
        annotation class Mark
        open class Base() {
            protected open val guarded: Int get() = 1
            @Mark fun inside() {}
        }
        class Child() : Base() {
            protected override val guarded: Int get() = 2
            @Mark fun child() {}
        }
        @Mark fun outside() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let functions = marked_functions(queries);
        let context = |name: &str| {
            SemanticContext::Function(
                *functions
                    .iter()
                    .find(|function| queries.signature(**function).name == name)
                    .unwrap(),
            )
        };
        let inside = context("inside");
        let child = context("child");
        let outside = context("outside");
        let base_type = resolve(queries, inside, "Base");
        let child_type = resolve(queries, child, "Child");
        let base = queries
            .nominal_properties(inside, base_type)
            .unwrap()
            .unwrap();
        assert!(base[0].getter.access.accessible);
        let inherited = queries
            .nominal_properties(child, base_type)
            .unwrap()
            .unwrap();
        assert!(!inherited[0].getter.access.accessible);
        let own = queries
            .nominal_properties(child, child_type)
            .unwrap()
            .unwrap();
        assert!(own[0].getter.access.accessible);
        assert_eq!(own[0].overrides, [base[0].identity]);
        assert_ne!(own[0].getter.identity, base[0].getter.identity);
        let inaccessible = queries
            .nominal_properties(outside, child_type)
            .unwrap()
            .unwrap();
        assert!(!inaccessible[0].getter.access.accessible);
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
