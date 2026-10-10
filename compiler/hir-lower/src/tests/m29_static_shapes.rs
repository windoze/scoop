use super::*;
use scoop_identity::SignatureTypeKey;

mod annotation_arrays;
mod classes;
mod other_types;
mod values;

fn with_shapes(
    provider: &str,
    consumer: &str,
    verify: impl Fn(&hir::Module, &hir::ImportedSemanticWorld<'_>, bool),
) {
    let core = complete_core_file();
    let input = DefinedTestSources::try_new(
        vec![ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "<core>",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        identified_test_sources(vec![
            scoop_parser::parse(provider).unwrap(),
            scoop_parser::parse(consumer).unwrap(),
        ]),
        |_| CurrentSourceDetails {
            display_locator: "<source>",
            source_text: "",
        },
    )
    .unwrap();
    let output = lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("source shapes lower normally");
    let module = output.export.module();
    let world = hir::ImportedSemanticWorld::from_dependencies(module.cone, vec![], vec![]).unwrap();
    verify(module, &world, false);
    assert!(!hir::dump_static_shapes(module, &world).is_empty());
    super::m23_generic_body_consumption::with_provider_consumer(
        provider,
        consumer,
        |output, world, _, _, _| {
            let module = output.output().export.module();
            verify(module, world, true);
            assert!(!hir::dump_static_shapes(module, world).is_empty());
        },
    )
    .expect("the same source shapes survive separate compilation");
}

fn nominal<'a>(module: &'a hir::Module, function: &str) -> hir::StaticNominalShape<'a> {
    let ty = module
        .functions
        .values()
        .find(|value| value.name == function)
        .unwrap()
        .return_ty;
    let hir::StaticTypeShape::Nominal(shape) = module.static_type_shape(ty, &[]).unwrap() else {
        panic!("{function} returns a nominal type")
    };
    shape
}

fn signature(module: &hir::Module, ty: hir::TypeId) -> SignatureTypeKey {
    hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(module))
        .map(ty, &[])
        .unwrap()
}

fn annotation_text(
    annotations: hir::StaticAnnotations<'_>,
    module: &hir::Module,
    world: &hir::ImportedSemanticWorld<'_>,
) -> String {
    let annotation = annotations.iter().next().unwrap();
    assert_eq!(annotation.name(module, world), "Label");
    let hir::CanonicalAnnotationValueV1::Scalar(hir::CanonicalConstValueV1::String(text)) =
        &annotation.arguments[0]
    else {
        panic!("Label has a string parameter")
    };
    text.clone()
}
