use super::*;

mod errors;
mod render;

fn lower_source(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

#[test]
fn constructor_nogc_effects_and_requirements_survive_concretization_and_mir() {
    for (source, expected) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/scalar.scoop"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/scalar.snap"
            )),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/generic.scoop"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-nogc/generic.snap"
            )),
        ),
    ] {
        let output = lower_source(source).unwrap();
        let mir = scoop_mir_lower::lower(&output.local).unwrap();
        assert_eq!(render::contracts(&output, &mir), expected);
    }
}
