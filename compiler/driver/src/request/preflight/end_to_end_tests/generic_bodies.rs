use super::*;

mod abstracts;
mod adapters;
mod argument_inference;
mod argument_materialization;
mod arrays;
mod bindings;
mod bound_properties;
mod bounds;
mod call_probes;
mod callable_order;
mod callable_signatures;
mod concrete_calls;
mod constructor_applications;
mod constructor_bodies;
mod constructor_requests;
mod constructors;
mod coroutines;
mod declaration_views;
mod delegates;
mod demanded_initialization;
mod equality;
mod globals;
mod helpers;
mod host_properties;
mod initialization;
mod iteration;
mod local_calls;
mod machine;
mod members;
mod method_calls;
mod native_addresses;
mod native_calls;
mod nominal_conditions;
mod nominals;
mod options;
mod parents;
mod pointer_construction;
mod pointers;
mod publication;
mod qualified_types;
mod ranges;
mod rebuilt_core;
mod reexported_namespaces;
mod reference_targets;
mod references;
mod selection;
mod shared_bounds;
mod shared_classes;
mod shared_closures;
mod shared_defaults;
mod shared_enums;
mod shared_interfaces;
mod shared_literals;
mod shared_requests;
mod shared_singletons;
mod shared_structs;
mod siblings;
mod source_calls;
mod value_layouts;

#[test]
fn ordinary_reader_retains_generic_bodies_from_actual_published_libraries() {
    let target = resolved_target().expect("generic body publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-generic-body-production");
    for case in ["standalone", "combined", "support", "concrete-support"] {
        let source = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
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
        let closure = scoop_slib::read_cross_cone_layout_artifact_closure(
            scoop_slib::CrossConeArtifactClosureInput::completed(
                identity,
                target.lir_target_selection(),
                vec![ConeIdentity::CORE],
                vec![&core_bytes],
                &bytes,
            ),
            target.c_bridge_toolchain().profile(),
        )
        .unwrap();
        let (sections, _) = closure.artifact(identity).unwrap();
        let interface = sections.hir_interface();
        let bodies = interface.generic_callable_bodies().records();
        assert!(
            bodies.iter().any(|body| matches!(
                body.owner(),
                scoop_hir::DefaultCallableDeclarationV1::GenericFunction(_)
            )),
            "{case}"
        );
        if case == "standalone" {
            assert_eq!(bodies.len(), 3);
            assert_eq!(interface.callable_interfaces().support_records().len(), 2);
        }
        if case == "combined" {
            assert!(bodies.iter().any(|body| !body.capture_types().is_empty()));
        }
        if case == "support" {
            assert_eq!(interface.nominal_interfaces().support_records().len(), 1);
            assert_eq!(interface.public_bindings().records().len(), 1);
        }
        if case == "concrete-support" {
            assert_eq!(interface.nominal_interfaces().support_records().len(), 1);
            assert_eq!(interface.public_bindings().records().len(), 1);
            assert_eq!(bodies.len(), 1);
            assert_eq!(
                sections
                    .hir_type_semantics()
                    .representation_support()
                    .records()
                    .len(),
                1,
            );
            for (kind, stage) in [(StageDumpKind::Mir, "mir"), (StageDumpKind::Lir, "lir")] {
                let mut request = build_manifest_request(
                    sysroot.path(),
                    &target,
                    &root,
                    &sysroot.path().join(format!("output/{case}-{stage}.slib")),
                    Vec::new(),
                    Vec::new(),
                );
                request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
                let output = request.build_and_publish().unwrap();
                let actual = output.emitted_dumps().first().unwrap().text();
                let snapshot = directory.join(format!("{case}.{stage}.snap"));
                if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                    std::fs::write(&snapshot, actual).unwrap();
                }
                assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            }
        }
    }
}
