use super::types::resolve;
use super::*;
use hir::{SemanticConstructor as Constructor, SourceAnnotationCallable as Callable};
use std::sync::Arc;

#[test]
fn constructor_body_queries_preserve_initialization_kinds_fragments_and_shared_common_steps() {
    let mut kinds = std::collections::BTreeSet::new();
    let mut identities = Vec::new();
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        for text in ["Value<Int>", "Box<Int>", "Alternate<Int>", "Empty"] {
            let ty = resolve(queries, context, text);
            let constructors = queries.nominal_constructors(context, ty).unwrap().unwrap();
            let mut common = None;
            for constructor in constructors {
                let hir::SemanticAnnotationTarget::Current(hir::SourceAnnotationTarget::Callable(
                    source,
                )) = constructor.annotations
                else {
                    panic!("source constructor")
                };
                let source = match source {
                    Callable::StructConstructor(id) => Constructor::CurrentStruct(id),
                    Callable::ClassConstructor(id) => Constructor::CurrentClass(id),
                    _ => panic!("class or struct constructor"),
                };
                let Some(Availability::Available(body)) = queries.constructor_body(source).unwrap()
                else {
                    panic!("complete constructor plan")
                };
                let Some(Availability::Available(repeated)) =
                    queries.constructor_body(source).unwrap()
                else {
                    panic!("cached plan")
                };
                assert!(Arc::ptr_eq(&body, &repeated));
                let value =
                    serde_json::to_value(scoop_generator_protocol::BodyJson::new(body.as_ref()))
                        .unwrap();
                let kind = value["initialization"]["kind"].as_str().unwrap();
                kinds.insert(kind.to_owned());
                assert_eq!(value["origin"]["source"]["path"], "src/user.scoop");
                assert_eq!(
                    value["origin"]["span"]["startByte"],
                    constructor.origin.span.start
                );
                assert_eq!(
                    value["origin"]["span"]["endByte"],
                    constructor.origin.span.end
                );
                match kind {
                    "structSecondary" => {
                        assert_eq!(
                            value["initialization"]["delegation"]["target"]["kind"],
                            "struct"
                        );
                        assert_eq!(
                            value["initialization"]["delegation"]["arguments"]["results"]
                                .as_array()
                                .unwrap()
                                .len(),
                            1
                        );
                        assert!(
                            !value["initialization"]["body"]["statements"]
                                .as_array()
                                .unwrap()
                                .is_empty()
                        );
                    }
                    "classPrimary" if text == "Box<Int>" => {
                        assert_eq!(
                            value["initialization"]["primaryStores"]
                                .as_array()
                                .unwrap()
                                .len(),
                            1
                        );
                    }
                    "classSecondaryTerminal" => {
                        assert_eq!(value["initialization"]["base"]["target"]["kind"], "class");
                        assert!(
                            !value["initialization"]["body"]["statements"]
                                .as_array()
                                .unwrap()
                                .is_empty()
                        );
                    }
                    _ => {}
                }
                if matches!(text, "Box<Int>" | "Alternate<Int>") {
                    assert_eq!(body.common.len(), 2);
                    assert_eq!(value["commonInitialization"][0]["kind"], "field");
                    assert_eq!(value["commonInitialization"][1]["kind"], "body");
                    if let Some(previous) = &common {
                        assert!(Arc::ptr_eq(previous, &body.common));
                    }
                    common = Some(Arc::clone(&body.common));
                } else {
                    assert!(body.common.is_empty());
                }
                identities.push((source, constructor.identity));
            }
        }
        Ok(Vec::new())
    });
    let module = compile(
        r#"
        annotation class Mark
        fun <T> keep(value: T): T = value
        struct Value<T>(val item: T) {
            constructor(item: T, checked: Boolean) : this(keep(item)) {
                if (checked) { keep(this.item) }
            }
        }
        class Box<T>(val item: T) {
            var count: Int = 1
            init { this.count = this.count + 1 }
            constructor(item: T, extra: Int) : this(item) { this.count = this.count + extra }
        }
        open class Base<T>(val item: T)
        class Alternate<T> : Base<T> {
            var count: Int = 1
            init { this.count = this.count + 1 }
            constructor(item: T, extra: Int) : super(keep(item)) { this.count = this.count + extra }
            constructor(item: T) : this(item, 1) {}
        }
        class Empty
        @Mark fun inspect() {}
    "#,
        &mut driver,
    )
    .unwrap();
    assert_eq!(
        kinds,
        [
            "structPrimary",
            "structSecondary",
            "classPrimary",
            "classSecondaryThis",
            "classSecondaryTerminal"
        ]
        .map(str::to_owned)
        .into_iter()
        .collect()
    );
    for (source, identity) in identities {
        let id = match source {
            Constructor::CurrentStruct(id) => module.constructor_identities[id].id(),
            Constructor::CurrentClass(id) => module.constructor_identities[id]
                .source_record()
                .unwrap()
                .id(),
            Constructor::Imported(_) => unreachable!(),
        };
        assert_eq!(
            identity,
            scoop_identity::CallableTemplateOrigin::Constructor(id)
        );
    }
}
