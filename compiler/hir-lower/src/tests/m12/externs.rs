use super::*;

fn extern_property(
    name: &str,
    mutable: bool,
    thread_local: bool,
    library: &str,
    native_symbol: &str,
) -> Decl {
    let mut annotations = vec![extern_annotation(library, native_symbol, "c")];
    if mutable {
        annotations.push(marker(if thread_local {
            "ThreadLocal"
        } else {
            "Global"
        }));
    }
    Decl::Global(ast::PropertyDecl {
        annotations,
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::ExternStorage,
        span: sp(),
    })
}

fn source_native_contracts(
    module: &hir::Module,
) -> Vec<(String, scoop_identity::SourceNativeExternalContractRecord)> {
    let mut records = module
        .source_native_contracts
        .iter()
        .filter_map(|entry| {
            let name = match entry.owner() {
                hir::HirSourceNativeContractOwner::Function(function) => {
                    &module.functions[function].name
                }
                hir::HirSourceNativeContractOwner::Global(global) => {
                    &module.properties[module.globals[global].property].name
                }
            };
            matches!(
                name.as_str(),
                "nativeAdd" | "nativeWrite" | "readOnly" | "mutableData" | "mutableTls"
            )
            .then(|| (name.clone(), entry.record().clone()))
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left.0.cmp(&right.0));
    records
}

fn rebuild_source_native_contracts(
    module: &hir::Module,
) -> Result<hir::HirSourceNativeContracts, hir::HirSourceNativeContractError> {
    hir::HirSourceNativeContracts::from_declarations(hir::HirSourceNativeContractInputs {
        functions: &module.functions,
        extern_functions: &module.extern_functions,
        globals: &module.globals,
        properties: &module.properties,
        function_identities: &module.function_identities,
        property_identities: &module.property_identities,
        type_inputs: hir::HirTypeIdentityInputs {
            types: &module.types,
            function_types: &module.function_types,
            structs: &module.structs,
            struct_applications: &module.struct_applications,
            enums: &module.enums,
            enum_applications: &module.enum_applications,
            classes: &module.classes,
            class_applications: &module.class_applications,
            interfaces: &module.interfaces,
            interface_applications: &module.interface_applications,
            objects: &module.objects,
            intrinsic_core: &module.intrinsic_type_core,
            nominal_identities: &module.nominal_identities,
        },
        unit: module.unit,
    })
}

fn native_contract_fixture(with_unrelated_prefix: bool) -> ast::SourceFile {
    let mut declarations = vec![
        extern_fun(
            "nativeAdd",
            vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
            Some(ty_named("Int")),
            extern_annotation("numbers", "native_add", "c"),
        ),
        extern_fun(
            "nativeWrite",
            vec![("message", ty_named("String"))],
            None,
            extern_annotation("", "scoop_rt_write", "scoop"),
        ),
        extern_property("readOnly", false, false, "data", "read_only"),
        extern_property("mutableData", true, false, "data", "mutable_data"),
        extern_property("mutableTls", true, true, "data", "mutable_tls"),
        fun("main", vec![]),
    ];
    if with_unrelated_prefix {
        declarations.insert(0, fun("unrelated", vec![]));
    }
    file(declarations)
}

#[test]
fn source_native_contracts_cover_functions_and_globals_without_arena_identity() {
    let first = lower_user(native_contract_fixture(false)).expect("native contract fixture lowers");
    let shifted =
        lower_user(native_contract_fixture(true)).expect("shifted native contract fixture lowers");
    let records = source_native_contracts(&first);
    assert_eq!(records, source_native_contracts(&shifted));
    assert_eq!(records.len(), 5);

    let contract = |name: &str| {
        records
            .iter()
            .find_map(|(candidate, record)| (candidate == name).then_some(record.contract()))
            .unwrap_or_else(|| panic!("missing source-native contract for {name}"))
    };
    assert!(matches!(
        contract("nativeAdd"),
        scoop_identity::SourceNativeExternalContract::Function {
            symbol,
            library: scoop_identity::SourceNativeLibraryBinding::LogicalLibrary(library),
            abi: scoop_identity::SourceExternFunctionAbi::C(signature),
            calling_convention: scoop_identity::SourceCallingConvention::Cdecl,
        } if symbol.as_bytes() == b"native_add"
            && library.as_str() == "numbers"
            && signature.parameters().len() == 2
            && matches!(signature.result(), scoop_identity::SourceCAbiReturn::Value(_))
    ));
    assert!(matches!(
        contract("nativeWrite"),
        scoop_identity::SourceNativeExternalContract::Function {
            symbol,
            library: scoop_identity::SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: scoop_identity::SourceExternFunctionAbi::Scoop {
                signature,
                gc_effect: scoop_identity::GcEffect::Managed,
            },
            ..
        } if symbol.as_bytes() == b"scoop_rt_write"
            && signature.parameters().len() == 1
    ));
    assert!(matches!(
        contract("readOnly"),
        scoop_identity::SourceNativeExternalContract::ReadOnlyData { .. }
    ));
    assert!(matches!(
        contract("mutableData"),
        scoop_identity::SourceNativeExternalContract::MutableData { .. }
    ));
    assert!(matches!(
        contract("mutableTls"),
        scoop_identity::SourceNativeExternalContract::MutableTls { .. }
    ));
}

#[test]
fn source_native_contract_relation_rejects_duplicate_extern_ownership() {
    let mut module = lower_user(native_contract_fixture(false))
        .expect("native contract fixture lowers")
        .into_module();
    let external = module
        .functions
        .iter()
        .find_map(|(_, function)| match function.kind {
            hir::FunctionKind::Extern(external) if function.name == "nativeAdd" => Some(external),
            _ => None,
        })
        .expect("nativeAdd extern entity");
    let main = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "main").then_some(id))
        .expect("main function");
    module.functions[main].kind = hir::FunctionKind::Extern(external);

    assert!(matches!(
        rebuild_source_native_contracts(&module),
        Err(hir::HirSourceNativeContractError::DuplicateExternFunction { .. })
    ));
}

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
