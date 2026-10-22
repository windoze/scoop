use super::*;
use scoop_generator_protocol::{QueryJson, TypeReference};
use serde_json::{Value, json};

fn input(value: Value) -> scoop_identity::DecodedSignatureTypeKey {
    serde_json::from_value::<TypeReference>(value).unwrap().0
}

fn encoded(queries: &dyn SemanticQueries, context: SemanticContext, ty: hir::TypeId) -> Value {
    serde_json::to_value(QueryJson(
        &queries.type_signature(context.into(), ty).unwrap(),
    ))
    .unwrap()
}

#[test]
fn complete_type_references_round_trip_real_source_types_and_scoped_parameters() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let span = queries.signature(function).origin.span;
        for text in [
            "Unit",
            "Any",
            "Int",
            "Boolean",
            "String",
            "Char",
            "Float",
            "Nothing",
            "(Int,)",
            "(Int, String)",
            "suspend Int.(String) -> Boolean",
            "Ptr<Int>",
            "Ptr<T>",
            "FunPtr<(Int) -> Boolean>",
            "Box<String>",
            "Wrapped<Int>",
            "Host<Int>.Companion",
            "T",
            "U",
            "U.Companion",
            "Box<T>",
            "(T, U)",
            "T.(U) -> T",
        ] {
            let ty = resolve(queries, context, text);
            let reference = encoded(queries, context, ty);
            let actual = queries
                .resolve_type_reference(context.into(), input(reference), span)
                .unwrap();
            assert_eq!(actual, ty, "{text}");
        }
        let ty = resolve(queries, context, "Box<Int>");
        let fields = queries.nominal_fields(context, ty).unwrap().unwrap();
        let nominal = queries.nominal_declaration(ty).unwrap().unwrap();
        let binders = nominal.declaration_binders();
        let field = fields.fields[0].value_type.declaration;
        let key = queries.type_signature((&binders).into(), field).unwrap();
        let reference = serde_json::to_value(QueryJson(&key)).unwrap();
        assert_eq!(
            queries
                .resolve_type_reference((&binders).into(), input(reference), span)
                .unwrap(),
            field
        );
        let caller = resolve(queries, context, "U");
        assert_ne!(field, caller);
        Ok(Vec::new())
    });
    compile(
        r#"
        annotation class Mark
        struct Box<T>(val value: T)
        typealias Wrapped<T> = Box<T>
        class Host<T> { public companion object {} }
        class Scope<T: value> {
            @Mark fun <U: ref> inspect(first: T, second: U) where U.Companion : ref {}
        }
    "#,
        &mut driver,
    )
    .unwrap();
}

