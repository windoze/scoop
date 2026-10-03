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

fn current_nominal(local: &concrete::Module, origin: &hir::HirNominalIdentity) -> bool {
    origin
        .source()
        .is_some_and(|source| source.declaration().origin() == local.cone)
}

fn nominal_names(local: &concrete::Module) -> BTreeSet<&str> {
    local
        .structs
        .iter()
        .map(|(_, nominal)| (&nominal.origin, nominal.name.as_str()))
        .chain(
            local
                .enums
                .iter()
                .map(|(_, nominal)| (&nominal.origin, nominal.name.as_str())),
        )
        .chain(
            local
                .classes
                .iter()
                .map(|(_, nominal)| (&nominal.origin, nominal.name.as_str())),
        )
        .chain(
            local
                .interfaces
                .iter()
                .map(|(_, nominal)| (&nominal.origin, nominal.name.as_str())),
        )
        .chain(
            local
                .objects
                .iter()
                .map(|(_, nominal)| (&nominal.origin, nominal.name.as_str())),
        )
        .filter_map(|(origin, name)| current_nominal(local, origin).then_some(name))
        .collect()
}

#[test]
fn automatic_nominal_roots_materialize_closed_storage_and_generic_parents() {
    for case in ["standalone", "combined", "shape-demand"] {
        with_hir_source(&fixture(case), |output, _| {
            let local = output.output().local.module();
            let names = nominal_names(local);
            let expected: &[&str] = match case {
                "standalone" => &["DeferredGeneric", "DeferredRoot", "ReadyValue"],
                "combined" => &[
                    "DeferredBox",
                    "DeferredConstructor",
                    "DeferredCycleA",
                    "DeferredCycleB",
                    "DeferredDerived",
                    "DeferredGeneric",
                    "DeferredHolder",
                    "DeferredLeaf",
                    "DeferredProtected.DeferredNested",
                    "DeferredObject",
                    "DeferredPrivate",
                    "DeferredPrivateHolder",
                    "DeferredProtected",
                    "DeferredRoot",
                    "DeferredSlot",
                    "DeferredVirtual",
                    "ReadyCycleA",
                    "ReadyCycleB",
                    "ReadyDirect",
                    "ReadyEmpty",
                ],
                "shape-demand" => &[
                    "DeferredBase",
                    "DeferredDerived",
                    "DeferredEnum",
                    "DeferredGeneric",
                    "DeferredGetter",
                    "ReadyOuter.DeferredNested",
                    "DeferredSetter",
                    "DeferredStorage",
                    "DeferredSuspend",
                    "ReadyDirect",
                    "ReadyOuter",
                    "ReadyPrivateConstructor",
                ],
                _ => unreachable!(),
            };
            assert_eq!(names, expected.iter().copied().collect(), "{case}");
            assert_eq!(
                local.initialization_units.len(),
                usize::from(case == "combined")
            );
        });
    }
}

#[test]
fn public_roots_and_actual_calls_request_complete_applications() {
    with_hir_source(&fixture("actual-demand"), |output, _| {
        let local = output.output().local.module();
        let names = nominal_names(local);
        assert!(names.contains("DeferredWrapper"));
        assert!(names.contains("DeferredBox"));
        assert!(names.contains("DeferredUnused"));
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
        assert!(
            local
                .functions
                .iter()
                .any(|(_, function)| function.name == "DeferredAccessorBox.$get$value")
        );
        let roots = output.output().local.materialization().roots();
        let names = roots
            .iter()
            .map(|root| match root.declaration().name() {
                scoop_identity::DeclarationName::Named(name) => name.as_str(),
                _ => panic!("named source root"),
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(names, BTreeSet::from(["DeferredUnused", "DeferredWrapper"]));
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
