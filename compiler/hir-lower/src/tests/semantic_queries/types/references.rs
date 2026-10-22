use super::*;
use scoop_generator_protocol::{QueryJson, TypeReference};
use scoop_identity::{BindableEntity, BindingNamespace, CanonicalIdentifier, ConeIdentity};
use serde_json::json;

#[test]
fn dependency_type_references_resolve_unloaded_declarations_and_keep_core_representations() {
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
                .find_map(|target| match target.target {
                    hir::SourceAnnotationTarget::Callable(
                        hir::SourceAnnotationCallable::Function(function),
                    ) => Some(function),
                    _ => None,
                })
                .unwrap();
            let context = hir::SemanticContext::Function(function);
            let span = queries.signature(function).origin.span;
            let provider = ConeCoordinate::new("test", "query-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
            let mut lookup = |provider, name| {
                let path = [CanonicalIdentifier::new(name).unwrap()];
                queries
                    .lookup_export(context, provider, &path, BindingNamespace::Type)
                    .unwrap()
            };
            let box_id = match lookup(provider, "Box").as_slice() {
                [BindableEntity::GenericType(id)] => *id,
                _ => panic!("actual exported Box declaration"),
            };
            let integer = match lookup(ConeIdentity::CORE, "Int").as_slice() {
                [BindableEntity::Type(id)] => *id,
                _ => panic!("actual core integer declaration"),
            };
            let reference = json!({
                "kind": "nominalApplication", "declaration": box_id.to_string(),
                "arguments": [{"kind": "nominal", "declaration": integer.to_string()}],
            });
            let reference: TypeReference = serde_json::from_value(reference).unwrap();
            let original = queries
                .resolve_type_reference(context.into(), reference.0, span)
                .unwrap();
            let resolved = queries
                .resolve_type(context, &scoop_parser::parse_type("Box<Int>").unwrap())
                .unwrap();
            assert_eq!(original, resolved);

            for text in [
                "Unit",
                "Any",
                "Int",
                "Boolean",
                "String",
                "Char",
                "Float",
                "Nothing",
                "Ptr<Int>",
                "FunPtr<(Int) -> Boolean>",
                "Box<String>",
                "Box<Int>.Companion",
                "Bounded<Int, IntView>",
                "(Box<T>, T.(Int) -> T)",
            ] {
                let ty = queries
                    .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
                    .unwrap();
                let key = queries.type_signature(context.into(), ty).unwrap();
                let value = serde_json::to_value(QueryJson(&key)).unwrap();
                let reference: TypeReference = serde_json::from_value(value).unwrap();
                assert_eq!(
                    queries
                        .resolve_type_reference(context.into(), reference.0, span)
                        .unwrap(),
                    ty,
                    "{text}"
                );
            }

            let bounded = queries
                .resolve_type(
                    context,
                    &scoop_parser::parse_type("Bounded<Int, IntView>").unwrap(),
                )
                .unwrap();
            let key = queries.type_signature(context.into(), bounded).unwrap();
            let mut value = serde_json::to_value(QueryJson(&key)).unwrap();
            let string = queries
                .resolve_type(context, &scoop_parser::parse_type("String").unwrap())
                .unwrap();
            value["arguments"][0] = serde_json::to_value(QueryJson(
                &queries.type_signature(context.into(), string).unwrap(),
            ))
            .unwrap();
            let reference: TypeReference = serde_json::from_value(value).unwrap();
            let diagnostics = queries
                .resolve_type_reference(context.into(), reference.0, span)
                .unwrap_err();
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("upper bound `View<String>`")),
                "{diagnostics:?}"
            );

            let ty = queries
                .resolve_type(context, &scoop_parser::parse_type("Box<Int>").unwrap())
                .unwrap();
            let members = queries.nominal_properties(context, ty).unwrap().unwrap();
            let nominal = queries.nominal_declaration(ty).unwrap().unwrap();
            let binders = nominal.declaration_binders();
            let member = members[0].value_type.declaration;
            let key = queries.type_signature((&binders).into(), member).unwrap();
            let reference: TypeReference =
                serde_json::from_value(serde_json::to_value(QueryJson(&key)).unwrap()).unwrap();
            assert_eq!(
                queries
                    .resolve_type_reference((&binders).into(), reference.0, span)
                    .unwrap(),
                member
            );
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        r#"
        public annotation class Mark
        public class Box<T> public constructor(public val value: T) {
            public companion object {}
        }
        public interface View<T> {}
        public class IntView : View<Int>
        public struct Bounded<T, U: View<T>>(val first: T, val second: U)
    "#,
        "@Mark fun <T: value> inspect(value: T) {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
