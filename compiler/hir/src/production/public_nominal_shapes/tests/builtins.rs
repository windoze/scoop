use super::*;

#[test]
fn builtin_shapes_belong_only_to_their_declared_provider_without_source_bindings() {
    let public = CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap();
    let identities = HirExportBindingIdentities::canonicalize(Vec::new()).unwrap();
    let direct = CanonicalDirectPublicSurfaceV1::from_public_bindings(&public).unwrap();
    let nominals = crate::CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap();
    let callables = crate::CanonicalCallableInterfacesV1::default();
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(
            LANGUAGE_BUILTINS
                .map(CoreBuiltinNominal::identity_record)
                .to_vec(),
        )
        .unwrap();
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let projected =
            PublicNominalShapeRequirementsV1::from_public_bindings(provider, &public, &identities)
                .unwrap();
        let decoded = PublicNominalShapeRequirementsV1::from_shared_surface(
            provider,
            &direct,
            &foundation,
            &nominals,
            &callables,
        )
        .unwrap();
        assert_eq!(projected, decoded);
        let mut expected = if provider == ConeIdentity::CORE {
            LANGUAGE_BUILTINS
                .map(|builtin| builtin.identity_record().id())
                .to_vec()
        } else {
            Vec::new()
        };
        expected.sort_unstable();
        assert_eq!(
            decoded
                .roots()
                .iter()
                .map(|root| root.source())
                .collect::<Vec<_>>(),
            expected
        );
        for declaration in decoded.source_declarations(&foundation).unwrap() {
            assert_eq!(declaration.origin(), provider);
        }
    }
}

#[test]
fn builtin_shapes_require_the_canonical_foundation_keys() {
    let direct = CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap();
    for missing in LANGUAGE_BUILTINS {
        let mut foundation = CanonicalHirFoundation::empty();
        foundation
            .set_types(
                LANGUAGE_BUILTINS
                    .into_iter()
                    .filter(|builtin| *builtin != missing)
                    .map(CoreBuiltinNominal::identity_record)
                    .chain([CborIdentityRecord::from_key(nominal(
                        ConeIdentity::SINGLE_FILE,
                        "Unit",
                        0,
                    ))
                    .unwrap()])
                    .collect(),
            )
            .unwrap();
        assert_eq!(
            PublicNominalShapeRequirementsV1::from_direct_surface(
                ConeIdentity::CORE,
                &direct,
                &foundation,
            ),
            Err(PublicNominalShapeProjectionError::MissingSourceNominal(
                missing.identity_record().id()
            ))
        );
    }
}
