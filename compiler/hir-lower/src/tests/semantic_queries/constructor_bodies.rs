use super::*;
use std::sync::Arc;

#[test]
fn dependency_constructor_body_queries_read_only_published_initializations() {
    struct Inspect(bool);
    impl hir::MethodBodyGenerator for Inspect {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let function = queries
                .annotations()
                .targets
                .iter()
                .find_map(|entry| match entry.target {
                    hir::SourceAnnotationTarget::Callable(
                        hir::SourceAnnotationCallable::Function(id),
                    ) => Some(id),
                    _ => None,
                })
                .unwrap();
            let context = hir::SemanticContext::Function(function);
            for text in ["Box<Int>", "Value<Int>", "Plain"] {
                let ty = queries
                    .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
                    .unwrap();
                let constructors = queries.nominal_constructors(context, ty).unwrap().unwrap();
                assert!(!constructors.is_empty());
                let mut common = None;
                for constructor in constructors {
                    let CallableTemplateOrigin::Constructor(id) = constructor.identity else {
                        panic!("source constructor")
                    };
                    let reference = hir::SemanticConstructor::Imported(id);
                    let body = queries.constructor_body(reference).unwrap().unwrap();
                    if text == "Plain" {
                        assert!(matches!(body, Availability::NotPublished));
                        continue;
                    }
                    let Availability::Available(body) = body else {
                        panic!("published generic initialization")
                    };
                    let Some(Availability::Available(repeated)) =
                        queries.constructor_body(reference).unwrap()
                    else {
                        panic!("cached dependency body")
                    };
                    assert!(Arc::ptr_eq(&body, &repeated));
                    assert_eq!(
                        body.declaration.definition_origin().origin().source(),
                        &constructor.origin.source
                    );
                    assert_ne!(constructor.origin.source.cone(), queries.current_cone());
                    if text == "Box<Int>" {
                        assert_eq!(body.common.len(), 2);
                        if let Some(previous) = &common {
                            assert!(Arc::ptr_eq(previous, &body.common));
                        }
                        common = Some(Arc::clone(&body.common));
                    } else {
                        assert!(body.common.is_empty());
                    }
                    let value = serde_json::to_value(scoop_generator_protocol::BodyJson::new(
                        body.as_ref(),
                    ))
                    .unwrap();
                    assert_eq!(
                        value["origin"]["source"]["cone"],
                        constructor.origin.source.cone().to_string()
                    );
                    assert!(
                        value["inputs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|local| local["valueType"]
                                == serde_json::json!({"kind":"binder","depth":0,"index":0}))
                    );
                }
            }
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        r#"
        public annotation class Mark
        public struct Value<T>(val item: T) {
            public constructor(item: T, ignored: Int) : this(item) {}
        }
        public class Box<T> public constructor(public val item: T) {
            public var count: Int = 1
            init { this.count = this.count + 1 }
            public constructor(item: T, extra: Int) : this(item) { this.count = this.count + extra }
        }
        public class Plain public constructor(public val item: Int)
    "#,
        "@Mark fun inspect() {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
