use super::*;

fn with_annotations(source: &str) -> ConeInput {
    project(&lower_with_core(&annotation_core(), source, true).unwrap())
}

pub(super) fn annotation_core() -> support::TrustedCoreFixture {
    let text = format!(
        "{}\n{}",
        include_str!(
            "../../../../../../sysroot/src/scoop.core/src/serialization_annotations.scoop"
        ),
        include_str!("../../../../../../sysroot/src/scoop.core/src/documentation.scoop")
    );
    let mut core = complete_core_file();
    core.declarations
        .extend(scoop_parser::parse(&text).unwrap().declarations);
    support::trusted_core_from_source(core, &text)
}

#[test]
fn m60_documentation_projection_applies_typed_annotations_and_retains_warnings() {
    let source = r#"
        import DocHidden as Omit
        import DocAlias as Keyword
        @Omit public class Hidden {
            public fun descendant(): Int = 1
        }
        @Keyword("record") @Keyword("record") @Keyword("条目")
        public struct Record(@Omit val secret: Int, val visible: Int)
        @Omit public fun parse(value: Int): Int = value
        public fun parse(value: String): Int = 0
        @Keyword(" \t ") public fun blank() {}
        public class State(@field:Omit public val retained: Int,
            @property:Omit public val omitted: Int)
        "#;
    let input = with_annotations(source);
    let names = input
        .visible_items(true)
        .iter()
        .map(|item| (item.kind, item.name.as_str()))
        .collect::<Vec<_>>();
    for hidden in ["Hidden", "descendant"] {
        assert!(!names.iter().any(|(_, name)| *name == hidden), "{names:?}");
    }
    assert!(!names.contains(&(ApiKind::Field, "secret")));
    assert!(names.contains(&(ApiKind::Parameter, "secret")));
    assert!(names.contains(&(ApiKind::Property, "retained")));
    assert!(!names.contains(&(ApiKind::Property, "omitted")));
    assert!(names.contains(&(ApiKind::Parameter, "omitted")));
    assert_eq!(names.iter().filter(|(_, name)| *name == "parse").count(), 1);
    let record = item(&input, ApiKind::Struct, "Record");
    assert_eq!(record.aliases, ["record", "条目"]);
    assert!(signature(record).contains("secret: Int"));
    assert_eq!(input.warnings.len(), 1);
    let warning = &input.warnings[0];
    assert_eq!(warning.message, "documentation alias must not be blank");
    assert!(
        source[warning.location.range.start as usize..warning.location.range.end as usize]
            .contains("Keyword")
    );
    assert_eq!(
        ConeInput::from_json(&input.to_json().unwrap())
            .unwrap()
            .warnings,
        input.warnings
    );
}

#[test]
fn m60_documentation_projection_ignores_same_named_user_annotations() {
    let input = with_annotations(
        r#"
        package shadow
        public annotation class DocHidden
        public annotation class DocAlias(val name: String)
        @DocHidden @DocAlias("not a keyword") public class Visible
        "#,
    );
    let visible = item(&input, ApiKind::Class, "Visible");
    assert!(!visible.hidden);
    assert!(visible.aliases.is_empty());
    assert!(
        input
            .visible_items(false)
            .iter()
            .any(|item| item.key == visible.key)
    );
}
