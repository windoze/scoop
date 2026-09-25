use super::*;
use hir::{
    DefaultSourceTemplateBuildError as Build, DefaultSourceTemplateResolutionError as Resolve,
};
use scoop_identity::{LocalValueSelector, SignatureTypeKey};

#[test]
fn source_template_reader_rejects_result_local_and_provider_corruption() {
    with_hir_source(SOURCE, |output, _| {
        let value = template(output, "SourceBase.choose", 1);
        let input = wire::replaced(
            &value,
            6,
            encode(&SignatureTypeKey::Binder {
                depth: 0,
                index: 42,
            })
            .unwrap(),
        );
        assert!(matches!(
            input.resolve(&mut identity_closure(output)),
            Err(Resolve::Record(Build::ResultType))
        ));
        let input = wire::replaced(&value, 4, vec![0x80]);
        assert!(matches!(
            input.resolve(&mut identity_closure(output)),
            Err(Resolve::Body(_))
        ));
        let other = template(output, "literal", 0);
        let input = wire::replaced(&value, 2, encode(&other.definition_root()).unwrap());
        assert!(matches!(
            input.resolve(&mut identity_closure(output)),
            Err(Resolve::Record(Build::ReferenceProvider { .. }))
        ));
    });
}

#[test]
fn source_template_builder_rejects_an_unmapped_stable_local_selector() {
    with_hir_source(SOURCE, |output, _| {
        let t = template(output, "literal", 0);
        let local = LocalValueSelector::Parameter {
            declaration_index: 99,
        };
        let expression = hir::DefaultExpressionV1::try_new(
            hir::DefaultExpressionKindV1::Local(local),
            t.result().clone(),
            t.definition_origin().clone(),
        )
        .unwrap();
        let body = hir::ExportDefaultBodyV1::try_new(vec![], expression).unwrap();
        let error = Template::try_new(
            t.key(),
            t.definition_root(),
            t.definition_path().clone(),
            t.locals().clone(),
            body,
            t.result().clone(),
            t.allows_suspend(),
            t.type_parameters().clone(),
            t.receiver().clone(),
            t.value_parameters().clone(),
            t.references().clone(),
            t.definition_origin().clone(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                Build::Index(hir::DefaultSourceTemplateIndexError::Body(_))
            ),
            "{error:?}"
        );
    });
}
