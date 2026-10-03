use super::*;

pub(super) fn with_mir(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core_bytes: &[u8],
    source: &str,
    inspect: impl FnOnce(scoop_mir_lower::MirTypeBridgeExportInputV1<'_>, &lir::SelectedExternalLirSet),
) {
    let root = sysroot.join("shape-consumer");
    write_manifest_cone(&root, "dev.example", "shape-consumer", "library", source);
    let loaded = build_manifest_request(
        sysroot,
        target,
        &root,
        &root.join("output.slib"),
        vec![],
        vec![],
    )
    .load_preflight()
    .unwrap();
    let request = loaded.validate().unwrap();
    let parsed = request.parse_current_sources().unwrap();
    let ValidatedCompilerProtocols::Imported(protocols) = request.protocols() else {
        panic!("the source imports the real provider declarations")
    };
    let closure = request.dependencies().semantic();
    let world = closure.imported_semantic_world().unwrap();
    let hir = current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        parsed.sources(),
        protocols.as_ref().clone().into(),
        &world,
        Default::default(),
    )
    .unwrap();
    let selected = closure
        .project_dependency_callables_to_mir(&hir.hir)
        .unwrap();
    let mir = hir.machine_input().lower_selected_mir(selected).unwrap();
    let callables = closure
        .project_dependency_callables_to_lir(mir.strong.selected_callables())
        .unwrap();
    let read = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            ConeIdentity::CORE,
            target.lir_target_selection(),
            vec![],
            vec![],
            core_bytes,
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    let (core, _) = read.artifact(ConeIdentity::CORE).unwrap();
    let (identities, _, core_hir) =
        support::identity::identities(&hir.hir, &mir.strong, None, core);
    let foundation = hir::CanonicalHirFoundation::from_type_semantics_output(&hir.hir).unwrap();
    let core = closure.direct_provider(ConeIdentity::CORE).unwrap();
    let source = scoop_hir_lower::produce_cross_cone_type_semantics(
        &hir.hir,
        hir::SharedTypeMetadataV1 {
            provider: mir.strong.module().cone,
            identities: &identities,
            foundation: &foundation,
            public: &hir.cross_cone_section,
        },
        &[hir::SharedTypeMetadataV1 {
            provider: core.identity(),
            identities: &identities,
            foundation: &core_hir,
            public: core.production().hir_interface(),
        }],
    )
    .unwrap();
    inspect(
        scoop_mir_lower::MirTypeBridgeExportInputV1 {
            hir: &hir.hir,
            public: &hir.cross_cone_section,
            source: &source,
            mir: &mir.strong,
            ordinary: &mir.public,
            dependency_objects: mir.strong.selected_callables().objects(),
            identities: &identities,
        },
        &callables,
    );
}
