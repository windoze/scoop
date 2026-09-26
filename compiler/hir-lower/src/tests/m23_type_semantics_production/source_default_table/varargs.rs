use super::*;
#[test]
fn complete_default_sources_distinguish_vararg_default_and_empty_omission() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/parameter-varargs.scoop"
    ));
    let mut core = complete_core_file();
    core.declarations
        .extend(scoop_parser::parse(source).unwrap().declarations);
    let parsed = super::super::super::m23_ordinary_core_only::support::parsed_core(core);
    let output = crate::lower_core_bootstrap(&parsed).unwrap();
    let export = output.export.module();
    let production = Production::from_export_hir(&output.export).unwrap();
    let defaulted = declaration(export, function(export, "Variadic.defaulted"));
    let collect = declaration(export, function(export, "Variadic.collect"));
    let default_key = hir::ProtectedDefaultTemplateKeyV1::try_new(defaulted, 1).unwrap();
    let empty_key = hir::ProtectedDefaultTemplateKeyV1::try_new(collect, 0).unwrap();
    let body = production.templates().get(default_key).unwrap();
    assert!(matches!(
        body.body().value().kind(),
        hir::DefaultExpressionKindV1::ArrayLiteral(_)
    ));
    assert!(production.templates().get(empty_key).is_none());
    assert_eq!(
        production.parameters().get(defaulted).unwrap().parameters()[1].calling_kind(),
        hir::ProtectedParameterCallingKindV1::VarargDefault
    );
    assert_eq!(
        production.parameters().get(collect).unwrap().parameters()[0].calling_kind(),
        hir::ProtectedParameterCallingKindV1::VarargEmpty
    );
    production
        .templates()
        .validate_parameter_coverage(production.parameters())
        .unwrap();
}
