use super::*;
use scoop_identity::{ScoopAbiArgument as Argument, ScoopAbiReturn as Return, ScoopAbiValueShape};

#[test]
fn formal_callable_exports_resolve_local_and_dependency_nominals_in_one_scope() {
    let target = resolved_target().expect("nominal signatures require the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-nominal-signature-scope");
    for case in ["standalone", "combined"] {
        let source = std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        let library = build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{case}.slib")),
        );
        let bytes = std::fs::read(library.artifact().path()).unwrap();
        let identity = ConeCoordinate::new("dev.example", case, "0.1.0")
            .unwrap()
            .identity()
            .unwrap();
        let mut session = scoop_identity::SemanticIdentitySession::new();
        let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
            identity,
            target.lir_target_selection(),
            vec![ConeIdentity::CORE],
            vec![&core_bytes],
            &bytes,
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        let production = closure.current_compile().production();
        assert_eq!(
            production
                .hir_interface()
                .nominal_interfaces()
                .support_records()
                .len(),
            usize::from(case == "combined"),
        );
        let world = closure.semantic().imported_semantic_world().unwrap();
        let classifier = world
            .nominal_exact_leaf_classifier(production.hir_interface().nominal_interfaces())
            .unwrap();
        let expected = production
            .hir_interface()
            .callable_interfaces()
            .records()
            .iter()
            .filter(|callable| callable.owner() == scoop_hir::PublicDeclarationOwnerV1::TopLevel)
            .filter_map(|callable| classifier.classify_callable(callable).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), 2);
        let mut kinds = std::collections::BTreeSet::new();
        for signature in expected {
            let mir = production
                .mir_cross_cone()
                .exports()
                .iter()
                .find(|export| export.implementation() == signature.implementation())
                .unwrap();
            assert_eq!(mir.signature(), signature.signature());
            let lir = production
                .lir_cross_cone()
                .export(signature.direct_declaration().unwrap())
                .unwrap();
            assert_eq!(lir.abi_signature().signature(), signature.signature());
            let abi = lir.abi_signature();
            let parameters = signature.signature().parameters();
            assert_eq!(abi.arguments().len(), parameters.len());
            if case == "standalone" {
                let Return::ElidedZst(storage) = abi.result() else {
                    panic!("Marker has no returned payload")
                };
                check_storage(storage, 0, 1, ScoopAbiValueShape::Aggregate);
                assert_eq!(storage.exact_type(), signature.signature().result());
                for argument in abi.arguments() {
                    assert_eq!(*argument, Argument::ElidedZst(storage));
                }
                kinds.insert(parameters.len());
            } else {
                assert_eq!(parameters.len(), 2);
                match abi.arguments() {
                    [Argument::Direct(first), Argument::Direct(second)] => {
                        for storage in [*first, *second] {
                            check_storage(storage, 8, 8, ScoopAbiValueShape::Scalar);
                        }
                        let Return::DirectParts(storage, coercion) = abi.result() else {
                            panic!("Payload uses a one-byte direct result")
                        };
                        check_storage(storage, 1, 1, ScoopAbiValueShape::Aggregate);
                        assert_eq!(storage.exact_type(), signature.signature().result());
                        assert_eq!(
                            coercion,
                            scoop_identity::AbiCoercion::One(
                                scoop_identity::AbiPart::new(
                                    scoop_identity::AbiCarrier::Integer(8),
                                    0,
                                    1,
                                    1,
                                )
                                .unwrap()
                            )
                        );
                        kinds.insert(0);
                    }
                    [
                        Argument::DirectParts(payload, coercion),
                        Argument::ElidedZst(marker),
                    ] => {
                        check_storage(*payload, 16, 8, ScoopAbiValueShape::Aggregate);
                        check_storage(*marker, 0, 1, ScoopAbiValueShape::Aggregate);
                        assert_eq!(payload.exact_type(), parameters[0]);
                        assert_eq!(marker.exact_type(), parameters[1]);
                        assert!(!coercion.has_managed_pointer());
                        assert_eq!(
                            coercion
                                .parts()
                                .iter()
                                .map(|part| part.extent())
                                .sum::<u64>(),
                            16
                        );
                        assert_eq!(abi.result(), Return::DirectParts(*payload, *coercion));
                        kinds.insert(1);
                    }
                    other => panic!("unexpected source signature: {other:?}"),
                }
            }
        }
        assert_eq!(kinds, std::collections::BTreeSet::from([0, 1]));
    }
}

fn check_storage(
    storage: scoop_identity::CanonicalScoopStorage,
    size: u64,
    alignment: u64,
    shape: ScoopAbiValueShape,
) {
    assert_eq!(storage.byte_size(), size);
    assert_eq!(storage.alignment().get(), alignment);
    assert_eq!(storage.shape(), shape);
}
