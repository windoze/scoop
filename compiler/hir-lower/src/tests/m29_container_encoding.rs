use super::*;

#[test]
fn containers_keep_unconstrained_elements_and_materialize_only_applicable_encoders() {
    let output = lower_with_sysroot(
        r#"
        struct Opaque()
        fun main() {
            val a = Array<Int>(1L) { _ -> 7 }
            val b = Array<Opaque>(1L) { _ -> Opaque() }
            val c = MutableArray<Int>(1L) { _ -> 7 }
            val d = MutableArray<Opaque>(1L) { _ -> Opaque() }
            val e = ArrayList<Int>()
            val f = ArrayList<Opaque>()
            val g: Option<Int> = Some(7)
            val h: Option<Opaque> = None
            val erased: Any = a
            if (erased is Encodable) {}
        }
    "#,
    )
    .unwrap();
    for declaration in output.export.classes.values().filter(|declaration| {
        ["Array", "MutableArray", "ArrayList"].contains(&declaration.name.as_str())
    }) {
        assert_eq!(declaration.type_params.len(), 1);
        assert!(matches!(
            declaration.type_params[0].bounds,
            hir::TypeParamBounds::Unconstrained
        ));
        assert!(declaration.element_encoding.is_some());
    }
    let encodes = |interfaces: &[hir::concrete::TypeId]| {
        interfaces.iter().any(|ty| {
            matches!(output.local.types[*ty].kind, hir::concrete::TypeKind::Interface(id)
            if output.local.interfaces[id].name == "Encodable")
        })
    };
    let mut checked = 0;
    for (arguments, interfaces) in output
        .local
        .classes
        .values()
        .filter(|declaration| {
            ["Array", "MutableArray", "ArrayList"].contains(&declaration.name.as_str())
        })
        .map(|declaration| (&declaration.type_arguments, &declaration.interfaces))
        .chain(
            output
                .local
                .enums
                .values()
                .filter(|declaration| declaration.name == "Option")
                .map(|declaration| (&declaration.type_arguments, &declaration.interfaces)),
        )
    {
        match output.local.types[arguments[0]].kind {
            hir::concrete::TypeKind::Integer(_) => {
                assert!(encodes(interfaces));
                checked += 1;
            }
            hir::concrete::TypeKind::Struct(id) if output.local.structs[id].name == "Opaque" => {
                assert!(!encodes(interfaces));
                checked += 1;
            }
            _ => {}
        }
    }
    assert!(
        checked >= 8,
        "all four positive and negative applications are concrete"
    );
}

#[test]
fn bounded_nested_and_recursive_elements_use_ordinary_encoding_methods() {
    lower_with_sysroot(
        r#"
        class Node(public val children: Array<Node>) : Encodable
        fun <T : Encodable> send(value: Array<Option<T>>, encoder: Encoder) {
            value.encode(encoder)
            val reference = value::encode
            reference(encoder)
        }
        fun main() {
            val empty = Array<Node>(0L) { _ -> throw IllegalStateException(None) }
            val root: Any = Node(empty)
            val children: Any = empty
            if (children is Encodable && root is Encodable) {}
        }
    "#,
    )
    .unwrap();
}

#[test]
fn unconstrained_elements_do_not_gain_encoding_members_or_bounds() {
    for container in ["Array", "MutableArray", "ArrayList", "Option"] {
        let errors = lower_with_sysroot(&format!(
            r#"
            fun <T> send(value: {container}<T>, encoder: Encoder) {{ value.encode(encoder) }}
            fun main() {{}}
        "#
        ))
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains("encode")),
            "{errors:?}"
        );
    }
}
