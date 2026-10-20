use super::*;

#[test]
fn m60_documentation_projection_routes_survive_arena_and_source_order_changes() {
    let first = project(
        &lower_documented(
            r#"
        /** [Record.equals] */ public struct Record<Element>(val value: Element)
        public fun <Element> identity(value: Element) = value
        public class Outer<T> { public inner class Nested<U> }
        public object Service
        "#,
        )
        .unwrap(),
    );
    let second = project(
        &lower_documented(
            r#"
        public class Extra<T>
        public fun <T> before(value: T): T = value
        public object Service
        public class Outer<T> { public inner class Nested<U> }
        public fun <Element> identity(value: Element) = value
        /** [Record.equals] */ public struct Record<Element>(val value: Element)
        "#,
        )
        .unwrap(),
    );
    for (kind, name) in [
        (ApiKind::Struct, "Record"),
        (ApiKind::Function, "identity"),
        (ApiKind::Class, "Nested"),
        (ApiKind::TypeParameter, "U"),
        (ApiKind::Object, "Service"),
        (ApiKind::Function, "equals"),
    ] {
        assert_eq!(
            item(&first, kind, name).key,
            item(&second, kind, name).key,
            "{name}"
        );
    }
    let equality = item(&first, ApiKind::Function, "equals");
    assert!(equality.synthesized);
    assert!(
        signature(equality).contains("Record<Element>"),
        "{}",
        signature(equality)
    );
    assert!(!signature(equality).contains("this:"));
    let record = item(&first, ApiKind::Struct, "Record");
    assert_eq!(
        record.comment.as_ref().unwrap().body.references[0].target,
        LinkTarget::Symbols(vec![equality.key.clone()])
    );
    let service = item(&first, ApiKind::Object, "Service");
    let bindings = first
        .bindings
        .iter()
        .filter(|binding| binding.name == "Service")
        .map(|binding| &binding.target)
        .collect::<Vec<_>>();
    assert!(bindings.contains(&&service.key));
    assert!(bindings.contains(&&service.alternate_keys[0]));
}

#[test]
fn m60_documentation_projection_keeps_overload_members_and_actual_binding_names() {
    let input = project(
        &lower_documented(
            r#"
        /** See [parse]. */ public class Reader
        public fun parse(value: Int): Int = value
        public fun parse(value: String): Int = 0
        "#,
        )
        .unwrap(),
    );
    let functions = input
        .items
        .iter()
        .filter(|item| item.name == "parse")
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 2);
    assert_ne!(functions[0].key, functions[1].key);
    let LinkTarget::Symbols(symbols) = &item(&input, ApiKind::Class, "Reader")
        .comment
        .as_ref()
        .unwrap()
        .body
        .references[0]
        .target
    else {
        panic!()
    };
    assert_eq!(symbols.len(), 2);
    assert!(functions.iter().all(|item| symbols.contains(&item.key)));
    assert_eq!(
        input
            .bindings
            .iter()
            .filter(|binding| binding.name == "parse")
            .count(),
        2
    );
}
