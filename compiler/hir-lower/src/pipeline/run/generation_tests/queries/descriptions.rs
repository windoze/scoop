use super::types::resolve;
use super::*;
use scoop_generator_protocol::{member_description, type_description};
use serde_json::{Value, json};

mod binders;

fn named<'a>(values: &'a Value, name: &str) -> &'a Value {
    values
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["name"] == name)
        .unwrap()
}

#[test]
fn member_json_preserves_storage_constructor_links_and_property_capabilities() {
    let source = r#"
        annotation class Label(val text: String)
        struct Pair<T>(@field:Label("值") val first: T, val second: T = first)
        enum Choice<T> { Empty, Item(val value: T), Pair(T, Int) }
        class Box<T>(val value: T) {
            var mirror: T get() = value
                private set(next) {}
            private fun hidden(): Int = 1
        }
        interface Reader<T> {
            fun read(): T
            fun answer(): Int = 42
        }
        @Label("inspect") fun inspect() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let ty = resolve(queries, context, "Pair<String>");
        let pair = member_description(queries, function, ty).unwrap();
        let first = &pair["fields"][0];
        assert_eq!(first["name"], "first");
        assert_eq!(first["storage"]["kind"], "declared");
        assert_eq!(
            first["annotations"][0]["arguments"][0]["value"]["value"],
            "值"
        );
        let parameters = &pair["constructors"][0]["parameters"];
        assert_eq!(parameters[0]["target"]["field"], first["declaration"]);
        assert_eq!(parameters[0]["valueType"], first["valueType"]);
        let default = &parameters[1]["calling"]["default"];
        assert_eq!(default["owner"], pair["constructors"][0]["declaration"]);
        assert_eq!(default["parameter"], 1);
        assert_eq!(
            default["declarationArguments"][0],
            json!({"kind": "binder", "depth": 0, "index": 0})
        );
        assert_eq!(default["arguments"][0], first["valueType"]["appliedType"]);
        assert_eq!(member_description(queries, function, ty).unwrap(), pair);

        let ty = resolve(queries, context, "Choice<String>");
        let choice = member_description(queries, function, ty).unwrap();
        assert_eq!(choice["variants"][0]["style"], "unit");
        assert_eq!(choice["variants"][2]["style"], "positional");
        let item = &choice["variants"][1];
        let constructor = choice["constructors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|constructor| constructor["constructorKind"]["variant"] == item["declaration"])
            .unwrap();
        assert_eq!(
            constructor["parameters"][0]["target"]["field"],
            item["fields"][0]["declaration"]
        );

        let ty = resolve(queries, context, "Box<String>");
        let class = member_description(queries, function, ty).unwrap();
        let mirror = named(&class["properties"], "mirror");
        assert_eq!(mirror["getter"]["access"]["accessible"], true);
        assert_eq!(mirror["setter"]["access"]["accessible"], false);
        assert_eq!(mirror["setter"]["access"]["visibility"], "private");
        assert_eq!(mirror["getter"]["body"]["availability"], "available");
        assert!(named(&class["properties"], "value")["setter"].is_null());
        assert_eq!(
            named(&class["methods"], "hidden")["access"]["accessible"],
            false
        );

        let ty = resolve(queries, context, "Reader<String>");
        let reader = member_description(queries, function, ty).unwrap();
        assert_eq!(
            named(&reader["methods"], "read")["body"]["availability"],
            "noBody"
        );
        assert_eq!(
            named(&reader["methods"], "answer")["body"]["availability"],
            "available"
        );
        let tuple = resolve(queries, context, "(Int, String)");
        assert!(
            member_description(queries, function, tuple)
                .unwrap()
                .is_null()
        );
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}

#[test]
fn type_json_retains_structure_nominal_origins_and_core_member_declarations() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let mut describe = |text| {
            let ty = resolve(queries, context, text);
            type_description(queries, function, ty).unwrap()
        };
        for (text, kind) in [
            ("Unit", "unit"),
            ("Any", "any"),
            ("(T, Int)", "tuple"),
            ("(T) -> Int", "function"),
            ("Ptr<Int>", "pointer"),
            ("FunPtr<(Int) -> Boolean>", "nativeFunctionPointer"),
            ("T", "parameter"),
        ] {
            let description = describe(text);
            assert_eq!(description["shape"]["kind"], kind);
        }
        let nominal = describe("Box<T>");
        assert_eq!(nominal["shape"]["nominalKind"]["kind"], "struct");
        assert_eq!(
            nominal["shape"]["origin"]["source"]["path"],
            "src/types.scoop"
        );
        assert_eq!(
            nominal["shape"]["parameters"][0]["origin"]["source"]["path"],
            "src/types.scoop"
        );
        assert_eq!(
            nominal["shape"]["arguments"][0],
            json!({"kind": "binder", "depth": 0, "index": 0})
        );
        for text in ["Unit", "Any", "Int", "Ptr<Int>", "FunPtr<(Int) -> Boolean>"] {
            let ty = resolve(queries, context, text);
            let nominal = queries.nominal_declaration(ty).unwrap().unwrap();
            assert!(matches!(nominal.kind, hir::StaticNominalKind::Intrinsic(_)));
            assert_eq!(
                nominal.origin.source.cone(),
                scoop_identity::ConeIdentity::CORE
            );
            let members = member_description(queries, function, ty).unwrap();
            assert!(!members.is_null(), "core declaration: {text}");
        }
        Ok(Vec::new())
    });
    compile_files(
        &[
            ("src/types.scoop", "struct Box<T>(val value: T)"),
            (
                "src/client.scoop",
                "annotation class Mark\n@Mark fun <T> inspect(value: T) {}",
            ),
        ],
        &mut driver,
    )
    .unwrap();
}
