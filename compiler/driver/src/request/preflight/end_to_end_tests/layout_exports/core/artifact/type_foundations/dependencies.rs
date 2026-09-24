//! Actual source metadata is decoded before provider-local fact replay.

use super::*;

mod constructors;
mod decoded;
mod defaults;
mod dispatch;
mod protected;
mod protocols;
use decoded::DecodedTypes;

pub(in super::super) fn check(core: CheckedSharedTypeFoundationV1<'_>) {
    let target = resolved_target().expect("type foundation fixtures require the supported target");
    let directory = tempfile::tempdir().unwrap();
    bootstrap_core(directory.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-shared-type-foundations");
    let provider_root = directory.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "type-provider",
        "library",
        &std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap(),
    );
    let provider = lower(directory.path(), &target, &provider_root, vec![], &[core]);
    let checked_provider = provider.check(&[core]).unwrap();

    checked_provider
        .with_inheritance_graph(&[core], &mut meter(), |graph, _| {
            for section in [core.section(), checked_provider.section()] {
                for record in section.inheritance().records() {
                    assert_eq!(graph.get(record.owner()).unwrap().edges(), record.edges());
                }
            }
        })
        .unwrap();
    assert_ne!(checked_provider.provider(), core.provider());
    assert!(checked_provider.facts().records().iter().any(|fact| {
        fact.kind()
            == (ExactTypeKindV1::Value {
                zst: ZstStatus::ZeroSized,
            })
    }));
    assert!(
        checked_provider
            .facts()
            .records()
            .iter()
            .any(|fact| { fact.gc() == ExactTypeGcV1::ContainsManagedReferences })
    );
    assert!(
        matches!(provider.check(&[]), Err(Error::MissingProvider(provider)) if provider == core.provider())
    );
    assert!(
        matches!(provider.check(&[core, core]), Err(Error::DuplicateProvider(provider)) if provider == core.provider())
    );
    let mut dump = format!("provider={}\n", checked_provider.provider());
    for fact in checked_provider.facts().records() {
        dump.push_str(&format!(
            "fact {} {:?} {:?}\n",
            fact.exact(),
            fact.kind(),
            fact.gc()
        ));
    }
    for record in checked_provider.section().inheritance().records() {
        dump.push_str(&format!(
            "inheritance {} {:?} {:?} {:?}\n",
            record.owner(),
            record.edges().modality(),
            record.edges().direct_base(),
            record.edges().direct_interfaces()
        ));
    }
    let snapshot = fixtures.join("provider.snap");
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
    constructors::check(core, directory.path(), &target, &fixtures);
    dispatch::check(core, directory.path(), &target, &fixtures);
    protected::check(core, directory.path(), &target, &fixtures);
    protocols::check(core, directory.path(), &target, &fixtures);
    defaults::check(core, directory.path(), &target, &fixtures);
}

fn lower(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    direct: Vec<std::path::PathBuf>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
) -> DecodedTypes {
    let loaded = build_manifest_request(
        sysroot,
        target,
        root,
        &root.join("output.slib"),
        direct,
        vec![],
    )
    .load_preflight(DecodeLimits::default())
    .unwrap();
    let request = loaded.validate().unwrap();
    let parsed = request.parse_current_sources().unwrap();
    let ValidatedCompilerProtocols::Imported(inputs) = request.protocols() else {
        panic!("ordinary source imports its actual language declarations")
    };
    let world = request
        .dependencies()
        .semantic()
        .imported_semantic_world()
        .unwrap();
    let hir = current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        parsed.sources(),
        inputs.as_ref().clone().into(),
        &world,
    )
    .unwrap();
    let ValidatedCurrentConeInput::Manifest { manifest } = request.current() else {
        panic!("the fixture uses a manifest Cone")
    };
    DecodedTypes::read(hir, manifest.coordinate(), dependencies)
}
