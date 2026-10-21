use super::*;
use scoop_identity::{
    BindableEntity as Entity, BindingNamespace as Namespace, CanonicalIdentifier,
};

#[test]
fn exports_keep_provider_scope_and_current_reexports_actual_targets() {
    const PROVIDER: &str = r#"
        public annotation class Mark
        public class Box<T> public constructor(public val value: T) {
            public class Nested
        }
        public fun choose(value: Int): Int = value
        private class Hidden
    "#;
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
                        hir::SourceAnnotationCallable::Function(id),
                    ) => Some(id),
                    _ => None,
                })
                .unwrap();
            let context = hir::SemanticContext::Function(function);
            let current = queries.current_cone();
            let provider = ConeCoordinate::new("test", "query-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
            let mut lookup = |cone, name: &str, namespace| {
                let path = name
                    .split('.')
                    .map(|part| CanonicalIdentifier::new(part).unwrap())
                    .collect::<Vec<_>>();
                queries
                    .lookup_export(context, cone, &path, namespace)
                    .unwrap()
            };
            let original = lookup(provider, "Box", Namespace::Type);
            assert!(matches!(original.as_slice(), [Entity::GenericType(_)]));
            assert_eq!(lookup(current, "PublicBox", Namespace::Type), original);
            assert_eq!(
                lookup(current, "PublicBox.Nested", Namespace::Type),
                lookup(provider, "Box.Nested", Namespace::Type)
            );
            assert_eq!(
                lookup(current, "forwarded", Namespace::Value),
                lookup(provider, "choose", Namespace::Value)
            );
            assert!(lookup(current, "Box", Namespace::Type).is_empty());
            assert!(lookup(provider, "PublicBox", Namespace::Type).is_empty());
            assert!(lookup(provider, "Hidden", Namespace::Type).is_empty());
            assert!(lookup(current, "Mark", Namespace::Type).is_empty());
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        PROVIDER,
        "public import Box as PublicBox\npublic import choose as forwarded\n@Mark fun inspect() {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
