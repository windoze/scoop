//! An Any parameter does not authorize a structural boxing definition.

use super::*;

pub(super) fn check(sysroot: &Path, target: &scoop_toolchain::ResolvedTargetProfile) {
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-any-call-signatures");
    let source = std::fs::read_to_string(fixtures.join("structural-box.scoop")).unwrap();
    let root = sysroot.join("structural-box");
    write_manifest_cone(&root, "dev.example", "structural-box", "library", &source);
    let loaded = build_manifest_request(
        sysroot,
        target,
        &root,
        &root.join("output.slib"),
        vec![],
        vec![],
    )
    .load_preflight(DecodeLimits::default())
    .unwrap();
    let request = loaded.validate().unwrap();
    let parsed = request.parse_current_sources().unwrap();
    let ValidatedCompilerProtocols::Imported(protocols) = request.protocols() else {
        panic!("the source imports its actual language declarations")
    };
    let closure = request.dependencies().semantic();
    let world = closure.imported_semantic_world().unwrap();
    let hir = current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        parsed.sources(),
        protocols.as_ref().clone().into(),
        &world,
    )
    .unwrap();
    let selected = closure
        .project_dependency_callables_to_mir(&hir.hir)
        .unwrap();
    let error = hir
        .machine_input()
        .lower_selected_mir(selected)
        .err()
        .expect("structural box ownership requires the M23-7 ODR capability");
    assert!(
        matches!(
            error,
            crate::request::preflight::machine::CurrentConeMirStageError::Foundation(
                mir::OdrFreeMirFoundationProjectionError::Odr(_)
            )
        ),
        "{error:?}"
    );
    let dump = format!("{error}\n");
    let path = fixtures.join("structural-box.snap");
    if std::env::var_os("SCOOP_UPDATE_ANY_CALLS").is_some() {
        std::fs::write(&path, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(path).unwrap());
}
