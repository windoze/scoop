use super::*;

mod support;

use support::*;

fn nominal_link_stem(name: impl Into<String>) -> mir::NominalLinkStem {
    mir::NominalLinkStem::from_session_local_encoding(name.into())
}

fn lower(module: &mir::Module) -> lir::Module {
    super::lower(module, lir::LirTargetProfile::DARWIN_AARCH64)
}

#[test]
fn selected_target_profile_is_embedded_in_lir_meta() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(&builder.finish(main));

    assert_eq!(
        module.meta.target_profile,
        lir::LirTargetProfile::DARWIN_AARCH64
    );
}

#[test]
fn nominal_descriptor_symbols_use_typed_application_identity() {
    let mut builder = Builder::new();
    let class_a = builder.class("Same", None, &[], Vec::new(), Vec::new());
    let class_b = builder.class("Same", None, &[], Vec::new(), Vec::new());
    builder.classes[class_a].link_stem = nominal_link_stem("$pkg$a$class$Same");
    builder.classes[class_b].link_stem = nominal_link_stem("$pkg$b$class$Same");
    let interface_a = builder.interface("View", &[]);
    let interface_b = builder.interface("View", &[]);
    builder.interfaces[interface_a].link_stem = nominal_link_stem("$pkg$a$interface$View");
    builder.interfaces[interface_b].link_stem = nominal_link_stem("$pkg$b$interface$View");
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    let function_type = source.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let invoke_function = source.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: "$closure.invoke".to_string(),
        symbol: "scoop.$closure.invoke".to_string(),
        params: Vec::new(),
        return_ty: mir::Type::Unit,
        body: mir::Body::unreachable(Arena::new()),
    });
    let invoke = source
        .closure_invoke_functions
        .alloc(mir::ClosureInvokeFunction {
            function: invoke_function,
        });
    let closure_a = nominal_link_stem("$generated$lambda$pkg-a");
    let closure_b = nominal_link_stem("$generated$lambda$pkg-b");
    for link_stem in [closure_a.clone(), closure_b.clone()] {
        source.closure_classes.alloc(mir::ClosureClass {
            link_stem,
            name: "SameClosure".to_string(),
            function_type,
            invoke,
            captures: Vec::new(),
            bridges: Vec::new(),
        });
    }
    let expected = [
        mir::encode_type(&source, &mir::Type::Class(class_a)).unwrap(),
        mir::encode_type(&source, &mir::Type::Class(class_b)).unwrap(),
        mir::encode_type(&source, &mir::Type::Interface(interface_a)).unwrap(),
        mir::encode_type(&source, &mir::Type::Interface(interface_b)).unwrap(),
        closure_a.as_str().to_string(),
        closure_b.as_str().to_string(),
    ]
    .map(|identity| format!("scoop_td_{identity}"));
    let module = lower(&source);

    let symbols = module
        .meta
        .type_descriptors
        .iter()
        .filter(|(_, descriptor)| {
            matches!(descriptor.name.as_str(), "Same" | "View" | "SameClosure")
        })
        .map(|(_, descriptor)| descriptor.symbol.as_str())
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 6);
    for expected in expected {
        assert!(symbols.contains(&expected.as_str()), "missing {expected}");
    }
    assert_eq!(
        module
            .meta
            .type_descriptors
            .iter()
            .find(|(_, descriptor)| descriptor.name == "String")
            .expect("intrinsic String descriptor")
            .1
            .symbol,
        lir::STRING_TD_SYMBOL
    );
}

