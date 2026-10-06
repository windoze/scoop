use super::*;

#[test]
fn tuple_encoding_uses_normal_members_bounds_and_nested_container_combinations() {
    let output = lower_with_sysroot(
        r#"
        struct Record(val value: (Int, String)) : Encodable
        fun <T : Encodable> send(value: T, encoder: Encoder) { value.encode(encoder) }
        fun use(encoder: Encoder) {
            val value = (7, "tuple", Unit)
            value.encode(encoder)
            val reference = value::encode
            reference(encoder)
            send(value, encoder)
            val boxed: Encodable = value
            boxed.encode(encoder)
            val nested = (value, Array<Option<Int>>(1L) { _ -> Some(8) })
            nested.encode(encoder)
            Record((9, "field")).encode(encoder)
        }
        fun main() {}
    "#,
    )
    .unwrap();
    assert!(!output.local.tuple_interface_implementations.is_empty());
    for implementation in output.local.tuple_interface_implementations.values() {
        assert_eq!(
            output.local.interfaces[implementation.interface].name,
            "Encodable"
        );
        let hir::concrete::InterfaceImplementationTarget::Method(function) =
            implementation.methods[0].target
        else {
            panic!("tuple encoding is a normal method")
        };
        assert!(matches!(
            output.local.functions[function].kind,
            hir::concrete::FunctionKind::User(_)
        ));
    }
}

#[test]
fn tuple_erasure_materializes_only_applicable_interface_tables() {
    let output = lower_with_sysroot(
        r#"
        private struct Opaque()
        private struct LocalOnly()
        fun <T> erase(value: T): Any = value
        fun main() {
            val good = erase((1, "value"))
            val bad = erase((2, Opaque()))
            val nested = erase(((3, "nested"), Unit))
            val local = LocalOnly()
            if (good is Encodable && bad !is Encodable && nested is Encodable) {}
        }
    "#,
    )
    .unwrap();
    let mut good = 0;
    let mut bad = 0;
    for (ty, value) in output.local.types.iter() {
        let hir::concrete::TypeKind::Tuple(elements) = &value.kind else {
            continue;
        };
        let opaque = elements.iter().any(|element| {
            matches!(output.local.types[*element].kind,
            hir::concrete::TypeKind::Struct(id) if output.local.structs[id].name == "Opaque")
        });
        if opaque {
            assert!(
                !output
                    .local
                    .tuple_interface_implementations
                    .contains_key(&ty)
            );
            bad += 1;
        } else if output
            .local
            .tuple_interface_implementations
            .contains_key(&ty)
        {
            good += 1;
        }
    }
    assert!(good >= 2 && bad >= 1);
    let shared = hir::CanonicalSourceNominalIdsV1::from_export_hir(&output.export).unwrap();
    for (id, definition) in output.export.structs.iter() {
        if matches!(definition.name.as_str(), "Opaque" | "LocalOnly") {
            let owner = output.export.nominal_identities[id]
                .concrete_type_id()
                .unwrap();
            assert_eq!(
                shared
                    .values()
                    .contains(&hir::SourceNominalId::Concrete(owner)),
                definition.name == "Opaque"
            );
        }
    }
}

#[test]
fn tuple_encoding_rejects_unconstrained_and_non_encodable_elements() {
    for source in [
        "fun <T> send(value: (Int, T), e: Encoder) { value.encode(e) }",
        "fun send(value: (Int, Any), e: Encoder) { value.encode(e) }",
        "fun send(value: (Int, () -> Unit), e: Encoder) { value.encode(e) }",
    ] {
        let errors = lower_with_sysroot(&format!("{source}\nfun main() {{}}")).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains("encode")),
            "{errors:?}"
        );
    }
}
