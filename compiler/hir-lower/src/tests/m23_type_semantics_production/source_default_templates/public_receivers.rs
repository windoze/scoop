use super::*;
use scoop_identity::SignatureTypeKey;

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
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let restored = decoded.resolve(&mut identity_closure(output)).unwrap();
        assert_eq!(templates, restored);
        let callables =
            hir::CanonicalCallableInterfacesV1::from_export_hir(&output.output().export).unwrap();
        for (parent, child) in [
            ("ProviderBase.pick", "ProviderChild.pick"),
            ("ProviderGeneric.pick", "ProviderApplied.pick"),
        ] {
            let original = template(output, parent, 1);
            let inherited = template(output, child, 1);
            let public_key = hir::ExportDefaultTemplateKeyV1::new(inherited.key().owner(), 1);
            let public = restored.get(public_key).unwrap();
            assert_eq!(public.definition_root(), original.definition_root());
            assert_eq!(public.receiver(), original.receiver());
            assert_eq!(public.type_parameters(), inherited.type_parameters());
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
            let original = template(output, name, position);
            let key = hir::ExportDefaultTemplateKeyV1::new(original.key().owner(), position);
            let public = restored.get(key).unwrap();
            assert_eq!(public.definition_root().declaration(), public.key().owner());
            assert_eq!(public.type_parameters().arguments(), expected);
        }
    });
}