#[test]
fn lowering_context_derives_scalar_pointer_and_runtime_prefix_layouts() {
    let profile = lir::LirTargetProfile::DARWIN_AARCH64;
    let context = LoweringContext::new(profile);
    let physical = |layout: lir::ScalarLayout| PhysicalLayout {
        size: layout.size_bytes(),
        align: layout.alignment_bytes(),
    };

    assert_eq!(
        context.scalar_layout(lir::BackendScalarKind::I1),
        physical(profile.scalar_layout(lir::BackendScalarKind::I1))
    );
    for kind in lir::IntegerKind::ALL {
        assert_eq!(
            context.integer_layout(kind),
            physical(profile.scalar_layout(kind.width().backend_scalar_kind()))
        );
    }
    assert_eq!(
        context.machine_scalar_layout(),
        physical(profile.scalar_layout(lir::BackendScalarKind::I64))
    );
    for kind in [
        lir::PointerKind::Managed,
        lir::PointerKind::Raw,
        lir::PointerKind::Code,
        lir::PointerKind::Metadata,
    ] {
        assert_eq!(
            context.pointer_layout(kind),
            physical(profile.pointer_layout(kind))
        );
    }

    let i64_layout = context.scalar_layout(lir::BackendScalarKind::I64);
    let metadata_pointer = context.pointer_layout(lir::PointerKind::Metadata);
    let (header_offsets, expected_header) =
        context.aggregate_layout([metadata_pointer, i64_layout]);
    assert_eq!(context.object_header_layout(), expected_header);
    assert_eq!(context.object_type_descriptor_offset(), header_offsets[0]);

    let (_, expected_string) = context.aggregate_layout([expected_header, i64_layout]);
    assert_eq!(context.string_layout(), expected_string);

    let code_pointer = context.pointer_layout(lir::PointerKind::Code);
    let (closure_offsets, expected_closure) =
        context.aggregate_layout([expected_header, code_pointer]);
    assert_eq!(
        context.closure_prefix(),
        (closure_offsets[1], expected_closure)
    );

    let raw_pointer = context.pointer_layout(lir::PointerKind::Raw);
    assert_eq!(
        context.closure_invoke_dispatch_slot(),
        u32::try_from(closure_offsets[1] / raw_pointer.size).expect("test slot fits u32")
    );
    let i32_layout = context.scalar_layout(lir::BackendScalarKind::I32);
    let (_, expected_exception) = context.aggregate_layout([raw_pointer, i32_layout]);
    assert_eq!(context.exception_record_layout(), expected_exception);

    let (descriptor_offsets, _) = context.aggregate_layout([
        i64_layout,
        i64_layout,
        i64_layout,
        metadata_pointer,
        metadata_pointer,
        metadata_pointer,
    ]);
    assert_eq!(
        context.type_descriptor_vtable_offset(),
        descriptor_offsets[5]
    );
}

#[test]
fn profile_layout_drives_aggregate_root_scan_offsets() {
    let context = LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64);
    let structs = lir::StructDefs::default();
    let enums = lir::EnumDefs::default();
    let ty = lir::LirType::Aggregate(vec![lir::LirType::I1, lir::MANAGED_PTR]);
    let bool_layout = context.scalar_layout(lir::BackendScalarKind::I1);
    let pointer_layout = context.pointer_layout(lir::PointerKind::Managed);
    let (offsets, expected) = context.aggregate_layout([bool_layout, pointer_layout]);

    assert_eq!(
        safepoints::lir_size_align(&context, &ty, &structs, &enums),
        (expected.size, expected.align)
    );
    assert_eq!(
        safepoints::root_scan(&context, &ty, &structs, &enums, 0),
        lir::RefScan::References(vec![offsets[1]])
    );
}

#[test]
fn no_gc_effect_is_preserved_in_lir() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    builder.functions[main].gc_effect = mir::GcEffect::NoGc;
    let module = lower(&builder.finish(main));
    assert_eq!(module.functions[0].gc_effect, lir::GcEffect::NoGc);
    assert!(lir::dump(&module).contains("-> void <no-gc>"));
}

#[test]
fn c_abi_preserves_all_eight_exact_integer_kinds() {
    let mut builder = Builder::new();
    let params = mir::IntegerKind::ALL
        .map(mir::Type::Integer)
        .into_iter()
        .collect::<Vec<_>>();
    builder.extern_functions.alloc(mir::ExternFunction {
        source_name: "integers".to_string(),
        native_symbol: "integers".to_string(),
        library: String::new(),
        abi: mir::ExternAbi::C,
        calling_convention: mir::CallingConvention::Cdecl,
        gc_effect: mir::GcEffect::NoGc,
        params,
        return_type: mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
    });
    let main = builder.main(Arena::new(), Vec::new());
    let mut mir_module = builder.finish(main);
    mir_module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: mir::IntegerKind::ALL
            .map(mir::Type::Integer)
            .into_iter()
            .collect(),
        return_type: mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
    });
    let module = lower(&mir_module);
    let (_, function) = module.extern_functions.iter().next().expect("one C extern");
    let lir::ExternFunctionKind::C { signature, .. } = &function.kind else {
        panic!("C declaration remains a C bridge")
    };
    assert_eq!(
        &signature.params,
        &lir::IntegerKind::ALL
            .map(lir::CType::Integer)
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        signature.return_type,
        lir::CReturnType::Value(Box::new(
            lir::CType::Integer(lir::IntegerKind::UNSIGNED_64,)
        ))
    );
    assert_eq!(
        signature.storage_params(),
        [
            lir::LirType::I8,
            lir::LirType::I16,
            lir::LirType::I32,
            lir::LirType::I64,
            lir::LirType::I8,
            lir::LirType::I16,
            lir::LirType::I32,
            lir::LirType::I64,
        ]
    );
    assert!(
        descriptor_values(&module).any(|descriptor| {
            descriptor.name == "function$FI8_I16_I32_I64_V8_V16_V32_V64RV64X"
        })
    );
}

