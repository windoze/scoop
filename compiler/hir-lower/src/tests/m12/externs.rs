use super::*;

#[test]
fn extern_functions_have_typed_identity_and_abi_specific_effects() {
    let c = extern_fun(
        "nativeAdd",
        vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("numbers", "native_add", "c"),
    );
    let scoop = extern_fun(
        "nativeWrite",
        vec![("message", ty_named("String"))],
        None,
        extern_annotation("", "scoop_rt_write", "scoop"),
    );
    let module = lower_user(file(vec![c, scoop, fun("main", vec![])]))
        .expect("both extern ABI categories must lower");
    let (_, c) = module
        .extern_functions
        .iter()
        .find(|(_, function)| function.source_name == "nativeAdd")
        .expect("the user C extern has a typed entity");
    assert_eq!(c.abi, hir::ExternAbi::C);
    assert_eq!(c.safety, hir::Safety::Unsafe);
    assert_eq!(c.gc_effect, hir::GcEffect::NoGc);
    let (_, scoop) = module
        .extern_functions
        .iter()
        .find(|(_, function)| function.source_name == "nativeWrite")
        .expect("the user Scoop extern has a typed entity");
    assert_eq!(scoop.abi, hir::ExternAbi::Scoop);
    assert_eq!(scoop.safety, hir::Safety::Safe);
    assert_eq!(scoop.gc_effect, hir::GcEffect::Managed);
    let dump = hir::dump(&module);
    assert!(dump.contains("abi=c symbol=native_add lib=numbers>"));
    assert!(dump.contains("abi=scoop symbol=scoop_rt_write>"));
}

#[test]
fn scoop_extern_accepts_fully_concrete_value_aggregates_with_managed_leaves() {
    let aggregate = struct_decl(
        "ManagedAggregate",
        vec![
            ("value", ty_named("String")),
            ("left", ty_named("Long")),
            ("right", ty_named("Long")),
        ],
    );
    let round_trip = extern_fun(
        "nativeAggregateRoundTrip",
        vec![("value", ty_named("ManagedAggregate"))],
        Some(ty_named("ManagedAggregate")),
        extern_annotation("fixture_native", "native_aggregate_round_trip", "scoop"),
    );
    let module = lower_user(file(vec![aggregate, round_trip, fun("main", vec![])]))
        .expect("Scoop ABI reuses the ordinary typed ABI for managed value aggregates");
    let (_, external) = module
        .extern_functions
        .iter()
        .find(|(_, function)| function.source_name == "nativeAggregateRoundTrip")
        .expect("managed aggregate Scoop extern has a typed entity");
    assert_eq!(external.abi, hir::ExternAbi::Scoop);
    assert_eq!(external.gc_effect, hir::GcEffect::Managed);
}

#[test]
fn extern_functions_reject_invalid_declarations_and_boundary_types() {
    let c_string = extern_fun(
        "cString",
        vec![("value", ty_named("String"))],
        None,
        extern_annotation("", "c_string", "c"),
    );
    let mut generic = extern_fun(
        "generic",
        vec![("value", ty_named("T"))],
        None,
        extern_annotation("", "generic", "c"),
    );
    let Decl::Function(generic_function) = &mut generic else {
        unreachable!()
    };
    generic_function.type_params = vec![type_param("T")];
    let mut suspend = extern_fun(
        "waitNative",
        vec![],
        None,
        extern_annotation("", "wait_native", "scoop"),
    );
    let Decl::Function(suspend_function) = &mut suspend else {
        unreachable!()
    };
    suspend_function.is_suspend = true;
    let errors = messages(vec![c_string, generic, suspend, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("ref type `String` is managed"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("must not be generic"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("cannot be used on a suspend"))
    );
}

#[test]
fn duplicate_native_symbols_must_have_one_consistent_contract() {
    let first = extern_fun(
        "first",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("one", "same_symbol", "c"),
    );
    let second = extern_fun(
        "second",
        vec![("value", ty_named("UInt"))],
        Some(ty_named("UInt")),
        extern_annotation("two", "same_symbol", "c"),
    );
    let errors = messages(vec![first, second, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("extern symbol `same_symbol` conflicts"))
    );

    let managed = extern_fun(
        "managedAlias",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("runtime", "same_scoop_symbol", "scoop"),
    );
    let mut no_gc = extern_fun(
        "noGcAlias",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("runtime", "same_scoop_symbol", "scoop"),
    );
    let Decl::Function(no_gc_function) = &mut no_gc else {
        unreachable!()
    };
    no_gc_function.annotations.push(marker("NoGC"));
    let errors = messages(vec![managed, no_gc, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("extern symbol `same_scoop_symbol` conflicts"))
    );
}
