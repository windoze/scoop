use super::*;

pub(super) fn parse_and_lower(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let core = core_file();
    lower_test_sources(
        vec![ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "<core>",
            source_text: "",
        }],
        &scoop_parser::parse(source).unwrap(),
        hir::IntrinsicProviderId::from_raw(1),
        "<user>",
        source,
        IntrinsicDeclarationPolicy::CoreOnly,
    )
}
