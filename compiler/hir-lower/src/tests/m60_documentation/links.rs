use super::*;

#[test]
fn m60_documentation_resolves_overloads_aliases_members_and_imported_core() {
    let output = lower_documented(
        r#"
        public fun parse(value: Int): Int = value
        public fun parse(value: String): String = value
        public struct Record(val field: Int)
        public typealias Entry = Record
        /** [parse] [Entry] [Entry.field] [Int] [Int.plus] */
        public fun describe() {}
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let links = &comment(&output, Kind::Function, "describe").body.references;
    assert!(matches!(&links[0].resolution, Link::Symbols(symbols) if symbols.len() == 2));
    assert_eq!(
        current(&links[1].resolution),
        named(&output, Kind::TypeAlias, "Entry").target
    );
    assert_eq!(
        current(&links[2].resolution),
        named(&output, Kind::Field, "field").target
    );
    assert!(matches!(&links[3].resolution, Link::Symbols(symbols)
        if matches!(symbols.first(), Symbol::Imported(hir::AnnotationTargetV1::Nominal(_)))));
    assert!(matches!(&links[4].resolution, Link::Symbols(symbols)
        if symbols.iter().all(|symbol| matches!(symbol, Symbol::Imported(hir::AnnotationTargetV1::Callable(_))))));
}

#[test]
fn m60_documentation_uses_definition_scope_and_inherited_members() {
    let output = lower_documented(
        r#"
        package sample
        public open class Base {
            public fun item(): Int = 1
        }
        public class Derived : Base() {
            /** [item] [Base.item] [sample.Base] [Derived] */
            public fun describe() {}
        }
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let links = &comment(&output, Kind::Function, "describe").body.references;
    assert_eq!(
        current(&links[0].resolution),
        named(&output, Kind::Function, "item").target
    );
    assert_eq!(
        current(&links[1].resolution),
        named(&output, Kind::Function, "item").target
    );
    assert_eq!(
        current(&links[2].resolution),
        named(&output, Kind::Class, "Base").target
    );
    assert_eq!(
        current(&links[3].resolution),
        named(&output, Kind::Class, "Derived").target
    );
}
