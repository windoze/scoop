use super::*;

#[test]
fn containers_keep_unconstrained_elements_without_implicit_encoding() {
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
            if (erased is Encodable<Array<Int>>) {}
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
                assert!(!encodes(interfaces));
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
fn nested_container_codecs_keep_explicit_element_dependencies() {
    lower_with_sysroot(
        r#"
        fun <T> send(value: Array<Option<T>>, element: Encodable<T>, encoder: Encoder) {
            val options = Option<T>.Companion.encoder(element)
            val codec = Array<Option<T>>.Companion.encoder(options)
            codec.encode(value, encoder)
            val reference = codec::encode
            reference(value, encoder)
        }
        fun use(encoder: Encoder) {
            send(Array<Option<Int>>(1L) { _ -> Some(8) }, Int.Companion, encoder)
            val mutable = MutableArray<Int>(1L) { _ -> 9 }
            MutableArray<Int>.Companion.encoder(Int.Companion).encode(mutable, encoder)
            val list = ArrayList<Int>()
            list.add(10)
            ArrayList<Int>.Companion.encoder(Int.Companion).encode(list, encoder)
        }
        fun main() {}
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
