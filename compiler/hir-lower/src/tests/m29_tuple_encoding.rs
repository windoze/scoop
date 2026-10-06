use super::*;

#[test]
fn tuple_codecs_use_ordinary_function_values_and_interface_calls() {
    lower_with_sysroot(
        r#"
        fun use(encoder: Encoder) {
            val codec = EncodeFunction<(Int, String, Unit)>({ value, sink ->
                val fields = sink.unkeyed()
                Int.Companion.encode(value._1, fields.element())
                String.Companion.encode(value._2, fields.element())
                UnitEncoder.encode(value._3, fields.element())
                fields.end()
            })
            val value = (7, "tuple", Unit)
            codec.encode(value, encoder)
            val reference = codec::encode
            reference(value, encoder)
            val erased: Encodable<(Int, String, Unit)> = codec
            erased.encode(value, encoder)
            val array = Array<(Int, String, Unit)>(1L) { _ -> value }
            Array<(Int, String, Unit)>.Companion.encoder(codec).encode(array, encoder)
        }
        fun main() {}
    "#,
    )
    .unwrap();
}

#[test]
fn tuple_erasure_retains_private_element_types_without_codec_conformance() {
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
            if (good is Encodable<(Int, String)> || bad is Encodable<(Int, Opaque)>) {}
        }
    "#,
    )
    .unwrap();
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
fn tuple_values_do_not_gain_implicit_encoding_members() {
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
