use super::*;

#[test]
fn m60_documentation_projects_source_signatures_and_inferred_types() {
    let source = r#"
        /** A value of [T].
         * @param T Element type.
         * @param value Initial element.
         * @constructor Constructs a cell.
         */
        public class Cell<T>(val value: T) {
            /** Reads the element. */
            public fun read() = value
        }
        public typealias Count = Int
        /** Adds the default increment. */
        public fun add(value: Count, increment: Int = 2) = value + increment
        public val inferred = 42
        "#;
    let output = lower_documented(source).unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let add = named(&output, Kind::Function, "add");
    let text = add
        .signature
        .iter()
        .filter_map(|part| match part {
            Part::Source(span)
            | Part::LinkedSource { span, .. }
            | Part::SourceType { span, .. } => {
                Some(&source[span.start as usize..span.end as usize])
            }
            _ => None,
        })
        .collect::<String>();
    assert!(text.contains("value: Count"));
    assert!(text.contains("increment: Int = 2"));
    assert!(
        add.signature
            .iter()
            .any(|part| matches!(part, Part::Type(ty)
        if matches!(output.output().export.types[*ty], hir::Type::Integer(_))))
    );
    assert!(
        named(&output, Kind::Property, "inferred")
            .signature
            .iter()
            .any(|part| matches!(part, Part::Type(_)))
    );
    let cell = comment(&output, Kind::Class, "Cell");
    assert_eq!(cell.tags.len(), 3);
    assert!(matches!(
        current(&cell.body.references[0].resolution),
        Target::TypeParameter { .. }
    ));
    assert!(matches!(
        cell.tags[1].kind,
        Tag::Param(Target::Declaration(Source::Parameter { .. }))
    ));
    assert!(matches!(
        cell.tags[2].kind,
        Tag::Constructor(Target::Declaration(Source::Callable(
            Callable::ClassConstructor(_)
        )))
    ));
}

#[test]
fn m60_documentation_keeps_primary_property_and_parameter_descriptions_separate() {
    let source = r#"
        /**
         * @param value Initial argument.
         * @property value Duplicate property description.
         */
        public class Cell(
            /** Stored element. */ public val value: Int
        ) {
            /** Read value. */
            public var current: Int
                /** Getter text. */
                get() = value
                /** Setter text.
                 * @param next Ignored replacement.
                 */
                private set(next) {}
        }
        "#;
    let output = lower_documented(source).unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert_eq!(docs.warnings.len(), 1, "{:?}", docs.warnings);
    assert!(docs.warnings[0].message.contains("duplicate documentation"));
    assert_eq!(
        comment(&output, Kind::Property, "value").body.text.text,
        "Stored element."
    );
    let cell = comment(&output, Kind::Class, "Cell");
    assert_eq!(cell.tags.len(), 1);
    assert!(matches!(cell.tags[0].kind, Tag::Param(_)));
    let setter = named(&output, Kind::Setter, "set");
    assert_eq!(setter.visibility, hir::DeclaredVisibility::Private);
    assert_eq!(
        setter.parent,
        Some(named(&output, Kind::Property, "current").target)
    );
    assert!(matches!(
        setter.comment.as_ref().unwrap().tags[0].kind,
        Tag::Param(Target::Declaration(Source::Parameter {
            callable: Callable::Setter(_),
            index: 0
        }))
    ));
}

#[test]
fn m60_documentation_has_variants_payloads_annotations_and_nested_owners() {
    let output = lower_documented(
        r#"
        /** A note.
         * @param text Note contents.
         */
        public annotation class Note(val text: String)
        public enum Message {
            /** Empty. */ Empty,
            /** Holds [value].
             * @param value Payload.
             */ Value(/** Payload text. */ val value: Int)
        }
        public struct Outer(val number: Int) {
            /** Nested type. */ public class Nested
            public companion object {
                /** Creates. */ public fun create(): Int = 1
            }
        }
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert_eq!(docs.warnings.len(), 1, "{:?}", docs.warnings);
    let value = comment(&output, Kind::Variant, "Value");
    assert!(matches!(
        current(&value.body.references[0].resolution),
        Target::Declaration(Source::VariantField(_))
    ));
    assert!(matches!(
        comment(&output, Kind::Annotation, "Note").tags[0].kind,
        Tag::Param(Target::AnnotationParameter { .. })
    ));
    assert_eq!(
        named(&output, Kind::Class, "Nested").parent,
        Some(named(&output, Kind::Struct, "Outer").target)
    );
    assert_eq!(
        named(&output, Kind::Function, "create").parent,
        Some(named(&output, Kind::Object, "Companion").target)
    );
}
