use scoop_ast::{ClassMember, Decl, Span};

use crate::tests::{err, ok};

#[test]
fn release_block_is_a_distinct_member_with_its_original_span() {
    let source = "class Resource<T>(val value: T) {\n    release { consume(value) }\n    fun release() {}\n}";
    let file = ok(source);
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::ReleaseBlock(release) = &class.members[0] else {
        panic!("expected release block")
    };
    let start = source.find("release {").unwrap() as u32;
    let end = source.find(" }\n").unwrap() as u32 + 2;
    assert_eq!(release.span, Span::new(start, end));
    assert_eq!(release.body.statements.len(), 1);
    assert_eq!(class.functions().count(), 1);
    assert_eq!(class.functions().next().unwrap().name.text, "release");
    assert_eq!(class.secondary_constructors().count(), 0);
}

#[test]
fn release_remains_an_identifier_outside_block_member_syntax() {
    ok(
        "fun release(release: Int): Int { val value = release; return value }\n\
        enum State { release, Done }\n\
        class Outer { class Inner { release {} }\n val release: Int = 1 }",
    );
}

#[test]
fn release_rejects_callable_spelling_and_member_prefixes() {
    for (member, expected) in [
        ("release() {}", "without parameters or a return type"),
        ("release: Unit {}", "without parameters or a return type"),
        ("release named {}", "without parameters or a return type"),
        ("@NoGC release {}", "annotations are not allowed"),
        ("public release {}", "visibility is not allowed"),
        ("private release {}", "visibility is not allowed"),
        ("open release {}", "modality"),
        ("override release {}", "modifier"),
    ] {
        let (_, message) = err(&format!("class Owner {{ {member} }}"));
        assert!(message.contains(expected), "{member}: {message}");
    }
}

#[test]
fn value_and_interface_bodies_reject_release_blocks_at_the_keyword() {
    for source in [
        "struct Value() { release {} }",
        "interface Contract { release {} }",
        "enum Choice { Ready,\n release {} }",
    ] {
        let (span, message) = err(source);
        assert_eq!(&source[span.start as usize..span.end as usize], "release");
        assert!(message.contains("only ordinary final classes"));
    }
}

#[test]
fn invalid_release_prefix_recovers_at_the_next_member() {
    let diagnostics =
        crate::parse("class Owner {\n @NoGC release {}\n fun broken(: Int) {}\n}").unwrap_err();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(
        diagnostics[0]
            .message
            .contains("annotations are not allowed")
    );
    assert!(diagnostics[1].message.contains("parameter name"));
}

#[test]
fn sealed_release_owner_is_rejected_by_the_parser() {
    let (span, message) = err("sealed class Owner { release {} }");
    assert_eq!(span, Span::new(0, 6));
    assert!(message.contains("`sealed` classes"));
}
