use super::types::resolve;
use super::*;
use hir::{AnnotationCallableV1 as Callable, AnnotationTargetV1 as Target};
use scoop_generator_protocol::AnnotationTarget;
use scoop_identity::{CallableTemplateOrigin as Declaration, PropertyOwner};
use serde_json::json;

mod components;

fn decoded(target: Target) -> hir::DecodedAnnotationTargetV1 {
    let value = serde_json::to_value(AnnotationTarget::from(target)).unwrap();
    serde_json::from_value::<AnnotationTarget>(value)
        .unwrap()
        .into()
}

fn check(
    queries: &mut dyn SemanticQueries,
    target: Target,
    annotations: hir::SemanticAnnotationTarget,
    origin: &hir::SemanticSourceLocation,
) {
    let declaration = queries
        .resolve_declaration(decoded(target))
        .unwrap()
        .unwrap();
    assert_eq!(declaration.reference, target);
    assert_eq!(declaration.annotations, annotations);
    assert_eq!(declaration.origin.source, origin.source);
    assert_eq!(declaration.origin.span, origin.span, "{target:?}");
}

fn parameter(queries: &mut dyn SemanticQueries, parameter: &hir::SemanticParameterDescription) {
    check(
        queries,
        Target::Parameter {
            callable: Callable::Declaration(parameter.identity.owner),
            index: parameter.identity.index,
        },
        parameter.annotations,
        &parameter.origin,
    );
}

#[test]
fn declaration_references_round_trip_members_with_their_actual_origins() {
    let mut checked = 0;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        for text in [
            "Box<String>",
            "Pair<Int>",
            "Choice<Int>",
            "Reader<Int>",
            "Values",
            "Int",
        ] {
            let ty = resolve(queries, context, text);
            let nominal = queries.nominal_declaration(ty).unwrap().unwrap();
            check(
                queries,
                Target::Nominal(nominal.declaration),
                nominal.annotations,
                &nominal.origin,
            );
            let Some(fields) = queries.nominal_fields(context, ty).unwrap() else {
                assert_eq!(text, "Int");
                continue;
            };
            for field in fields.fields {
                let hir::StaticFieldIdentity::Field(id) = field.identity else {
                    panic!("nominal field")
                };
                check(queries, Target::Field(id), field.annotations, &field.origin);
                checked += 1;
            }
            for variant in fields.variants {
                check(
                    queries,
                    Target::Variant(variant.identity),
                    variant.annotations,
                    &variant.origin,
                );
                for field in variant.fields {
                    let hir::StaticFieldIdentity::VariantField(id) = field.identity else {
                        panic!("variant field")
                    };
                    check(
                        queries,
                        Target::VariantField(id),
                        field.annotations,
                        &field.origin,
                    );
                    checked += 1;
                }
            }
            for constructor in queries.nominal_constructors(context, ty).unwrap().unwrap() {
                check(
                    queries,
                    Target::Callable(Callable::Declaration(constructor.identity)),
                    constructor.annotations,
                    &constructor.origin,
                );
                for value in &constructor.parameters {
                    parameter(queries, value);
                }
            }
            for method in queries.nominal_methods(context, ty).unwrap().unwrap() {
                check(
                    queries,
                    Target::Callable(Callable::Declaration(method.identity)),
                    method.annotations,
                    &method.origin,
                );
                let receiver = queries
                    .resolve_declaration(decoded(Target::CallableReceiver(Callable::Declaration(
                        method.identity,
                    ))))
                    .unwrap()
                    .unwrap();
                assert_eq!(receiver.origin.span, method.origin.span);
                for value in &method.parameters {
                    parameter(queries, value);
                }
            }
            for property in queries.nominal_properties(context, ty).unwrap().unwrap() {
                check(
                    queries,
                    Target::Property(property.identity),
                    property.annotations,
                    &property.origin,
                );
                let owner = PropertyOwner::Property(property.identity);
                assert!(
                    queries
                        .resolve_declaration(decoded(Target::PropertyReceiver(owner)))
                        .unwrap()
                        .is_some()
                );
                for accessor in std::iter::once(&property.getter).chain(property.setter.as_ref()) {
                    check(
                        queries,
                        Target::Callable(Callable::Declaration(Declaration::Accessor(
                            accessor.identity,
                        ))),
                        accessor.annotations,
                        &accessor.origin,
                    );
                }
                for role in [
                    hir::AnnotationStorageRole::Backing,
                    hir::AnnotationStorageRole::Delegate,
                ] {
                    assert!(
                        queries
                            .resolve_declaration(decoded(Target::PropertyStorage {
                                property: owner,
                                role
                            }))
                            .unwrap()
                            .is_none()
                    );
                }
            }
        }
        let id = match queries.method_identity(function).unwrap() {
            hir::HirSourceFunctionIdentity::Plain(record) => Declaration::Function(record.id()),
            _ => panic!("plain inspect function"),
        };
        assert!(
            queries
                .resolve_declaration(decoded(Target::Parameter {
                    callable: Callable::Declaration(id),
                    index: 0
                }))
                .unwrap()
                .is_none()
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::CallableReceiver(Callable::Declaration(id))))
                .unwrap()
                .is_none()
        );
        let malformed_kind = serde_json::from_value::<AnnotationTarget>(json!({"kind":"callable","callable":{"kind":"genericFunction","id":match id {Declaration::Function(id)=>id.to_string(),_=>unreachable!()}}})).unwrap();
        assert!(
            queries
                .resolve_declaration(malformed_kind.into())
                .unwrap()
                .is_none()
        );
        Ok(Vec::new())
    });
    compile(
        r#"
        annotation class Mark
        annotation class Label(val text: String)
        struct Pair<T>(@field:Label("field") val first: T, val second: T = first)
        enum Choice<T> { Empty, Item(val value: T), Pair(T, Int) }
        class Box<T>(val value: T) {
            @get:Label("getter") @setparam:Label("setter parameter")
            var mirror: T get() = value
                private set(next) {}
            private fun <U> hidden(value: U): U = value
        }
        interface Reader<T> { fun read(value: T): T }
        object Values { val value: Int = 1 }
        @Mark fun inspect() {}
    "#,
        &mut driver,
    )
    .unwrap();
    assert!(checked >= 6);
}
