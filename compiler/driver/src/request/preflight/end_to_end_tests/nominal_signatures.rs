use super::*;

#[test]
fn formal_callable_exports_resolve_local_and_dependency_nominals_in_one_scope() {
    let Some(target) = resolved_target() else {
        return;
    };
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
            .filter_map(|callable| classifier.classify_callable(callable).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), 2);
        assert_eq!(production.mir_cross_cone().exports().len(), expected.len());
        assert_eq!(production.lir_cross_cone().exports().len(), expected.len());
        let mut rows = Vec::new();
        for signature in expected {
            let mir = production
                .mir_cross_cone()
                .exports()
                .iter()
                .find(|export| export.declaration() == signature.declaration())
                .unwrap();
            assert_eq!(mir.signature(), signature.signature());
            let lir = production
                .lir_cross_cone()
                .export(signature.declaration())
                .unwrap();
            assert_eq!(lir.abi_signature().signature(), signature.signature());
            rows.push(format!(
                "parameters={} arguments=[{}] result={}\n",
                signature.signature().parameters().len(),
                lir.abi_signature()
                    .arguments()
                    .iter()
                    .copied()
                    .map(argument)
                    .collect::<Vec<_>>()
                    .join(", "),
                result(lir.abi_signature().result()),
            ));
        }
        rows.sort();
        let dump = rows.concat();
        if let Some(directory) = std::env::var_os("SCOOP_NOMINAL_SIGNATURE_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(Path::new(&directory).join(format!("{case}.snap")), dump).unwrap();
        } else {
            assert_eq!(
                dump,
                std::fs::read_to_string(fixtures.join(format!("{case}.snap"))).unwrap()
            );
        }
    }
}

fn argument(value: scoop_identity::ScoopAbiArgument) -> String {
    use scoop_identity::ScoopAbiArgument as Argument;
    let (kind, storage) = match value {
        Argument::ElidedZst(storage) => ("elided-zst", storage),
        Argument::Direct(storage) => ("direct", storage),
        Argument::Indirect(storage) => ("indirect", storage),
    };
    describe(kind, storage)
}

fn result(value: scoop_identity::ScoopAbiReturn) -> String {
    use scoop_identity::ScoopAbiReturn as Return;
    let (kind, storage) = match value {
        Return::UnitVoid => return "unit-void".to_owned(),
        Return::ElidedZst(storage) => ("elided-zst", storage),
        Return::Direct(storage) => ("direct", storage),
        Return::Indirect(storage) => ("indirect", storage),
    };
    describe(kind, storage)
}

fn describe(kind: &str, storage: scoop_identity::CanonicalScoopStorage) -> String {
    format!(
        "{kind}({},{},{:?})",
        storage.byte_size(),
        storage.alignment().get(),
        storage.shape()
    )
}
