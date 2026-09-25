use super::*;
use scoop_identity::{NonEmptyVec, SignatureTypeKey};

#[test]
fn source_bytes_keep_the_original_provider_receiver_through_inherited_substitution() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/provider-receivers.scoop"
    ));
    with_hir_source(source, |output, _| {
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
            let direct = round_trip(output, &template(output, name, position));
            assert_eq!(direct.key().owner(), direct.definition_root().declaration());
            assert_eq!(direct.type_parameters().arguments(), expected);
        }
        for (parent, child, generic) in [
            ("ProviderBase.pick", "ProviderChild.pick", false),
            ("ProviderGeneric.pick", "ProviderApplied.pick", true),
        ] {
            let parent = round_trip(output, &template(output, parent, 1));
            let child = round_trip(output, &template(output, child, 1));
            assert_ne!(parent.key(), child.key());
            assert_eq!(parent.definition_root(), child.definition_root());
            assert_eq!(parent.receiver(), child.receiver());
            assert_eq!(parent.locals(), child.locals());
            let expected = parent.receiver().receiver().unwrap().value_type();
            child
                .receiver()
                .validate_provider_semantics(Some(expected), child.locals())
                .unwrap();
            let declarations = hir::CanonicalNominalSourceCallablesV1::from_export_hir(
                &output.output().export,
                &std::collections::BTreeSet::from([child.key().owner()]),
            )
            .unwrap();
            let publishing = match declarations
                .get(child.key().owner())
                .unwrap()
                .payload()
                .owner()
            {
                hir::SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
                hir::SourceNominalId::GenericTemplate(origin) => {
                    SignatureTypeKey::NominalApplication {
                        origin,
                        arguments: NonEmptyVec::from_first(
                            SignatureTypeKey::Binder { depth: 0, index: 0 },
                            [],
                        ),
                    }
                }
            };
            assert_ne!(expected, &publishing);
            assert!(matches!(
                child
                    .receiver()
                    .validate_provider_semantics(Some(&publishing), child.locals()),
                Err(hir::TemplateReceiverSemanticValidationError::CallableType { .. })
            ));
            if generic {
                assert!(
                    matches!(&child.type_parameters().arguments()[0], SignatureTypeKey::Tuple(elements) if elements.as_slice() == [SignatureTypeKey::Binder { depth: 0, index: 0 }, SignatureTypeKey::Binder { depth: 0, index: 0 }])
                );
            } else {
                assert!(child.type_parameters().is_empty());
            }
        }
    });
}
