use super::*;

#[test]
fn m57_impossible_conditions_establish_negative_facts_for_open_applications() {
    check(
        r#"
        interface Mark {}
        interface Flag {}
        open class Left
        open class Right
        struct RefOnly<T>() { override interface Mark where T : ref {} }
        struct LeftOnly<T>() { override interface Mark where T : Left {} }
        struct Marked<T>() { override interface Mark where T : Flag {} }
        struct Recursive<T>() { override interface Mark where T : Mark {} }
        struct Pair<A, B>() { override interface Mark where A : ref, B : Flag {} }
        fun <X : !Mark> absent(value: X): Int = 1
        fun <T : value> byKind(value: RefOnly<T>): Int = absent(value)
        fun <T : Right> byClass(value: LeftOnly<T>): Int = absent(value)
        fun <T : !Flag> byNegativeFact(value: Marked<T>): Int = absent(value)
        fun <T, U : value> independent(value: Pair<U, T>): Int = absent(value)
        fun <T : !Mark> nested(value: Recursive<Recursive<T>>): Int = absent(value)
        fun main() { byKind(RefOnly<Int>()) }
    "#,
    )
    .unwrap();
}

#[test]
fn m57_query_cycle_points_to_its_declaration_file() {
    let text = "public interface Mark {}\npublic open class Gate<T> public constructor() { override interface Mark where T : Mark {} }";
    let declaration = scoop_parser::parse(text).unwrap();
    let consumer = scoop_parser::parse("class Loop : Gate<Loop>()\nfun main() {}").unwrap();
    let errors = lower(&[complete_core_file(), declaration, consumer]).unwrap_err();
    let cycle = errors
        .iter()
        .find(|error| error.message.contains("cyclic conditional interface proof"))
        .unwrap_or_else(|| panic!("{errors:#?}"));
    assert_eq!(cycle.file, 1, "{cycle:?}");
    assert_eq!(
        cycle.span.unwrap().start as usize,
        text.find("override").unwrap()
    );
}

#[test]
fn m57_query_cycle_keeps_the_dependency_source_identity() {
    let errors = super::super::m23_generic_body_consumption::with_provider_consumer(
        "package api\npublic interface Mark {}\npublic open class Gate<T> public constructor() { override interface Mark where T : Mark {} }",
        "import api.*\nclass Loop : Gate<Loop>()\nfun main() {}",
        |_, _, _, _, _| (),
    ).unwrap_err();
    let cycle = errors
        .iter()
        .find(|error| error.message.contains("cyclic conditional interface proof"))
        .unwrap_or_else(|| panic!("{errors:#?}"));
    let provider =
        scoop_identity::ConeCoordinate::new("test", "generic-provider", "1.0.0").unwrap();
    assert_eq!(
        cycle.source.as_ref().unwrap().cone(),
        provider.identity().unwrap()
    );
}
