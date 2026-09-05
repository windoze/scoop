use super::*;

mod support;

use support::*;

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
    assert_eq!(
        context.legacy_integer_layout(),
        physical(profile.scalar_layout(lir::BackendScalarKind::I64))
    );
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
    let structs = Arena::new();
    let enums = Arena::new();
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
fn foreign_callback_bridge_preserves_its_nominal_family() {
    let mut builder = Builder::new();
    let callback = builder.structs.alloc(mir::StructDef {
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
    let state = builder.enums.alloc(mir::EnumDef {
        name: "ForeignCallbackState".to_string(),
        gc_free: true,
        variants: ["Registered", "Active", "Completed", "Failed"]
            .map(unit_variant)
            .into(),
    });
    let failure = builder.option_enum("Option<Any>", mir::Type::Any);
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
            state,
            failure,
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
            mode: mir::ForeignCallbackMode::Reusable,
        });

    let lowered = lower(&module);
    let lowered_family = lowered.foreign_callback_families.iter().next().unwrap().1;
    assert_eq!(lowered_family.callback.into_raw(), callback.into_raw());
    assert_eq!(lowered_family.state.into_raw(), state.into_raw());
    assert_eq!(lowered_family.failure.into_raw(), failure.into_raw());
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
