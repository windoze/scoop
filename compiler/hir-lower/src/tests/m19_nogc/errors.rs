use super::*;

fn position(source: &str, offset: u32) -> (usize, usize) {
    let prefix = &source[..offset as usize];
    (
        prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        prefix.rsplit('\n').next().unwrap().chars().count() + 1,
    )
}

#[test]
fn constructor_nogc_errors_preserve_messages_and_source_locations() {
    let mut rows = Vec::new();
    for (name, source) in [
        (
            "class-primary",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/class-primary.scoop"
            )),
        ),
        (
            "class-secondary",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/class-secondary.scoop"
            )),
        ),
        (
            "duplicate",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/duplicate.scoop"
            )),
        ),
        (
            "arguments",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/arguments.scoop"
            )),
        ),
        (
            "parameter",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/parameter.scoop"
            )),
        ),
        (
            "result",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/result.scoop"
            )),
        ),
        (
            "local",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/local.scoop"
            )),
        ),
        (
            "body",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/body.scoop"
            )),
        ),
        (
            "delegation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/delegation.scoop"
            )),
        ),
        (
            "caller",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/caller.scoop"
            )),
        ),
        (
            "default",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/default.scoop"
            )),
        ),
        (
            "generic",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/generic.scoop"
            )),
        ),
        (
            "wrapper-function",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/wrapper-function.scoop"
            )),
        ),
        (
            "wrapper-class",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/wrapper-class.scoop"
            )),
        ),
        (
            "generic-delegation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/generic-delegation.scoop"
            )),
        ),
        (
            "generic-base",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/generic-base.scoop"
            )),
        ),
        (
            "ref-bound",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/ref-bound.scoop"
            )),
        ),
        (
            "polymorphic-class",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/polymorphic-class.scoop"
            )),
        ),
        (
            "polymorphic-struct",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/polymorphic-struct.scoop"
            )),
        ),
        (
            "polymorphic-mixed",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/errors/polymorphic-mixed.scoop"
            )),
        ),
    ] {
        for error in lower_source(source).unwrap_err() {
            assert_eq!(error.file, 1, "{name}: {error:?}");
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
            "/../../tests/fixtures/m19-constructor-nogc/errors.snap"
        ))
    );
}
