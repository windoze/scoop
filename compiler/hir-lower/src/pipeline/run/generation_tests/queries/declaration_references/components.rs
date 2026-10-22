use super::*;
use scoop_identity::{
    BindableEntity as Entity, BindingNamespace as Namespace, CanonicalIdentifier,
};

fn lookup(
    queries: &mut dyn SemanticQueries,
    context: SemanticContext,
    name: &str,
    namespace: Namespace,
) -> Entity {
    let values = queries
        .lookup_export(
            context,
            queries.current_cone(),
            &[CanonicalIdentifier::new(name).unwrap()],
            namespace,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    values[0]
}

#[test]
fn declaration_references_keep_empty_targets_storage_roles_and_local_parameters_distinct() {
    let source = r#"
        annotation class Mark
        public annotation class Visible
        public class Root
        public typealias Alias = Root
        public var answer: Int = 42
        public val String.extra: Int get() = 1
        @Mark fun inspect() {
            fun local(@Mark value: Int): Int = value
            local(1)
        }
    "#;
    let mut saved_alias = None;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let Entity::TypeAlias(alias) = lookup(queries, context, "Alias", Namespace::Type) else {
            panic!("alias")
        };
        let declaration = queries
            .resolve_declaration(decoded(Target::TypeAlias(alias)))
            .unwrap()
            .unwrap();
        saved_alias = Some(alias);
        assert!(matches!(
            queries.describe_annotations(declaration.annotations),
            hir::SemanticAnnotations::Current([])
        ));
        assert!(
            source[declaration.origin.span.start as usize..declaration.origin.span.end as usize]
                .contains("typealias Alias")
        );
        let Entity::Annotation(annotation) = lookup(queries, context, "Visible", Namespace::Type)
        else {
            panic!("annotation")
        };
        assert!(
            queries
                .resolve_declaration(decoded(Target::Annotation(annotation)))
                .unwrap()
                .is_some()
        );
        let Entity::Property(property) = lookup(queries, context, "answer", Namespace::Value)
        else {
            panic!("property")
        };
        let owner = PropertyOwner::Property(property);
        assert!(
            queries
                .resolve_declaration(decoded(Target::PropertyStorage {
                    property: owner,
                    role: hir::AnnotationStorageRole::Backing
                }))
                .unwrap()
                .is_some()
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::PropertyStorage {
                    property: owner,
                    role: hir::AnnotationStorageRole::Delegate
                }))
                .unwrap()
                .is_none()
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::PropertyReceiver(owner)))
                .unwrap()
                .is_none()
        );
        let Entity::ExtensionProperty(property) =
            lookup(queries, context, "extra", Namespace::Value)
        else {
            panic!("extension property")
        };
        assert!(
            queries
                .resolve_declaration(decoded(Target::ExtensionProperty(property)))
                .unwrap()
                .is_some()
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::PropertyReceiver(
                    PropertyOwner::ExtensionProperty(property)
                )))
                .unwrap()
                .is_some()
        );
        let local = queries
            .annotations()
            .targets
            .iter()
            .find_map(|entry| match entry.target {
                hir::SourceAnnotationTarget::Parameter {
                    callable: hir::SourceAnnotationCallable::Function(id),
                    index: 0,
                } => Some(id),
                _ => None,
            })
            .unwrap();
        let Some(Availability::Available(body)) = queries
            .projected_body(SemanticCallable::Current(local))
            .unwrap()
        else {
            panic!("local body")
        };
        let callable = match body.owner() {
            hir::DefaultCallableDeclarationV1::Function(id) => {
                Callable::Declaration(Declaration::Function(id))
            }
            hir::DefaultCallableDeclarationV1::GenericFunction(id) => {
                Callable::Declaration(Declaration::GenericFunction(id))
            }
            _ => panic!("named local declaration"),
        };
        let value = queries
            .resolve_declaration(decoded(Target::Parameter { callable, index: 0 }))
            .unwrap()
            .unwrap();
        assert!(
            matches!(queries.describe_annotations(value.annotations), hir::SemanticAnnotations::Current(values) if values.len() == 1)
        );
        assert!(
            source[value.origin.span.start as usize..value.origin.span.end as usize]
                .contains("value")
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::Parameter { callable, index: 1 }))
                .unwrap()
                .is_none()
        );
        let unknown = serde_json::from_value::<AnnotationTarget>(
            json!({"kind":"field","field":"ef".repeat(32)}),
        )
        .unwrap();
        assert!(
            queries
                .resolve_declaration(unknown.into())
                .unwrap()
                .is_none()
        );
        Ok(Vec::new())
    });
    let module = compile(source, &mut driver).unwrap();
    let (alias, _) = module
        .type_aliases
        .iter()
        .find(|(_, alias)| alias.name == "Alias")
        .unwrap();
    assert_eq!(saved_alias, Some(module.type_alias_identities[alias].id()));
}

#[test]
fn lexical_callable_references_exclude_capture_abi_parameters() {
    let source = r#"
        annotation class Mark
        fun inspect() {
            val closure: Int.(Int) -> Int = fun Int.(@Mark value: Int): Int { return this + value }
        }
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = queries
            .annotations()
            .targets
            .iter()
            .find_map(|entry| match entry.target {
                hir::SourceAnnotationTarget::Parameter {
                    callable: hir::SourceAnnotationCallable::Function(id),
                    index: 0,
                } => Some(id),
                _ => None,
            })
            .unwrap();
        let Some(Availability::Available(body)) = queries
            .projected_body(SemanticCallable::Current(function))
            .unwrap()
        else {
            panic!("anonymous body")
        };
        let hir::DefaultCallableDeclarationV1::Generated(id) = body.owner() else {
            panic!("anonymous identity")
        };
        let callable = Callable::Local(id);
        let value = queries
            .resolve_declaration(decoded(Target::Parameter { callable, index: 0 }))
            .unwrap()
            .unwrap();
        assert_eq!(
            &source[value.origin.span.start as usize..value.origin.span.end as usize],
            "value"
        );
        assert!(
            matches!(queries.describe_annotations(value.annotations), hir::SemanticAnnotations::Current(values) if values.len() == 1)
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::CallableReceiver(callable)))
                .unwrap()
                .is_some()
        );
        assert!(
            queries
                .resolve_declaration(decoded(Target::Parameter { callable, index: 1 }))
                .unwrap()
                .is_none()
        );
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
