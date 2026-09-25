use super::*;
use scoop_identity::SignatureTypeKey;

#[test]
fn private_literal_body_is_independent_of_public_lookup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/literal.scoop"
    ));
    with_hir_source(source, |output, _| {
        let export = output.output().export.module();
        let owner = function(export, "hidden");
        assert!(
            hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(output)
                .unwrap()
                .records()
                .is_empty()
        );
        let ordinary = Body::from_dependency_hir(output, owner, 0).unwrap();
        let direct = Body::from_export_hir(export, owner, 0).unwrap();
        assert_eq!(ordinary.body(), direct.body());
        assert_eq!(ordinary.result(), direct.result());
        assert!(
            matches!(ordinary.body().value().kind(), hir::DefaultExpressionKindV1::StringLiteral { value, .. } if value == "isolated")
        );
        assert_eq!(ordinary.parameter_position(), 0);
        assert_eq!(
            ordinary
                .definition_origin()
                .origin()
                .source()
                .logical_path()
                .as_str(),
            "src/main.scoop"
        );
    });
}

#[test]
fn vararg_default_projects_the_array_body_and_empty_omission_is_not_a_body() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/parameter-varargs.scoop"
    ));
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let export = output.export.module();
    let body = Body::from_export_hir(export, function(export, "Variadic.defaulted"), 1).unwrap();
    assert!(
        matches!(body.result(), SignatureTypeKey::NominalApplication { arguments, .. } if arguments.as_slice().len() == 1)
    );
    assert!(
        matches!(body.body().value().kind(), hir::DefaultExpressionKindV1::ArrayLiteral(elements) if elements.len() == 1)
    );
    assert!(matches!(
        Body::from_export_hir(export, function(export, "Variadic.collect"), 0),
        Err(Error::NoDefault { position: 0, .. })
    ));
}

#[test]
fn inherited_generic_body_keeps_provider_binders_and_target_substitution() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/inherited-generics.scoop"
    ));
    with_hir_source(source, |output, _| {
        let export = output.output().export.module();
        let body =
            Body::from_dependency_hir(output, function(export, "GenericChild.choose"), 1).unwrap();
        let parent =
            Body::from_dependency_hir(output, function(export, "GenericParent.choose"), 1).unwrap();
        assert_ne!(body.owner(), parent.owner());
        assert_eq!(body.definition_root(), parent.definition_root());
        assert_eq!(body.body(), parent.body());
        assert_eq!(
            body.result(),
            &SignatureTypeKey::Binder { depth: 0, index: 0 }
        );
        let [SignatureTypeKey::Tuple(arguments)] = body.type_parameters().arguments() else {
            panic!("one tuple substitution for the provider binder")
        };
        assert_eq!(
            arguments.as_slice(),
            &[
                SignatureTypeKey::Binder { depth: 0, index: 0 },
                SignatureTypeKey::Binder { depth: 0, index: 0 }
            ]
        );
    });
}

#[test]
fn source_body_rejects_corrupt_provider_binder_identity() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let owner = function(export, "Base.generic");
        let interface = export
            .source_parameter_interfaces
            .iter()
            .find(|source| source.owner == owner)
            .unwrap();
        let source = calling_source(interface.parameters[1].calling).unwrap();
        let expression = export.export_default_sources[source].expression;
        let mut corrupted = export.clone();
        corrupted.export_default_exprs[expression]
            .type_parameters
            .clear();
        assert!(matches!(
            Body::from_export_hir(&corrupted, owner, 1),
            Err(Error::Body(
                hir::DefaultTemplateEnvelopeProjectionError::TypeParameterArity {
                    expected: 1,
                    actual: 0
                }
            ))
        ));
        let mut corrupted = export.clone();
        corrupted.export_default_sources[source]
            .type_arguments
            .clear();
        assert!(matches!(
            Body::from_export_hir(&corrupted, owner, 1),
            Err(Error::Body(
                hir::DefaultTemplateEnvelopeProjectionError::TypeParameterArity {
                    expected: 1,
                    actual: 0
                }
            ))
        ));
    });
}
