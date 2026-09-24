use super::*;
use hir::{
    CanonicalExportDefinitionSourcesV1, CanonicalIntegerConstantV1, DefaultExpressionKindV1,
    DefaultExpressionV1,
};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let template = checked
        .section()
        .protected_defaults()
        .records()
        .iter()
        .find(|template| {
            matches!(
                template.body().value().kind(),
                DefaultExpressionKindV1::IntegerLiteral(_)
            )
        })
        .unwrap();
    let value = template.body().value();
    let body = ExportDefaultBodyV1::try_new(
        template.body().statements().to_vec(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed32(19)),
            value.result_type().clone(),
            value.definition_origin().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let changed = replace(
        template,
        body,
        template.references().clone(),
        template.allows_suspend(),
    );
    assert!(
        matches!(reject(checked, core, changed), Error::DefaultContract(key) if key == template.key())
    );
    let changed = replace(
        template,
        template.body().clone(),
        template.references().clone(),
        CanonicalBooleanV1::True,
    );
    assert!(
        matches!(reject(checked, core, changed), Error::DefaultContract(key) if key == template.key())
    );

    let source = checked.section();
    let mut origins = source.definition_sources().sources().to_vec();
    origins.remove(0);
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        source.inheritance().clone(),
        source.protected_declarations().clone(),
        source.protected_source_interfaces().clone(),
        source.protected_defaults().clone(),
        CanonicalExportDefinitionSourcesV1::try_new(origins).unwrap(),
        source.selected().clone(),
    );
    assert!(matches!(
        reject_section(checked, core, &candidate),
        Error::DefinitionSources(_)
    ));
}
