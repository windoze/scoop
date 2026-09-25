use super::*;

#[test]
fn source_reference_wire_requires_all_six_sequences_and_three_record_fields() {
    let empty = References::try_new(vec![], vec![], vec![], vec![], vec![], vec![]).unwrap();
    let bytes = encode(&empty).unwrap();
    assert_eq!(
        bytes,
        [0xa6, 1, 0x80, 2, 0x80, 3, 0x80, 4, 0x80, 5, 0x80, 6, 0x80]
    );
    let decoded: Decoded = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded
            .resolve(
                &mut scoop_identity::PendingIdentityValidation::new()
                    .finish()
                    .unwrap()
            )
            .unwrap(),
        empty
    );
    for count in [0xa5, 0xa7] {
        let mut bad = bytes.clone();
        bad[0] = count;
        assert!(decode_canonical::<Decoded>(&bad).is_err());
    }
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "callable"),
            0,
        )
        .unwrap();
        let record = encode(&body.references().callables()[0]).unwrap();
        for count in [0xa2, 0xa4] {
            let mut bad = record.clone();
            bad[0] = count;
            assert!(
                decode_canonical::<
                    hir::DecodedDefaultSourceReferenceV1<hir::DecodedExportDefaultCallableTargetV1>,
                >(&bad)
                .is_err()
            );
        }
    });
}

#[test]
fn source_reference_reader_retains_even_identical_occurrences_in_encoded_order() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "combined"),
            0,
        )
        .unwrap();
        let refs = body.references();
        let callables = vec![
            refs.callables()[1].clone(),
            refs.callables()[0].clone(),
            refs.callables()[0].clone(),
        ];
        let reordered =
            References::try_new(callables, vec![], vec![], vec![], vec![], vec![]).unwrap();
        let bytes = encode(&reordered).unwrap();
        let decoded: Decoded = decode_canonical(&bytes).unwrap();
        let restored = decoded.resolve(&mut identity_closure(output)).unwrap();
        assert_eq!(restored, reordered);
        assert_eq!(restored.callables()[1], restored.callables()[2]);
        assert_ne!(restored.callables()[0], restored.callables()[1]);
    });
}

#[test]
fn source_reference_reader_rejects_unknown_target_identity() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "callable"),
            0,
        )
        .unwrap();
        let decoded: Decoded = decode_canonical(&encode(body.references()).unwrap()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        let error = decoded.resolve(&mut empty).unwrap_err();
        assert!(
            matches!(
                error,
                hir::DefaultSourceReferencesResolutionError::Record {
                    kind: hir::ExportDefaultReferenceKindV1::Callable,
                    index: 0,
                    error: hir::DefaultSourceReferenceResolutionError::Target(_)
                }
            ),
            "{error:?}"
        );
    });
}

#[test]
fn source_reference_constructor_cannot_masquerade_as_a_local_function() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "constructor"),
            0,
        )
        .unwrap();
        let reference = &body.references().constructors()[0];
        let hir::DefaultConstructorRefV1::Struct { declaration, .. } = reference.target() else {
            panic!("struct constructor required")
        };
        let wrong = hir::DefaultSourceReferenceV1::new(
            hir::ExportDefaultCallableTargetV1::LocalFunction {
                declaration: scoop_identity::CallableTemplateOrigin::Constructor(*declaration),
            },
            reference.definition_origin().clone(),
            reference.witness().clone(),
        );
        assert!(matches!(
            References::try_new(vec![wrong.clone()], vec![], vec![], vec![], vec![], vec![]),
            Err(hir::DefaultSourceReferencesBuildError::CallableTarget { index: 0, .. })
        ));
        let bytes = [
            vec![0xa6, 1, 0x81],
            encode(&wrong).unwrap(),
            vec![2, 0x80, 3, 0x80, 4, 0x80, 5, 0x80, 6, 0x80],
        ]
        .concat();
        let decoded: Decoded = decode_canonical(&bytes).unwrap();
        let error = decoded.resolve(&mut identity_closure(output)).unwrap_err();
        assert!(
            matches!(
                error,
                hir::DefaultSourceReferencesResolutionError::Record {
                    kind: hir::ExportDefaultReferenceKindV1::Callable,
                    index: 0,
                    error: hir::DefaultSourceReferenceResolutionError::Target(
                        hir::ExportDefaultReferenceTargetResolutionError::Callable(
                            hir::ExportDefaultCallableTargetResolutionError::Shape(_)
                        )
                    )
                }
            ),
            "{error:?}"
        );
    });
}
