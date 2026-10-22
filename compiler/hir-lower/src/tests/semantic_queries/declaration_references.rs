use super::*;
use hir::{AnnotationCallableV1 as Callable, AnnotationTargetV1 as Target};
use scoop_generator_protocol::AnnotationTarget;
use scoop_identity::{BindableEntity, BindingNamespace, CanonicalIdentifier, PropertyOwner};

fn resolve_target(queries: &mut dyn SemanticQueries, target: Target) -> hir::SemanticDeclaration {
    let wire = serde_json::to_value(AnnotationTarget::from(target)).unwrap();
    let reference = serde_json::from_value::<AnnotationTarget>(wire).unwrap();
    let declaration = queries
        .resolve_declaration(reference.into())
        .unwrap()
        .unwrap();
    assert_eq!(declaration.reference, target);
    assert_eq!(
        declaration.annotations,
        hir::SemanticAnnotationTarget::Imported(target)
    );
    assert_ne!(declaration.origin.source.cone(), queries.current_cone());
    declaration
}

fn check(queries: &mut dyn SemanticQueries, target: Target, origin: &hir::SemanticSourceLocation) {
    let declaration = resolve_target(queries, target);
    assert_eq!(declaration.origin.source, origin.source);
    assert_eq!(declaration.origin.span, origin.span, "{target:?}");
}

#[test]
fn dependency_declaration_references_keep_published_components_and_locations() {
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
            let provider = ConeCoordinate::new("test", "query-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
            let mut bindings = Vec::new();
            for (name, namespace) in [
                ("Mark", BindingNamespace::Type),
                ("Alias", BindingNamespace::Type),
                ("answer", BindingNamespace::Value),
                ("extra", BindingNamespace::Value),
                ("opaque", BindingNamespace::Value),
            ] {
                bindings.extend(
                    queries
                        .lookup_export(
                            context,
                            provider,
                            &[CanonicalIdentifier::new(name).unwrap()],
                            namespace,
                        )
                        .unwrap(),
                );
            }
            assert_eq!(bindings.len(), 5);
            for binding in bindings {
                let target = match binding {
                    BindableEntity::Annotation(id) => Target::Annotation(id),
                    BindableEntity::TypeAlias(id) => Target::TypeAlias(id),
                    BindableEntity::Property(id) => {
                        let property = PropertyOwner::Property(id);
                        resolve_target(
                            queries,
                            Target::PropertyStorage {
                                property,
                                role: hir::AnnotationStorageRole::Backing,
                            },
                        );
                        assert!(
                            queries
                                .resolve_declaration(
                                    Target::PropertyStorage {
                                        property,
                                        role: hir::AnnotationStorageRole::Delegate
                                    }
                                    .into()
                                )
                                .unwrap()
                                .is_none()
                        );
                        assert!(
                            queries
                                .resolve_declaration(Target::PropertyReceiver(property).into())
                                .unwrap()
                                .is_none()
                        );
                        Target::Property(id)
                    }
                    BindableEntity::ExtensionProperty(id) => {
                        resolve_target(
                            queries,
                            Target::PropertyReceiver(PropertyOwner::ExtensionProperty(id)),
                        );
                        Target::ExtensionProperty(id)
                    }
                    BindableEntity::Function(id) => Target::Callable(Callable::Declaration(
                        CallableTemplateOrigin::Function(id),
                    )),
                    _ => panic!("selected published declaration"),
                };
                let declaration = resolve_target(queries, target);
                assert!(matches!(
                    queries.describe_annotations(declaration.annotations),
                    hir::SemanticAnnotations::Imported([])
                ));
            }
            for text in ["Box<Int>", "Choice<Int>", "Row<Int>"] {
                let ty = queries
                    .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
                    .unwrap();
                let nominal = queries.nominal_declaration(ty).unwrap().unwrap();
                check(
                    queries,
                    Target::Nominal(nominal.declaration),
                    &nominal.origin,
                );
                let fields = queries.nominal_fields(context, ty).unwrap().unwrap();
                for field in fields.fields {
                    let hir::StaticFieldIdentity::Field(id) = field.identity else {
                        panic!("field")
                    };
                    check(queries, Target::Field(id), &field.origin);
                }
                for variant in fields.variants {
                    check(queries, Target::Variant(variant.identity), &variant.origin);
                    for field in variant.fields {
                        let hir::StaticFieldIdentity::VariantField(id) = field.identity else {
                            panic!("variant field")
                        };
                        check(queries, Target::VariantField(id), &field.origin);
                    }
                }
                for constructor in queries.nominal_constructors(context, ty).unwrap().unwrap() {
                    check(
                        queries,
                        Target::Callable(Callable::Declaration(constructor.identity)),
                        &constructor.origin,
                    );
                    for parameter in constructor.parameters {
                        check(
                            queries,
                            Target::Parameter {
                                callable: Callable::Declaration(parameter.identity.owner),
                                index: parameter.identity.index,
                            },
                            &parameter.origin,
                        );
                    }
                }
                for method in queries.nominal_methods(context, ty).unwrap().unwrap() {
                    check(
                        queries,
                        Target::Callable(Callable::Declaration(method.identity)),
                        &method.origin,
                    );
                    resolve_target(
                        queries,
                        Target::CallableReceiver(Callable::Declaration(method.identity)),
                    );
                    for parameter in method.parameters {
                        check(
                            queries,
                            Target::Parameter {
                                callable: Callable::Declaration(parameter.identity.owner),
                                index: parameter.identity.index,
                            },
                            &parameter.origin,
                        );
                    }
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
        public struct Box<T>(@Mark val value: T)
        public enum Choice<T> { Empty, Item(val value: T) }
        public class Row<T> public constructor(public val value: T) {
            public fun <U> echo(@Mark value: U): U = value
        }
        public typealias Alias = Box<Int>
        public var answer: Int = 42
        public val String.extra: Int get() = 1
        public fun opaque(): Int = 1
    "#,
        "@Mark fun inspect() {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
