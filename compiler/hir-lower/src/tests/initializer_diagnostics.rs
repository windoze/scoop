use super::*;

fn errors(source: &str) -> Vec<ast::Diagnostic> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap_err()
}

#[test]
fn rejected_initializers_preserve_independent_errors() {
    let errors = errors(
        "fun take(value: Long) {}\n\
         fun take(value: String) {}\n\
         fun main() {\n\
             val x = missing\n\
             val y = x\n\
             take(y)\n\
             val z = unrelated\n\
         }",
    );
    assert_eq!(errors.len(), 2, "{errors:#?}");
    assert!(
        errors[0].message.contains("unknown variable `missing`"),
        "{errors:#?}"
    );
    assert!(
        errors[1].message.contains("unknown variable `unrelated`"),
        "{errors:#?}"
    );
}

#[test]
fn rejected_initializers_respect_lexical_scope_and_outer_initializer_reads() {
    let errors = errors(
        "fun take(value: Long) {}\n\
         fun text(value: String) {}\n\
         fun main() {\n\
             val x = \"outer\"\n\
             if (true) {\n\
                 val x = x.absent()\n\
                 take(x)\n\
             }\n\
             text(x)\n\
         }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(errors[0].message, "type `String` has no method `absent`");
}

#[test]
fn rejected_initializers_do_not_block_valid_inner_captures() {
    let errors = errors(
        "fun take(value: Long) {}\n\
         fun main() {\n\
             val x = missing\n\
             if (true) {\n\
                 val x = 1L\n\
                 val read: () -> Long = { x }\n\
                 take(read())\n\
             }\n\
         }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(
        errors[0].message.contains("unknown variable `missing`"),
        "{errors:#?}"
    );
}

#[test]
fn rejected_initializers_remain_rejected_in_nested_callables() {
    let errors = errors(
        "fun main() {\n\
             val x = missing\n\
             val read: () -> Long = { x }\n\
             fun readLater(): Long = x\n\
             read()\n\
             readLater()\n\
         }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(
        errors[0].message.contains("unknown variable `missing`"),
        "{errors:#?}"
    );
}

#[test]
fn rejected_initializers_do_not_cascade_through_writes_or_calls() {
    let errors = errors(
        "fun main() {\n\
             var x = missing\n\
             x = 1L\n\
             x += 1L\n\
             x++\n\
             x()\n\
             val reference: () -> Long = ::x\n\
             x.toString()\n\
         }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(
        errors[0].message.contains("unknown variable `missing`"),
        "{errors:#?}"
    );
}

#[test]
fn rejected_initializers_cover_destructured_bindings() {
    let errors = errors(
        "fun take(value: Long) {}\n\
         fun main() {\n\
             val (x, { member: y }) = missing\n\
             take(x)\n\
             take(y)\n\
         }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(
        errors[0].message.contains("unknown variable `missing`"),
        "{errors:#?}"
    );
}

#[test]
fn rejected_initializer_annotations_do_not_create_untyped_locals() {
    for declaration in ["val x: Long = \"bad\"", "val x: MissingType = 1L"] {
        let errors = errors(&format!("fun main() {{ {declaration}\n val y = x }}"));
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert!(
            !errors[0].message.contains("unknown variable `x`"),
            "{errors:#?}"
        );
    }
}
