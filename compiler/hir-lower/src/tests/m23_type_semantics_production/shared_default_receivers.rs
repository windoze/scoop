use super::*;
use scoop_identity::SignatureTypeKey;
use scoop_wire::{decode_canonical, encode};
use source_dispatch::with_hir_source;
use source_inventory::identity_closure;

#[test]
fn shared_template_preserves_generic_inherited_binder_substitution_in_bytes() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/inherited-generics.scoop"
    ));
    with_hir_source(source, |output, _| {
        let templates =
            hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(output).unwrap();
        let bytes = encode(&templates.index_locals().unwrap()).unwrap();
        let decoded: hir::DecodedCanonicalExportDefaultTemplatesV1 =
            decode_canonical(&bytes).unwrap();
        let restored = decoded.resolve(&mut identity_closure(output)).unwrap();
        assert_eq!(templates, restored);
        let parent = template(&restored, output, "GenericParent.choose", 1);
        let child = template(&restored, output, "GenericChild.choose", 1);
        assert_eq!(parent.definition_root(), child.definition_root());
        assert_eq!(parent.result(), child.result());
        assert_eq!(parent.body(), child.body());
        assert_ne!(parent.type_parameters(), child.type_parameters());
        let SignatureTypeKey::Tuple(elements) = &child.type_parameters().arguments()[0] else {
            panic!("tuple substitution required")
        };
        assert_eq!(
            elements.as_slice(),
            &[
                SignatureTypeKey::Binder { depth: 0, index: 0 },
                SignatureTypeKey::Binder { depth: 0, index: 0 },
            ]
        );
    });
}

#[test]
fn public_template_bytes_preserve_provider_receivers_and_binder_mappings() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/public-provider-receivers.scoop"
    ));
    with_hir_source(source, |output, _| {
        let templates =
            hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(output).unwrap();
        let bytes = encode(&templates.index_locals().unwrap()).unwrap();
        let decoded: hir::DecodedCanonicalExportDefaultTemplatesV1 =
            decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let restored = decoded.resolve(&mut identity_closure(output)).unwrap();
        assert_eq!(templates, restored);
        let callables =
            hir::CanonicalCallableInterfacesV1::from_export_hir(&output.output().export).unwrap();
        for (parent, child) in [
            ("ProviderBase.pick", "ProviderChild.pick"),
            ("ProviderGeneric.pick", "ProviderApplied.pick"),
        ] {
            let original = template(&restored, output, parent, 1);
            let inherited = template(&restored, output, child, 1);
            let public_key = hir::ExportDefaultTemplateKeyV1::new(inherited.key().owner(), 1);
            let public = restored.get(public_key).unwrap();
            assert_eq!(public.definition_root(), original.definition_root());
            assert_eq!(public.receiver(), original.receiver());
            let provider = original.receiver().receiver().unwrap().value_type();
            public
                .receiver()
                .validate_provider_semantics(Some(provider), public.locals())
                .unwrap();
            let callable = callables.get(public.key().owner()).unwrap();
            assert!(
                callable.receiver().is_none(),
                "member receivers are implicit"
            );
            let publishing = match callable.owner() {
                hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(id)) => {
                    SignatureTypeKey::Nominal(id)
                }
                hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(
                    origin,
                )) => SignatureTypeKey::NominalApplication {
                    origin,
                    arguments: scoop_identity::NonEmptyVec::from_first(
                        SignatureTypeKey::Binder { depth: 0, index: 0 },
                        [],
                    ),
                },
                owner => panic!("expected a nominal owner, got {owner:?}"),
            };
            assert_ne!(provider, &publishing);
            assert!(matches!(
                public
                    .receiver()
                    .validate_provider_semantics(Some(&publishing), public.locals()),
                Err(hir::TemplateReceiverSemanticValidationError::CallableType { .. })
            ));
        }
        for (name, position, expected) in [
            (
                "ProviderGeneric.copy",
                2,
                vec![
                    SignatureTypeKey::Binder { depth: 1, index: 0 },
                    SignatureTypeKey::Binder { depth: 0, index: 0 },
                ],
            ),
            (
                "ProviderGeneric.Static.copy",
                1,
                vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
            ),
        ] {
            let original = template(&restored, output, name, position);
            let key = hir::ExportDefaultTemplateKeyV1::new(original.key().owner(), position);
            let public = restored.get(key).unwrap();
            assert_eq!(public.definition_root().declaration(), public.key().owner());
            assert_eq!(public.type_parameters().arguments(), expected);
        }
    });
}

fn template<'a>(
    templates: &'a hir::CanonicalExportDefaultTemplatesV1,
    output: &hir::DependencyHirOutput,
    name: &str,
    position: u32,
) -> &'a hir::ExportDefaultTemplateV1 {
    let export = output.output().export.module();
    let (function, _) = export
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .unwrap();
    let declaration = match export.function_identities[function]
        .source_identity()
        .unwrap()
    {
        hir::HirSourceFunctionIdentity::Plain(record) => {
            scoop_identity::CallableTemplateOrigin::Function(record.id())
        }
        hir::HirSourceFunctionIdentity::Generic(record) => {
            scoop_identity::CallableTemplateOrigin::GenericFunction(record.id())
        }
    };
    templates
        .get(hir::ExportDefaultTemplateKeyV1::new(declaration, position))
        .unwrap()
}
