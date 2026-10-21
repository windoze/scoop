use super::*;
use scoop_doc::RelationKind;

const PROVIDER: &str = r#"
    package api
    public open class Base<T> public constructor(public val value: T) {
        public fun copy(input: T): T = input
        public fun <U> other(input: U, fallback: T): U = input
        public fun <U> constrained(input: U): Int where requires(x: U) { x.read(): T } = 1
        public fun defaulted(value: Int = 1): Int = value
        @Unsafe public fun hazardous(): Int = 2
        public open fun number(): Int = 1
        public open val size: Int get() = 1
        public var state: Int get() = 1
            private set(next) {}
        @DocHidden public fun secret(): Int = 0
        protected fun protectedItem(): Int = 3
    }
    public interface Readable<T> {
        public fun read(value: T): T = value
    }
"#;
const CONSUMER: &str = r#"
    import api.Base
    import api.Readable
    public class Child : Base<Int>(42) {
        public override fun number(): Int = 2
        public override val size: Int get() = 2
    }
    public interface Reader : Readable<Int> {}
    public interface Left : Readable<Int> {}
    public interface Right : Readable<Int> {}
    public interface Diamond : Left, Right {}
    public class Explicit : Readable<Int> {
        public override fun read(value: Int): Int = value
    }
"#;

fn check(input: &ConeInput) {
    let child = item(input, ApiKind::Class, "Child");
    let inherited = child
        .relations
        .iter()
        .filter(|r| r.kind == RelationKind::Inherited)
        .collect::<Vec<_>>();
    assert!(
        inherited
            .iter()
            .any(|r| r.signature == "fun copy(input: Int): Int" && r.origin == "Base<Int>"),
        "{inherited:?}"
    );
    assert!(
        inherited.iter().any(|r| r.signature
            == "fun constrained<U>(input: U): Int where requires(x: U) { x.read(): Int }"),
        "{inherited:?}"
    );
    assert!(
        inherited
            .iter()
            .any(|r| r.signature == "fun defaulted(value: Int = …): Int"),
        "{inherited:?}"
    );
    assert!(
        inherited
            .iter()
            .any(|r| r.signature == "fun other<U>(input: U, fallback: Int): U"),
        "{inherited:?}"
    );
    assert!(
        inherited.iter().any(|r| r.signature == "val value: Int"),
        "{inherited:?}"
    );
    assert!(
        !inherited.iter().any(|r| r.signature.contains("secret")
            || r.signature.contains("number")
            || r.signature.contains("size")),
        "{inherited:?}"
    );
    let reader = item(input, ApiKind::Interface, "Reader");
    assert!(
        inherited
            .iter()
            .any(|r| r.signature == "var state: Int { public get; private set }"),
        "{inherited:?}"
    );
    assert!(
        inherited
            .iter()
            .any(|r| r.signature == "@Unsafe fun hazardous(): Int"),
        "{inherited:?}"
    );
    assert!(
        reader
            .relations
            .iter()
            .any(|r| r.signature == "fun read(value: Int): Int"),
        "{:?}",
        reader.relations
    );
    let diamond = item(input, ApiKind::Interface, "Diamond");
    assert_eq!(
        diamond
            .relations
            .iter()
            .filter(
                |r| r.kind == RelationKind::Inherited && r.signature == "fun read(value: Int): Int"
            )
            .count(),
        1,
        "{:?}",
        diamond.relations
    );
    let number = input
        .items
        .iter()
        .find(|i| i.name == "number" && i.parent.as_ref() == Some(&child.key))
        .unwrap();
    assert!(
        number
            .relations
            .iter()
            .any(|r| r.kind == RelationKind::Overrides)
    );
    let size = input
        .items
        .iter()
        .find(|i| {
            i.kind == ApiKind::Property && i.name == "size" && i.parent.as_ref() == Some(&child.key)
        })
        .unwrap();
    assert!(
        size.relations
            .iter()
            .any(|r| r.kind == RelationKind::Overrides)
    );
    assert!(number.comment.is_none());
    let explicit = item(input, ApiKind::Class, "Explicit");
    let read = input
        .items
        .iter()
        .find(|i| i.name == "read" && i.parent.as_ref() == Some(&explicit.key))
        .unwrap();
    assert!(
        read.relations
            .iter()
            .any(|r| r.kind == RelationKind::Overrides)
    );
}

#[test]
fn m60_documentation_inheritance_substitutes_members_and_keeps_real_override_origins() {
    let source = format!(
        "{}\n{}",
        PROVIDER.replace("package api", ""),
        CONSUMER
            .replace("import api.Base", "")
            .replace("import api.Readable", "")
    );
    let output = lower_with_core(&annotations::annotation_core(), &source, true).unwrap();
    let input = project(&output);
    check(&input);
}

#[test]
fn m60_documentation_imported_inheritance_does_not_select_machine_calls_or_hidden_members() {
    use super::super::super::m23_ordinary_dependencies::support::{
        alias_expansions, project_dependency_text,
    };
    let mut core = annotations::annotation_core();
    let coordinate = scoop_identity::ConeCoordinate::new("test", "doc-provider", "1.0.0").unwrap();
    let provider = format!(
        "{PROVIDER}\n@DocHidden public class HiddenOuter {{ public class Inner }}\n@DocHidden public object HiddenObject\n@DocHidden public fun hidden(): Int = 0"
    );
    let (foundation, interface) =
        project_dependency_text(&core, &coordinate, &provider, &["Boolean"]);
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 73);
    let aliases = alias_expansions(interface.type_aliases());
    let consumer = format!(
        "public import api.HiddenOuter.Inner as HiddenAlias\npublic import api.HiddenObject as HiddenValue\npublic import api.hidden as hiddenFunction\npublic import api.Base as VisibleBase\n{CONSUMER}"
    );
    let parsed = support::parsed_ordinary_text(&consumer);
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
    let file = scoop_parser::parse(&consumer).unwrap();
    let input = crate::CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let ordinary =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let input = input
        .with_documentation(vec![ast::IdentifiedSourceDocumentation {
            source: test_source_identity("src/main.scoop"),
            documentation: scoop_doc::prepare_source(&file, &consumer),
        }])
        .unwrap();
    let output =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let projected = project(&output);
    check(&projected);
    let bindings = projected
        .bindings
        .iter()
        .map(|binding| binding.name.as_str())
        .collect::<Vec<_>>();
    assert!(bindings.contains(&"VisibleBase"), "{bindings:?}");
    for hidden in ["HiddenAlias", "HiddenValue", "hiddenFunction"] {
        assert!(!bindings.contains(&hidden), "{bindings:?}");
    }
    assert_eq!(
        output.output().export.imported_dependency_callables.len(),
        ordinary.output().export.imported_dependency_callables.len()
    );
    assert_eq!(
        output.output().export.types.len(),
        ordinary.output().export.types.len()
    );
}
