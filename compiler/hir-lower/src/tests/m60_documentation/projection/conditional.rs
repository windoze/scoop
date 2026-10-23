use super::*;

const PROVIDER: &str = r#"
    package api
    public interface Read {
        public fun read(): Int
        public val size: Int
    }
    public interface DefaultRead { public fun defaultRead(): Int = 2 }
    public interface Mark {}
    public interface Project<C> { public fun projected(): C }
    public struct Plain()
    public struct Wrapper<T>() {
        public companion object {
            override interface Mark where T : Hash {}
        }
    }
    public open class Base<T> public constructor() {
        override interface Read where T : Hash {
            public override fun read(): Int = 1
            public override val size: Int get() = 1
        }
    }
    public class Defaults<T> {
        override interface DefaultRead where T : Hash {}
    }
    public class Choice<T> {
        override interface DefaultRead where T : value {}
        override interface DefaultRead where T : ref {}
    }
    public open class Projected<T> public constructor() {
        override interface Project<T.Companion> where T.Companion : Mark {
            public override fun projected(): T.Companion = T.Companion
        }
    }
"#;

const CONSUMER: &str = r#"
    import api.*
    public class Child<U> : Base<U>()
    public class Known : Base<Int>()
    public class Absent : Base<Plain>()
    public class Constructed<U> : Projected<Wrapper<U>>()
    public class NoProjection : Projected<Plain>()
"#;

fn check(input: &ConeInput) {
    let child = item(input, ApiKind::Class, "Child");
    for signature in [
        "fun read(): Int where U : Hash",
        "val size: Int where U : Hash",
    ] {
        assert!(
            child
                .relations
                .iter()
                .any(|relation| relation.signature == signature),
            "{:?}",
            child.relations
        );
    }
    let known = item(input, ApiKind::Class, "Known");
    assert!(
        known
            .relations
            .iter()
            .any(|relation| relation.signature == "val size: Int where Int : Hash"),
        "{:?}",
        known.relations
    );
    assert!(
        known
            .relations
            .iter()
            .any(|relation| relation.signature == "fun read(): Int where Int : Hash"),
        "{:?}",
        known.relations
    );
    for name in ["Absent", "NoProjection"] {
        let declaration = item(input, ApiKind::Class, name);
        assert!(
            !declaration
                .relations
                .iter()
                .any(|relation| relation.kind == scoop_doc::RelationKind::Inherited),
            "{name}: {:?}",
            declaration.relations
        );
    }
    let constructed = item(input, ApiKind::Class, "Constructed");
    assert!(
        constructed
            .relations
            .iter()
            .any(|relation| relation.signature.contains("fun projected()")
                && relation
                    .signature
                    .ends_with("where Wrapper<U>.Companion : Mark")),
        "{:?}",
        constructed.relations
    );
}

#[test]
fn m57_documentation_preserves_conditions_on_relations_and_declared_members() {
    let source = format!(
        "{}\n{}",
        PROVIDER.replace("package api", ""),
        CONSUMER.replace("import api.*", "")
    );
    let input = project(&lower_documented(&source).unwrap());
    check(&input);
    let base = item(&input, ApiKind::Class, "Base");
    assert!(
        base.relations
            .iter()
            .any(|relation| relation.signature == "Read where T : Hash"),
        "{:?}",
        base.relations
    );
    for (kind, name, expected) in [
        (
            ApiKind::Function,
            "read",
            "public override fun read(): Int where T : Hash",
        ),
        (
            ApiKind::Property,
            "size",
            "public override val size: Int where T : Hash",
        ),
        (ApiKind::Getter, "get", "get(): Int where T : Hash"),
    ] {
        let member = input
            .items
            .iter()
            .find(|item| {
                item.kind == kind
                    && item.name == name
                    && (kind == ApiKind::Getter || item.parent.as_ref() == Some(&base.key))
            })
            .unwrap();
        assert_eq!(signature(member), expected);
    }
    let defaults = item(&input, ApiKind::Class, "Defaults");
    assert!(
        defaults
            .relations
            .iter()
            .any(|relation| relation.signature == "fun defaultRead(): Int where T : Hash"),
        "{:?}",
        defaults.relations
    );
    let choice = item(&input, ApiKind::Class, "Choice");
    for suffix in ["where T : value", "where T : ref"] {
        assert_eq!(
            choice
                .relations
                .iter()
                .filter(|relation| relation.signature.ends_with(suffix))
                .count(),
            2,
            "{:?}",
            choice.relations
        );
    }
    let projected = item(&input, ApiKind::Class, "Projected");
    let method = input
        .items
        .iter()
        .find(|item| item.name == "projected" && item.parent.as_ref() == Some(&projected.key))
        .unwrap();
    assert!(signature(method).ends_with("where T.Companion : Mark"));
}

#[test]
fn m57_documentation_imported_members_keep_applied_conditions_without_machine_uses() {
    use super::super::super::m23_ordinary_dependencies::support::{
        alias_expansions, project_dependency_text,
    };
    let mut core = support::trusted_core();
    let coordinate =
        scoop_identity::ConeCoordinate::new("test", "conditional-doc", "1.0.0").unwrap();
    let (foundation, interface) =
        project_dependency_text(&core, &coordinate, PROVIDER, &["Boolean"]);
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 74);
    let aliases = alias_expansions(interface.type_aliases());
    let parsed = support::parsed_ordinary_text(CONSUMER);
    let world = hir::ImportedSemanticWorld::from_dependencies(
        parsed.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let sources = crate::CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let ordinary =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &sources).unwrap();
    let file = scoop_parser::parse(CONSUMER).unwrap();
    let sources = sources
        .with_documentation(vec![ast::IdentifiedSourceDocumentation {
            source: test_source_identity("src/main.scoop"),
            documentation: scoop_doc::prepare_source(&file, CONSUMER),
        }])
        .unwrap();
    let output =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &sources).unwrap();
    check(&project(&output));
    assert_eq!(
        output.output().export.imported_dependency_callables.len(),
        ordinary.output().export.imported_dependency_callables.len()
    );
    assert_eq!(
        output.output().export.types.len(),
        ordinary.output().export.types.len()
    );
}
