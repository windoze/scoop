use super::*;
use hir::concrete;
use source_dispatch::with_hir_source;

mod initialization;

fn fixture(case: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tests/fixtures/m23-source-only-nominals/{case}.scoop"
        )),
    )
    .unwrap()
}

fn nominal_names(local: &concrete::Module) -> BTreeSet<&str> {
    local
        .structs
        .iter()
        .map(|(_, nominal)| nominal.name.as_str())
        .chain(local.enums.iter().map(|(_, nominal)| nominal.name.as_str()))
        .chain(
            local
                .classes
                .iter()
                .map(|(_, nominal)| nominal.name.as_str()),
        )
        .chain(
            local
                .interfaces
                .iter()
                .map(|(_, nominal)| nominal.name.as_str()),
        )
        .collect()
}

#[test]
fn automatic_nominal_roots_do_not_materialize_source_only_members_or_constructors() {
    let mut snapshot = String::new();
    for case in ["standalone", "combined", "shape-demand"] {
        with_hir_source(&fixture(case), |output, _| {
            let local = output.output().local.module();
            let names = nominal_names(local);
            assert!(
                names.iter().all(|name| name.starts_with("Ready")),
                "{names:?}"
            );
            assert!(local.initialization_units.is_empty());
            let mut rows = names
                .iter()
                .map(|name| format!("  type {name}\n"))
                .collect::<Vec<_>>();
            for (_, constructor) in local.class_constructors.iter() {
                rows.push(format!(
                    "  class-constructor {} arity={}\n",
                    local.classes[constructor.class].name,
                    constructor.parameters.len()
                ));
            }
            for (_, constructor) in local.struct_constructors.iter() {
                rows.push(format!(
                    "  struct-constructor {} arity={}\n",
                    local.structs[constructor.structure].name,
                    constructor.parameters.len()
                ));
            }
            for (_, function) in local.functions.iter() {
                assert!(!function.name.contains("Deferred"), "{}", function.name);
                assert_ne!(function.name, "ReadyDirect.take");
                rows.push(format!("  function {}\n", function.name));
            }
            rows.sort();
            snapshot.push_str(&format!("{case}\n{}", rows.concat()));
        });
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-source-only-nominals/automatic.hir.snap");
    if std::env::var_os("SCOOP_UPDATE_AUTOMATIC_SNAPSHOTS").is_some() {
        std::fs::write(&path, &snapshot).unwrap();
    }
    assert_eq!(snapshot, std::fs::read_to_string(path).unwrap());
}

#[test]
fn actual_local_uses_request_complete_source_only_instances() {
    with_hir_source(&fixture("actual-demand"), |output, _| {
        let local = output.output().local.module();
        let names = nominal_names(local);
        assert!(names.contains("DeferredWrapper"));
        assert!(names.contains("DeferredBox"));
        assert!(!names.contains("DeferredUnused"));
        assert!(
            local
                .classes
                .iter()
                .any(|(_, class)| class.name == "DeferredWrapper")
        );
        assert!(
            local
                .class_constructors
                .iter()
                .any(|(_, constructor)| local.classes[constructor.class].name == "DeferredWrapper")
        );
        assert!(
            local
                .struct_constructors
                .iter()
                .any(|(_, constructor)| local.structs[constructor.structure].name == "DeferredBox")
        );
        assert!(
            local
                .functions
                .iter()
                .any(|(_, function)| function.name == "DeferredWrapper.read")
        );
        assert!(output.output().local.materialization().roots().is_empty());
    });
}

#[test]
fn generic_source_application_records_need_an_evaluated_use_before_materialization() {
    let source = fixture("generic-records");
    for (suffix, expected) in [
        ("", 0),
        ("public fun explicit(): Int = dormant(5)\n", 0),
        ("public fun evaluate(): Int = dormant()\n", 1),
        ("public fun evaluate(): Int = dormantBody(3)\n", 1),
    ] {
        with_hir_source(&format!("{source}\n{suffix}"), |output, _| {
            assert!(!output.output().export.instantiations.is_empty());
            let count = output
                .output()
                .local
                .functions
                .iter()
                .filter(|(_, function)| function.name == "requested")
                .count();
            assert_eq!(count, expected, "{suffix}");
        });
    }
}

#[test]
fn unused_source_only_bodies_still_receive_definition_site_type_diagnostics() {
    let source = fixture("errors/invalid-unused-body");
    let diagnostics =
        lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()]).unwrap_err();
    let start = source.find("\"invalid\"").unwrap() as u32;
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.span == Some(ast::Span::new(start, start + 9)))
        .unwrap_or_else(|| panic!("{diagnostics:?}"));
    assert_eq!(diagnostic.file, 1);
    assert!(
        diagnostic.message.contains("Int") && diagnostic.message.contains("String"),
        "{diagnostic:?}"
    );
}
