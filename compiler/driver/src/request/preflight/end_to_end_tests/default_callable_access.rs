use super::*;
use scoop_hir::{DefaultBoundCallableSourceV1, ExportDefaultCallableTargetV1 as Target};
use scoop_wire::WirePath;

#[test]
fn ordinary_reader_consumes_default_callable_references_from_published_bytes() {
    let target = resolved_target().expect("callable access publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let directory = crate::workspace_root().join("tests/fixtures/m23-default-callable-access");
    for (case, expected_templates) in [("standalone", 1), ("combined", 20)] {
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
        if std::env::var_os("SCOOP_UPDATE_CALLABLE_ACCESS_SNAPSHOTS").is_some() {
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
        let mut kinds = [false; 7];
        let mut bounds = [false; 2];
        let mut accessors = false;
        let mut repeated_substitutions = false;
        for template in templates {
            for reference in template.references().callables() {
                let kind = match reference.target() {
                    Target::Callable(callee) => {
                        accessors |= matches!(
                            callee.declaration(),
                            scoop_hir::DefaultCallableDeclarationV1::PropertyAccessor(_)
                        );
                        0
                    }
                    Target::Bound(bound) => {
                        bounds[match bound.source() {
                            DefaultBoundCallableSourceV1::Class { .. } => 0,
                            DefaultBoundCallableSourceV1::Interface { .. } => 1,
                        }] = true;
                        1
                    }
                    Target::DerivedEquality { .. } => 2,
                    Target::LocalFunction { .. } => 3,
                    Target::Lambda { .. } => 4,
                    Target::AnonymousFunction { .. } => 5,
                    Target::CallableReference { .. } => 6,
                    Target::FunctionAddress { .. } => continue,
                };
                kinds[kind] = true;
            }
            let nested = template.index_nested_callables(&WirePath::root()).unwrap();
            let mut seen = Vec::new();
            for occurrence in nested.occurrences() {
                let descriptor = occurrence.descriptor();
                repeated_substitutions |= seen.iter().any(|(identity, previous)| {
                    *identity == descriptor.identity() && *previous != descriptor.function_type()
                });
                seen.push((descriptor.identity(), descriptor.function_type()));
            }
        }
        assert!(kinds[0]);
        if case == "combined" {
            assert!(kinds.into_iter().all(|present| present), "{kinds:?}");
            assert!(bounds.into_iter().all(|present| present), "{bounds:?}");
            assert!(accessors && repeated_substitutions);
        }
    }
}
