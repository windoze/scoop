use super::*;

mod accessor_forms;
mod interior_mutability;

#[test]
fn formal_publication_preserves_private_storage_and_nested_declaration_support() {
    let target =
        resolved_target().expect("shared nominal publication requires the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    for (case, public_count, support_count) in [("standalone", 1, 1), ("published", 2, 4)] {
        let source = std::fs::read_to_string(crate::workspace_root().join(format!(
            "tests/fixtures/m23-shared-nominal-declarations/{case}.scoop"
        )))
        .unwrap();
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
        let table = closure
            .current_compile()
            .production()
            .hir_interface()
            .nominal_interfaces();
        assert_eq!(table.records().len(), public_count);
        assert_eq!(table.support_records().len(), support_count);
        for record in table.support_records() {
            assert!(table.get(record.declaration()).is_none());
            assert!(record.constructors().is_empty());
            assert!(!record.declaration_details().constructors().is_empty());
            assert!(record.members().members().is_empty());
            assert!(record.nested_bindings().is_empty());
        }
        if case == "standalone" {
            let support = &table.support_records()[0];
            let field = &table.records()[0].source_shape().declared_fields()[0];
            let scoop_identity::SignatureTypeKey::Nominal(owner) = field.value_type() else {
                panic!("the private storage has a concrete nominal type")
            };
            assert_eq!(
                support.declaration(),
                scoop_hir::SourceNominalId::Concrete(*owner)
            );
            assert_eq!(support.source_shape().declared_fields().len(), 1);
        }
    }
}
