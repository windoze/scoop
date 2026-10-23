use super::*;
use scoop_identity::{
    BindableEntity as Entity, BindingNamespace as Namespace, CanonicalIdentifier,
};

#[test]
fn current_exports_follow_public_namespaces_and_keep_distinct_entity_kinds() {
    let source = r#"
        annotation class Mark
        public annotation class Visible
        public class Root {
            public class Nested
            public companion object Factory { public fun make(): Int = 1 }
        }
        public object Single
        public enum Choice { Empty }
        public typealias Alias = Root
        public fun choose(value: Int): Int = value
        public fun choose(value: String): String = value
        public val answer: Int = 42
        public val String.extra: Int get() = 1
        internal class Internal
        private fun secret(): Int = 1
        @Mark fun inspect() {}
    "#;
    let mut alias_identity = None;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let provider = queries.current_cone();
        let mut lookup = |name: &str, namespace| {
            let path = name
                .split('.')
                .map(|part| CanonicalIdentifier::new(part).unwrap())
                .collect::<Vec<_>>();
            queries
                .lookup_export(context, provider, &path, namespace)
                .unwrap()
        };
        for name in ["Internal", "Mark", "Missing"] {
            assert!(lookup(name, Namespace::Type).is_empty());
        }
        assert!(lookup("secret", Namespace::Value).is_empty());
        assert!(matches!(
            lookup("Root.Nested", Namespace::Type).as_slice(),
            [Entity::Type(_)]
        ));
        assert!(lookup("Nested", Namespace::Type).is_empty());
        assert!(matches!(
            lookup("Root.make", Namespace::Value).as_slice(),
            [Entity::Function(_)]
        ));
        assert!(matches!(
            lookup("Single", Namespace::Type).as_slice(),
            [Entity::Type(_)]
        ));
        assert!(matches!(
            lookup("Single", Namespace::Value).as_slice(),
            [Entity::ObjectValue(_)]
        ));
        assert!(matches!(
            lookup("Choice.Empty", Namespace::Value).as_slice(),
            [Entity::EnumVariant(_)]
        ));
        assert!(matches!(
            lookup("Visible", Namespace::Type).as_slice(),
            [Entity::Annotation(_)]
        ));
        assert_eq!(lookup("choose", Namespace::Value).len(), 2);
        assert!(matches!(
            lookup("answer", Namespace::Value).as_slice(),
            [Entity::Property(_)]
        ));
        assert!(matches!(
            lookup("extra", Namespace::Value).as_slice(),
            [Entity::ExtensionProperty(_)]
        ));
        let first = lookup("Alias", Namespace::Type);
        assert_eq!(lookup("Alias", Namespace::Type), first);
        let [Entity::TypeAlias(id)] = first.as_slice() else {
            panic!("actual alias declaration")
        };
        alias_identity = Some(*id);
        Ok(Vec::new())
    });
    let module = compile(source, &mut driver).unwrap();
    let (alias, _) = module
        .type_aliases
        .iter()
        .find(|(_, alias)| alias.name == "Alias")
        .unwrap();
    assert_eq!(
        alias_identity,
        Some(module.type_alias_identities[alias].id())
    );
}
