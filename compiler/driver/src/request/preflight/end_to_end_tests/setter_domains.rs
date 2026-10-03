use super::*;
use scoop_hir::DeclaredVisibilityV1;
use scoop_identity::CallableTemplateOrigin;

#[test]
fn ordinary_artifact_reader_checks_setter_domains_in_restricted_and_generic_owners() {
    let target = resolved_target().expect("setter publication requires the supported target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-setter-domains");
    let source = std::fs::read_to_string(directory.join("combined.scoop")).unwrap();
    let root = sysroot.path().join("setter-domains");
    write_manifest_cone(&root, "dev.example", "setter-domains", "library", &source);
    let request = build_manifest_request(
        sysroot.path(),
        &target,
        &root,
        &sysroot.path().join("output/setter-domains.slib"),
        vec![],
        vec![],
    );
    let library = request.build_and_publish().unwrap();
    let bytes = std::fs::read(library.artifact().path()).unwrap();
    let identity = ConeCoordinate::new("dev.example", "setter-domains", "0.1.0")
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
    let interface = closure.current_compile().production().hir_interface();
    assert_eq!(interface.property_interfaces().declaration_count(), 10);
    let restricted = interface.property_interfaces().support_records();
    assert_eq!(restricted.len(), 8);
    let internal_protected = restricted
        .iter()
        .filter(|property| {
            let Some(setter) = property.accessors().setter() else {
                return false;
            };
            property.declared_visibility() == DeclaredVisibilityV1::Internal
                && interface
                    .callable_interfaces()
                    .declaration(CallableTemplateOrigin::Accessor(setter))
                    .unwrap()
                    .declared_visibility()
                    == DeclaredVisibilityV1::Protected
        })
        .count();
    assert_eq!(internal_protected, 3);
    assert!(
        interface
            .nominal_interfaces()
            .all_records()
            .any(|record| !record.type_parameters().is_empty())
    );
}
