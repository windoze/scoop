use super::*;
use serde_json::json;

#[test]
fn pending_method_json_keeps_owner_binders_context_and_every_parameter_calling_form() {
    let mut inspected = 0;
    let mut generator = Generator(|queries: &mut dyn SemanticQueries| {
        let mut bodies = Vec::new();
        for pending in queries.pending_methods() {
            let before = queries.signature(pending.function);
            let signature =
                scoop_generator_protocol::pending_method_signature(queries, &pending).unwrap();
            assert_eq!(signature["owner"]["kind"], "generic");
            let binders = &signature["binders"];
            assert_eq!(binders.as_array().unwrap().len(), 1);
            assert_eq!(binders[0]["name"], "T");
            assert_eq!(binders[0]["depth"], 0);
            assert_eq!(binders[0]["index"], 0);
            assert_eq!(binders[0]["declaration"]["owner"], signature["owner"]);
            assert_eq!(binders[0]["declaration"]["kind"], "nominal");
            assert_eq!(binders[0]["bounds"]["kind"], "nominal");
            assert_eq!(binders[0]["bounds"]["bounds"][0]["kind"], "interface");
            assert_eq!(signature["effects"]["execution"], "ordinary");
            assert_eq!(signature["effects"]["safety"], "safe");
            assert_eq!(signature["effects"]["gc"], "managed");
            assert_eq!(signature["effects"]["callingConvention"], "c");
            let source = match before.name.as_str() {
                "copy" => {
                    let parameters = &signature["parameters"];
                    assert_eq!(parameters[0]["name"], "first");
                    assert_eq!(parameters[0]["calling"]["kind"], "required");
                    assert_eq!(parameters[1]["name"], "second");
                    assert_eq!(parameters[1]["calling"]["kind"], "default");
                    assert_eq!(parameters[1]["calling"]["default"]["parameter"], 1);
                    assert_eq!(
                        parameters[1]["calling"]["default"]["owner"],
                        parameters[1]["declaration"]["owner"]
                    );
                    assert_eq!(
                        parameters[1]["calling"]["default"]["arguments"],
                        json!([{"kind": "binder", "depth": 0, "index": 0}])
                    );
                    assert_eq!(parameters[0]["valueType"], signature["result"]);
                    "{ return first }"
                }
                "count" | "total" => {
                    let parameter = &signature["parameters"][0];
                    assert_eq!(parameter["calling"]["kind"], "vararg");
                    assert_eq!(
                        parameter["calling"]["elementType"],
                        parameter["valueType"]["arguments"][0]
                    );
                    let omission = if before.name == "count" {
                        "emptyArray"
                    } else {
                        "default"
                    };
                    assert_eq!(parameter["calling"]["omission"]["kind"], omission);
                    "{ return 0 }"
                }
                "read" => {
                    let context = &signature["contextParameters"][0];
                    assert_eq!(
                        context["label"],
                        json!({"kind": "named", "name": "current"})
                    );
                    assert_eq!(context["valueType"]["arguments"][0], signature["result"]);
                    assert_eq!(context["declaration"]["owner"], signature["method"]);
                    "{ return current.value }"
                }
                _ => panic!("only the actual interface methods are pending"),
            };
            assert!(matches!(
                queries.body(SemanticCallable::Current(pending.function)),
                Some(Availability::PendingGeneration)
            ));
            assert_eq!(queries.signature(pending.function).result, before.result);
            inspected += 1;
            bodies.push(generated(queries, pending.function, source));
        }
        Ok(bodies)
    });
    compile(
        r#"
        public interface Marker {}
        public class Payload<T>(public val value: T)
        public interface Copies<T> {
            public fun copy(first: T, second: T = first): T
            public fun count(vararg items: Int): Int
            public fun total(vararg items: Int = [1]): Int
            context(current: Payload<T>) public fun read(): T
        }
        class Holder<T: Marker> : Copies<T>
        fun <T: Marker> call(value: Holder<T>, item: T): T {
            value.count()
            value.total()
            return value.copy(item)
        }
    "#,
        &mut generator,
    )
    .unwrap();
    assert_eq!(inspected, 4);
}

#[test]
fn generated_methods_reject_conflicting_defaults_before_calling_the_generator() {
    let mut generator = Generator(|_: &mut dyn SemanticQueries| {
        panic!("conflicting parameter defaults are diagnosed before generation")
    });
    let diagnostics = compile(
        "public interface Left { public fun choose(value: Int = 1): Int }\n\
         public interface Right { public fun choose(value: Int = 2): Int }\n\
         class Both : Left, Right",
        &mut generator,
    )
    .unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("inherits conflicting default expressions for parameter `value`")
    }));
}

#[test]
fn inherited_context_signature_keeps_the_synthesized_methods_actual_source_file() {
    let mut inspected = false;
    let mut generator = Generator(|queries: &mut dyn SemanticQueries| {
        let pending = queries.pending_methods()[0].clone();
        let signature = queries.signature(pending.function);
        assert_eq!(signature.context_parameters[0].span, signature.origin.span);
        let json = scoop_generator_protocol::pending_method_signature(queries, &pending).unwrap();
        let context = &json["contextParameters"][0];
        assert_eq!(context["origin"], json["origin"]);
        assert_eq!(context["origin"]["source"]["path"], "src/model.scoop");
        inspected = true;
        Ok(vec![generated(
            queries,
            pending.function,
            "{ return input.value }",
        )])
    });
    compile_files(
        &[
            ("src/contracts.scoop", "class Payload<T>(val value: T)\ninterface Reader<T> { context(input: Payload<T>) fun read(): T }"),
            ("src/model.scoop", "class Model<T> : Reader<T>"),
        ],
        &mut generator,
    ).unwrap();
    assert!(inspected);
}