#[test]
fn c_abi_nullable_refs_bind_the_exact_lowered_pointee_and_signature() {
    let mut builder = Builder::new();
    let raw_payload = mir::Type::Ptr(Box::new(INT));
    let raw_option = builder.option_enum("Option$Ptr$Int", raw_payload.clone());
    let native_signature = builder.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![mir::Type::Integer(mir::IntegerKind::UNSIGNED_16)],
        return_type: mir::Type::Integer(mir::IntegerKind::SIGNED_8),
    });
    let code_payload = mir::Type::FunPtr(native_signature);
    let code_option = builder.option_enum("Option$FunPtr", code_payload.clone());
    builder.extern_functions.alloc(mir::ExternFunction {
        source_name: "nullablePointers".to_string(),
        native_symbol: "nullable_pointers".to_string(),
        library: String::new(),
        abi: mir::ExternAbi::C,
        calling_convention: mir::CallingConvention::Cdecl,
        gc_effect: mir::GcEffect::NoGc,
        params: vec![
            mir::Type::Enum(raw_option, vec![raw_payload]),
            mir::Type::Enum(code_option, vec![code_payload]),
        ],
        return_type: mir::Type::Unit,
    });
    let main = builder.main(Arena::new(), Vec::new());

    let module = lower(&builder.finish(main));
    let (_, function) = module.extern_functions.iter().next().expect("one C extern");
    let lir::ExternFunctionKind::C { signature, .. } = &function.kind else {
        panic!("C declaration remains a C bridge")
    };
    let lir::CType::DataPointer {
        pointee,
        storage: lir::CDataPointerStorage::Nullable(raw_reference),
    } = &signature.params[0]
    else {
        panic!("first parameter is an exact nullable data pointer")
    };
    assert_eq!(pointee, raw_reference.pointee());
    assert_eq!(
        module
            .enums
            .nullable_data_pointer_binding(raw_reference.definition()),
        Some(raw_reference.pointee())
    );
    let lir::CType::CodePointer {
        signature: code_signature,
        storage: lir::CCodePointerStorage::Nullable(code_reference),
    } = &signature.params[1]
    else {
        panic!("second parameter is an exact nullable code pointer")
    };
    assert_eq!(code_signature.as_ref(), code_reference.signature());
    assert_eq!(
        module
            .enums
            .nullable_code_pointer_binding(code_reference.definition()),
        Some(code_reference.signature())
    );
    let dump = lir::dump(&module);
    assert!(dump.contains("data-ptr<Int,nullable=enum0<Int>>"), "{dump}");
    assert!(
        dump.contains("code-ptr<(UInt16)->Int8,nullable=enum1<(UInt16)->Int8>>"),
        "{dump}"
    );
}

#[test]
#[should_panic(expected = "HIR C-FFI classification rejects")]
fn c_abi_does_not_guess_nullable_pointer_from_a_non_option_enum_shape() {
    let mut builder = Builder::new();
    let payload = mir::Type::Ptr(Box::new(INT));
    let lookalike = builder.enums.alloc(mir::EnumDef {
        link_stem: nominal_link_stem(format!("$test$nominal${}", line!())),
        name: "LooksLikeOption".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Some".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "_1".to_string(),
                    ty: payload.clone(),
                }],
            },
            mir::VariantDef {
                name: "None".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
    });
    builder.extern_functions.alloc(mir::ExternFunction {
        source_name: "lookalike".to_string(),
        native_symbol: "lookalike".to_string(),
        library: String::new(),
        abi: mir::ExternAbi::C,
        calling_convention: mir::CallingConvention::Cdecl,
        gc_effect: mir::GcEffect::NoGc,
        params: vec![mir::Type::Enum(lookalike, vec![payload])],
        return_type: mir::Type::Unit,
    });
    let main = builder.main(Arena::new(), Vec::new());

    let _ = lower(&builder.finish(main));
}

