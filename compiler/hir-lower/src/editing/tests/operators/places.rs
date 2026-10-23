use super::*;

mod aliases;

const BOX: &str = "class Box<T>(var item: T) { operator fun get(index: Boolean): T = item; operator fun set(index: Boolean, value: T) { item = value } }\n";
const VALUE: &str = "class Value { operator fun inc(): Value = this; operator fun dec(): Value = this; operator fun plus(other: Value): Value = this; operator fun minus(other: Value): Value = this; operator fun times(other: Value): Value = this; operator fun div(other: Value): Value = this; operator fun rem(other: Value): Value = this }\n";

fn selected<'a>(
    references: &'a [hir::EditingCallableReference],
    text: &str,
    expression: &str,
    token: &str,
) -> Vec<&'a hir::EditingCallableReference> {
    let start = text.find(expression).unwrap() + expression.find(token).unwrap();
    let span = ast::Span::new(start as u32, (start + token.len()) as u32);
    references
        .iter()
        .filter(|reference| reference.span == span)
        .collect()
}

fn assert_selected(
    references: &[hir::EditingCallableReference],
    text: &str,
    expression: &str,
    token: &str,
    methods: &[&str],
) {
    let selected = selected(references, text, expression, token);
    assert_eq!(
        selected.len(),
        methods.len(),
        "{expression} {token}: {selected:?}"
    );
    for method in methods {
        let origin = at(
            "src/places.scoop",
            text,
            &format!("{method}("),
            method.len(),
        );
        assert!(
            selected
                .iter()
                .any(|reference| reference.definition.as_ref() == Some(&origin)),
            "{method}: {selected:?}"
        );
    }
}

#[test]
fn generic_subscript_reads_and_assignments_keep_the_original_methods() {
    let text = format!(
        "{BOX}fun editing(first: Box<Boolean>, second: Box<String>) {{ val one = first[true]; first[false] = true; val two = second[false]; second[true] = \"text\" }}"
    );
    let mut analysis = analyze_files(&[("src/places.scoop", &text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 4, "{references:?}");
    for (expression, method) in [
        ("first[true]", "get"),
        ("first[false]", "set"),
        ("second[false]", "get"),
        ("second[true]", "set"),
    ] {
        assert_selected(&references, &text, expression, "[", &[method]);
    }
    assert_eq!(references[0].target, references[2].target);
    assert_eq!(references[1].target, references[3].target);
}

#[test]
fn updates_and_compound_fallbacks_keep_reads_operators_and_writebacks() {
    let expressions = [
        ("++box[true]", "++", "inc"),
        ("--box[true]", "--", "dec"),
        ("box[false]++", "++", "inc"),
        ("box[false]--", "--", "dec"),
        ("box[true] += value", "+=", "plus"),
        ("box[true] -= value", "-=", "minus"),
        ("box[true] *= value", "*=", "times"),
        ("box[true] /= value", "/=", "div"),
        ("box[true] %= value", "%=", "rem"),
    ];
    let body = expressions
        .iter()
        .map(|(expression, _, _)| *expression)
        .collect::<Vec<_>>()
        .join("; ");
    let text = format!(
        "{VALUE}{BOX}fun editing(box: Box<Value>, value: Value) {{ {body}; var local = value; ++local; local--; local += value }}"
    );
    let mut analysis = analyze_files(&[("src/places.scoop", &text)]);
    let references = references(&mut analysis);
    assert_eq!(
        references.len(),
        expressions.len() * 3 + 3,
        "{references:?}"
    );
    for (expression, token, method) in expressions {
        assert_selected(&references, &text, expression, "[", &["get", "set"]);
        assert_selected(&references, &text, expression, token, &[method]);
    }
    for (expression, token, method) in [
        ("++local", "++", "inc"),
        ("local--", "--", "dec"),
        ("local += value", "+=", "plus"),
    ] {
        assert_selected(&references, &text, expression, token, &[method]);
    }
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let bracket = at("src/places.scoop", &text, "[true] +=", 1);
    assert_eq!(index.targets_at(&bracket).len(), 2);
    for target in index.targets_at(&bracket) {
        assert_eq!(index.references_to(*target).count(), expressions.len() + 1);
    }
}

#[test]
fn assignment_operators_on_read_only_subscripts_do_not_invent_set_calls() {
    let operations = [
        ("+=", "plusAssign"),
        ("-=", "minusAssign"),
        ("*=", "timesAssign"),
        ("/=", "divAssign"),
        ("%=", "remAssign"),
    ];
    let methods = operations
        .iter()
        .map(|(_, name)| format!("operator fun {name}(other: Value) {{}}"))
        .collect::<Vec<_>>()
        .join("; ");
    let body = operations
        .iter()
        .map(|(token, _)| format!("box[true] {token} value"))
        .collect::<Vec<_>>()
        .join("; ");
    let text = format!(
        "class Value {{ {methods} }}\nclass ReadOnly(val item: Value) {{ operator fun get(index: Boolean): Value = item }}\nfun editing(box: ReadOnly, value: Value) {{ {body} }}"
    );
    let mut analysis = analyze_files(&[("src/places.scoop", &text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), operations.len() * 2, "{references:?}");
    for (token, method) in operations {
        let expression = format!("box[true] {token} value");
        assert_selected(&references, &text, &expression, "[", &["get"]);
        assert_selected(&references, &text, &expression, token, &[method]);
    }
}

#[test]
fn nested_index_updates_keep_each_source_operation_separate() {
    let text = format!(
        "{VALUE}{BOX}class Outer(val item: Value) {{ operator fun get(index: Value): Value = item; operator fun set(index: Value, value: Value) {{}} }}\nfun editing(box: Outer, slot: Box<Value>, value: Value) {{ box[slot[true]++] += value }}"
    );
    let mut analysis = analyze_files(&[("src/places.scoop", &text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 6, "{references:?}");
    let inner = selected(&references, &text, "slot[true]", "[");
    let outer = selected(&references, &text, "box[slot", "[");
    assert_eq!(inner.len(), 2);
    assert_eq!(outer.len(), 2);
    assert!(
        inner
            .iter()
            .all(|reference| outer.iter().all(|other| reference.target != other.target))
    );
    assert_selected(&references, &text, "slot[true]++", "++", &["inc"]);
    assert_selected(&references, &text, "] += value", "+=", &["plus"]);
}

#[test]
fn ambiguous_or_unwritable_updates_and_cancellation_publish_no_partial_targets() {
    for (methods, expression) in [
        (
            "operator fun plus(other: Value): Value = this; operator fun plusAssign(other: Value) {}",
            "box[true] += value",
        ),
        ("operator fun inc(): Value = this", "box[true]++"),
        (
            "operator fun plus(other: Value): Value = this",
            "box[true] += value",
        ),
    ] {
        let text = format!(
            "class Value {{ {methods} }}\nclass ReadOnly(val item: Value) {{ operator fun get(index: Boolean): Value = item }}\nfun editing(box: ReadOnly, value: Value) {{ {expression} }}"
        );
        let mut analysis = analyze_files(&[("src/places.scoop", &text)]);
        let function = function(&analysis, "editing");
        assert!(analysis.function_facts(function, || true).is_err());
        assert!(matches!(
            analysis.function_facts(function, || false).unwrap(),
            V::Unavailable(U::BodyFailure)
        ));
        let index = analysis.reference_index(|| false).unwrap();
        assert!(!index.unavailable.is_empty());
        let bracket = at("src/places.scoop", &text, "[true]", 1);
        assert!(index.targets_at(&bracket).is_empty());
    }
}
