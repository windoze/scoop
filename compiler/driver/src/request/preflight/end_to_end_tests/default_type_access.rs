use super::*;
use scoop_identity::SignatureTypeKey;

#[test]
fn ordinary_reader_consumes_default_type_references_from_published_bytes() {
    let target = resolved_target().expect("default type publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_pointer_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let mut session = scoop_identity::SemanticIdentitySession::new();
    let core_closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
        ConeIdentity::CORE,
        target.lir_target_selection(),
        vec![],
        vec![],
        &core_bytes,
        target.c_bridge_toolchain().profile(),
        &mut session,
    )
    .unwrap();
    let core_kinds = reference_kinds(
        core_closure
            .current_compile()
            .production()
            .hir_interface()
            .default_templates()
            .records(),
    );
    assert!(core_kinds[4] && core_kinds[5]);
    let directory = crate::workspace_root().join("tests/fixtures/m23-default-type-access");
    for (case, expected_templates) in [("standalone", 1), ("combined", 4)] {
        let source = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        let mut request = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{case}.slib")),
            vec![],
            vec![],
        );
        request.emit = StageDumpPolicy::Stage(StageDumpKind::Hir);
        let library = request.build_and_publish().unwrap();
        let dump = library.emitted_dump().unwrap();
        let snapshot = directory.join(format!("{case}.hir.snap"));
        if std::env::var_os("SCOOP_UPDATE_TYPE_ACCESS_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, dump.text()).unwrap();
        }
        assert_eq!(dump.text(), std::fs::read_to_string(snapshot).unwrap());
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
        let templates = closure
            .current_compile()
            .production()
            .hir_interface()
            .default_templates()
            .records();
        assert_eq!(templates.len(), expected_templates, "{case}");
        let kinds = reference_kinds(templates);
        if case == "combined" {
            assert!(kinds[..4].iter().all(|present| *present), "{kinds:?}");
        }
    }
}

fn reference_kinds(templates: &[scoop_hir::ExportDefaultTemplateV1]) -> [bool; 6] {
    let mut kinds = [false; 6];
    for reference in templates
        .iter()
        .flat_map(|template| template.references().types())
    {
        let index = match reference.target() {
            SignatureTypeKey::Nominal(_) => 0,
            SignatureTypeKey::NominalApplication { .. } => 1,
            SignatureTypeKey::Tuple(_) => 2,
            SignatureTypeKey::Function { .. } => 3,
            SignatureTypeKey::RawPointer(_) => 4,
            SignatureTypeKey::NativeFunctionPointer { .. } => 5,
            SignatureTypeKey::Binder { .. } => panic!("a bare binder is not a reference"),
        };
        kinds[index] = true;
    }
    kinds
}

fn bootstrap_pointer_core(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
) -> SingleConeProductionSuccess {
    copy_trusted_core_sources(sysroot);
    let slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    std::fs::copy(
        crate::workspace_root().join("tests/fixtures/m23-default-type-access/pointers.scoop"),
        slot.source_root().join("src/default-pointers.scoop"),
    )
    .unwrap();
    let artifact_path = slot.artifact().to_path_buf();
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(slot.source_root()),
        },
        ExplicitDependencyInputs::new(vec![], vec![]),
        TrustedCoreInput::BootstrapSelf,
        target.clone(),
        SlibOutputDestination::new(&artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish()
    .unwrap()
}
