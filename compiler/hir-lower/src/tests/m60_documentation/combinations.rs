use super::*;

#[test]
fn m60_documentation_excludes_local_functions_and_includes_derived_methods() {
    let output = lower_documented(
        r#"
        /** Uses [Record.equals]. */
        public struct Record(val value: Int)
        public fun outer(): Int {
            /** Local helper.
             * @param value Local parameter.
             */
            fun local(value: Int): Int = value
            return local(1)
        }
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    assert!(!docs.declarations.iter().any(|entry| entry.name == "local"));
    let equals = named(&output, Kind::Function, "equals");
    assert!(equals.synthesized);
    assert!(matches!(equals.target, Target::DerivedFunction(_)));
    assert_eq!(
        current(&comment(&output, Kind::Struct, "Record").body.references[0].resolution),
        equals.target
    );
    assert!(
        equals
            .signature
            .iter()
            .any(|part| matches!(part, Part::Text(text) if text == "other: "))
    );
    assert!(
        !equals
            .signature
            .iter()
            .any(|part| matches!(part, Part::Text(text) if text.contains("this:")))
    );
}

#[test]
fn m60_documentation_retains_context_receiver_and_member_requirements() {
    let source = r#"
        /** Uses [request] and [T].
         * @param request Active request.
         * @param T Element.
         * @receiver Original number.
         * @return Constant result.
         */
        context(request: String)
        public fun <T> Int.inspect(value: T): Int
            where requires(x: T) { x.read(): Int } = 1
        /** Uses [request].
         * @param request Active request.
         * @receiver Original number.
         */
        context(request: String)
        public val Int.label: String get() = request
        "#;
    let output = lower_documented(source).unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let inspect = named(&output, Kind::Function, "inspect");
    let comment = inspect.comment.as_ref().unwrap();
    assert_eq!(comment.tags.len(), 4);
    assert!(matches!(
        comment.tags[0].kind,
        Tag::Param(Target::ContextParameter { .. })
    ));
    assert!(matches!(
        comment.tags[2].kind,
        Tag::Receiver(Target::Declaration(Source::Receiver(_)))
    ));
    let written = inspect
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
    assert!(written.contains("requires(x: T)"));
    let property = named(&output, Kind::Property, "label");
    assert!(
        docs.declarations
            .iter()
            .any(|entry| entry.kind == Kind::ContextParameter
                && entry.parent == Some(property.target))
    );
}

#[test]
fn m60_documentation_input_requires_the_selected_source_identities() {
    let core = support::trusted_core();
    let parsed = support::parsed_ordinary_text("public fun entry() {}");
    let world = core.world(parsed.cone());
    let sources = crate::CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let result = sources.with_documentation(vec![ast::IdentifiedSourceDocumentation {
        source: test_source_identity("src/another.scoop"),
        documentation: ast::SourceDocumentation::default(),
    }]);
    assert!(matches!(
        result,
        Err(crate::CurrentConeSourceError::DocumentationSourcesMismatch)
    ));
}

#[test]
fn m60_documentation_keeps_inner_class_binders_distinct_from_outer_binders() {
    let output = lower_documented(
        r#"
        public class Outer<T> {
            /** [T] and [U].
             * @param U Inner element.
             */
            public inner class Inner<U>(val value: U)
        }
        "#,
    )
    .unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    let inner = comment(&output, Kind::Class, "Inner");
    let outer_parameter = current(&inner.body.references[0].resolution);
    let inner_parameter = current(&inner.body.references[1].resolution);
    assert_ne!(outer_parameter, inner_parameter);
    let Target::TypeParameter {
        parameter: outer, ..
    } = outer_parameter
    else {
        panic!();
    };
    let Target::TypeParameter {
        parameter: inner, ..
    } = inner_parameter
    else {
        panic!();
    };
    assert_ne!(outer, inner);
}