#[test]
fn foreign_callback_bridge_preserves_its_nominal_family() {
    let mut builder = Builder::new();
    let callback = builder.structs.alloc(mir::StructDef {
        link_stem: nominal_link_stem(format!("$test$nominal${}", line!())),
        type_arguments: Vec::new(),
        name: "ForeignCallback<(Int) -> Unit>".to_string(),
        gc_free: true,
        representation: mir::StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![
                mir::Field {
                    name: "function".to_string(),
                    ty: mir::Type::FunPtr(mir::FunctionTypeId::from_raw(0.into())),
                },
                mir::Field {
                    name: "context".to_string(),
                    ty: mir::Type::Ptr(Box::new(mir::Type::Unit)),
                },
            ],
        },
    });
    let unit_variant = |name: &str| mir::VariantDef {
        name: name.to_string(),
        gc_free: true,
        fields: Vec::new(),
    };
    let mode = builder.enums.alloc(mir::EnumDef {
        link_stem: nominal_link_stem(format!("$test$nominal${}", line!())),
        name: "ForeignCallbackMode".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: ["Reusable", "OneShot"].map(unit_variant).into(),
    });
    let state = builder.enums.alloc(mir::EnumDef {
        link_stem: nominal_link_stem(format!("$test$nominal${}", line!())),
        name: "ForeignCallbackState".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: ["Registered", "Active", "Completed", "Failed"]
            .map(unit_variant)
            .into(),
    });
    let throwable = builder.class("Throwable", None, &[], Vec::new(), Vec::new());
    let failure = builder.option_enum("Option<Throwable>", mir::Type::Class(throwable));
    let modes = mir::ForeignCallbackModes::checked(
        &builder.enums,
        mir::MirVariantRef::new(&builder.enums, mode, 0).expect("Reusable"),
        mir::MirVariantRef::new(&builder.enums, mode, 1).expect("OneShot"),
    )
    .expect("callback modes");
    let states = mir::ForeignCallbackStates::checked(
        &builder.enums,
        mir::MirVariantRef::new(&builder.enums, state, 0).expect("Registered"),
        mir::MirVariantRef::new(&builder.enums, state, 1).expect("Active"),
        mir::MirVariantRef::new(&builder.enums, state, 2).expect("Completed"),
        mir::MirVariantRef::new(&builder.enums, state, 3).expect("Failed"),
    )
    .expect("callback states");
    let failure_result = mir::ForeignCallbackFailureResult::checked(
        &builder.enums,
        *builder.option_core.last().expect("failure Option metadata"),
        throwable,
    )
    .expect("callback failure result");
    let main = builder.main(Arena::new(), Vec::new());
    let mut module = builder.finish(main);
    let native_signature = module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![mir::Type::Ptr(Box::new(mir::Type::Unit))],
        return_type: mir::Type::Unit,
    });
    let managed_signature = module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let family = module
        .foreign_callback_families
        .alloc(mir::ForeignCallbackFamily {
            callback,
            modes,
            states,
            failure_result,
        });
    let adapter = module
        .foreign_callback_adapters
        .alloc(mir::ForeignCallbackAdapter {
            function: main,
            managed_signature,
        });
    module
        .foreign_callback_bridges
        .alloc(mir::ForeignCallbackBridge {
            adapter,
            family,
            native_signature,
            context_index: 0,
            mode: modes.reusable(),
        });

    let lowered = lower(&module);
    let lowered_family = lowered.foreign_callback_families.iter().next().unwrap().1;
    assert_eq!(lowered_family.callback.into_raw(), callback.into_raw());
    assert_eq!(
        lowered_family.states.definition().into_raw(),
        state.into_raw()
    );
    assert_eq!(
        lowered_family.failure_result.definition().into_raw(),
        failure.into_raw()
    );
    assert_eq!(
        lowered_family.modes.reusable().definition().into_raw(),
        mode.into_raw()
    );
    assert_eq!(
        lowered
            .foreign_callback_bridges
            .iter()
            .next()
            .unwrap()
            .1
            .family,
        scoop_lir::ForeignCallbackFamilyId::from_raw(family.into_raw())
    );
}

mod arrays;
mod basics;
mod enums;
mod exceptions;
mod functions;
mod objects;
mod traps;
