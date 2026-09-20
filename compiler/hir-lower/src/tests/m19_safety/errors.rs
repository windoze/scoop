use super::*;

fn position(source: &str, offset: u32) -> (usize, usize) {
    let prefix = &source[..offset as usize];
    (
        prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        prefix.rsplit('\n').next().unwrap().chars().count() + 1,
    )
}

#[test]
fn constructor_safety_errors_preserve_diagnostic_messages_and_locations() {
    let mut rows = Vec::new();
    for (name, source) in [
        (
            "direct-class",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/direct-class.scoop"
            )),
        ),
        (
            "direct-struct",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/direct-struct.scoop"
            )),
        ),
        (
            "generic",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/generic.scoop"
            )),
        ),
        (
            "delegation-this",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/delegation-this.scoop"
            )),
        ),
        (
            "delegation-super",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/delegation-super.scoop"
            )),
        ),
        (
            "default",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/default.scoop"
            )),
        ),
        (
            "body",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/body.scoop"
            )),
        ),
        (
            "common-init",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/common-init.scoop"
            )),
        ),
        (
            "common-property",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/common-property.scoop"
            )),
        ),
        (
            "safe-block",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/safe-block.scoop"
            )),
        ),
        (
            "overload",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/overload.scoop"
            )),
        ),
        (
            "duplicate",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/duplicate.scoop"
            )),
        ),
        (
            "conflict",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/conflict.scoop"
            )),
        ),
        (
            "arguments",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-safety/errors/arguments.scoop"
            )),
        ),
    ] {
        let errors = lower_source(source).unwrap_err();
        for error in errors {
            let span = error.span.unwrap();
            let (line, column) = position(source, span.start);
            let (end_line, end_column) = position(source, span.end);
            rows.push(format!(
                "{name}:{line}:{column}-{end_line}:{end_column}: {}\n",
                error.message
            ));
        }
    }
    rows.sort();
    assert_eq!(
        rows.concat(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m19-constructor-safety/errors.snap"
        ))
    );
}