#[test]
fn invalid_type_references_return_query_diagnostics_without_poisoning_compilation() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let location = queries.signature(function).origin;
        let integer_ty = resolve(queries, context, "Int");
        let integer = encoded(queries, context, integer_ty);
        let string_ty = resolve(queries, context, "String");
        let string = encoded(queries, context, string_ty);
        let mut template = |text: &str| {
            let ty = resolve(queries, context, text);
            encoded(queries, context, ty)
        };
        let boxed_string = template("Box<String>");
        let boxed = template("Box<Int>");
        let mut value = template("Value<Int>");
        value["arguments"] = json!([string.clone()]);
        let mut upper = template("Bounded<String, StringView>");
        upper["arguments"] = json!([integer.clone(), template("StringView")]);
        let mut no_gc = template("Native<Int>");
        no_gc["arguments"] = json!([string.clone()]);
        let mut c_layout = template("Layout<Int>");
        c_layout["arguments"] = json!([string.clone()]);
        let mut wrong_arity = boxed.clone();
        wrong_arity["arguments"] = json!([integer.clone(), integer.clone()]);
        let mut native = template("FunPtr<(Int) -> Boolean>");
        let mut managed_native = native.clone();
        managed_native["parameters"] = json!([string.clone()]);
        native["parameters"] = json!([{"kind": "binder", "depth": 0, "index": 0}]);
        let function =
            json!({"kind": "function", "effect": "suspend", "parameters": [], "result": integer});
        let pointer = resolve(queries, context, "FunPtr<(Int) -> Boolean>");
        let declaration = match queries
            .nominal_declaration(pointer)
            .unwrap()
            .unwrap()
            .declaration
        {
            hir::SourceNominalId::GenericTemplate(id) => id.to_string(),
            _ => panic!("generic FunPtr declaration"),
        };
        for (reference, expected) in [
            (
                json!({"kind": "nominal", "declaration": "ab".repeat(32)}),
                "unknown nominal type reference",
            ),
            (
                json!({"kind": "nominal", "declaration": boxed["declaration"]}),
                "unknown nominal type reference",
            ),
            (
                json!({"kind": "nominalApplication", "declaration": integer["declaration"], "arguments": [integer.clone()]}),
                "unknown generic nominal reference",
            ),
            (wrong_arity, "takes 1 type argument(s)"),
            (value, "must satisfy `value`"),
            (upper, "upper bound `View<Int>`"),
            (no_gc, "`@NoGC` struct specialization"),
            (c_layout, "concrete `@CLayout` type is not C-FFI-safe"),
            (
                json!({"kind": "rawPointer", "pointee": string}),
                "must satisfy `value`",
            ),
            (
                json!({"kind": "tuple", "elements": [{"kind": "rawPointer", "pointee": boxed_string}]}),
                "`Ptr` pointee must be GC-free",
            ),
            (
                json!({"kind": "binder", "depth": 1, "index": 0}),
                "does not exist in the query context",
            ),
            (
                json!({"kind": "binder", "depth": 0, "index": 9}),
                "does not exist in the query context",
            ),
            (
                json!({"kind": "companion", "host": {"kind": "binder", "depth": 0, "index": 0}}),
                "does not require a public companion",
            ),
            (
                json!({"kind": "companion", "host": boxed}),
                "has no public companion",
            ),
            (native, "ordinary concrete function type"),
            (managed_native, "`FunPtr` signature is not C-FFI-safe"),
            (
                json!({"kind": "nominalApplication", "declaration": declaration, "arguments": [function]}),
                "ordinary concrete function type",
            ),
        ] {
            let diagnostics = queries
                .resolve_type_reference(context.into(), input(reference), location.span)
                .unwrap_err();
            assert!(
                diagnostics.iter().any(|d| d.message.contains(expected)),
                "{expected}: {diagnostics:?}"
            );
            assert!(
                diagnostics.iter().all(|d| d.span == Some(location.span)
                    && d.source.as_deref() == Some(&location.source))
            );
            assert_eq!(resolve(queries, context, "Int"), integer_ty);
        }
        let diagnostics = queries
            .resolve_type(
                context,
                &scoop_parser::parse_type("Ptr<Box<String>>").unwrap(),
            )
            .unwrap_err();
        assert!(
            diagnostics[0]
                .message
                .contains("`Ptr` pointee must be GC-free")
        );
        let diagnostics = queries
            .resolve_type(
                context,
                &scoop_parser::parse_type("FunPtr<(String) -> Int>").unwrap(),
            )
            .unwrap_err();
        assert!(
            diagnostics[0]
                .message
                .contains("`FunPtr` signature is not C-FFI-safe")
        );
        Ok(Vec::new())
    });
    compile(
        r#"
        annotation class Mark
        struct Box<T>(val value: T)
        struct Value<T: value>(val value: T)
        interface View<T> {}
        class StringView : View<String>
        struct Bounded<T, U: View<T>>(val first: T, val second: U)
        @NoGC struct Native<T>(val value: T)
        @CLayout struct Layout<T>(val value: T)
        @Mark fun <T: value> inspect(value: T) {}
    "#,
        &mut driver,
    )
    .unwrap();
}

#[test]
fn nominal_lookup_does_not_depend_on_previously_returned_references_or_applications() {
    const SOURCE: &str =
        "annotation class Mark\nstruct Box<T>(val value: T)\n@Mark fun inspect() {}";
    let mut saved = None;
    let mut capture = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let ty = resolve(queries, context, "Box<Int>");
        saved = Some(encoded(queries, context, ty));
        Ok(Vec::new())
    });
    compile(SOURCE, &mut capture).unwrap();
    let reference = input(saved.unwrap());
    let mut consume = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let span = queries.signature(function).origin.span;
        let first = queries
            .resolve_type_reference(context.into(), reference.clone(), span)
            .unwrap();
        assert_eq!(first, resolve(queries, context, "Box<Int>"));
        assert_eq!(
            queries
                .resolve_type_reference(context.into(), reference.clone(), span)
                .unwrap(),
            first
        );
        Ok(Vec::new())
    });
    compile(SOURCE, &mut consume).unwrap();
}
