use super::*;

#[test]
fn type_descriptors_carry_the_gc_scan_descriptors() {
    // A class's plain table is count-prefixed (`[N, off0, ..]`,
    // runtime/include/scoop_rt.h's M9 scan-descriptor contract).
    let heap = heap_module();
    let point_scan = strong_scan_symbol(
        &heap,
        type_descriptor(&heap, "Point")
            .instance_layout
            .scan_record()
            .id(),
    );
    let ir = strong_shape_ir_of(&heap);
    assert!(
        ir.contains(&format!(
            "@\"{point_scan}\" = constant [2 x i64] [i64 1, i64 24]"
        )),
        "plain scan table must be count-prefixed:\n{ir}"
    );

    // A reference-element array's object scan carries the SCOOP_REFS_ARRAY
    // sentinel (u64::MAX, printed -1), the dynamic count/data offsets, its
    // stride, and a pointer to the recursive scan for one inline element.
    let nested_element_scan = RefScan::References(vec![8, 16]);
    let mut meta = string_metadata();
    let ref_array_type = array_type(
        &mut meta,
        "ArrayRef",
        scoop_lir::ArrayKind::Immutable,
        MANAGED_PTR,
        8,
        8,
        RefScan::References(vec![0]),
    );
    let nested_array_type = array_type(
        &mut meta,
        "ArrayNested",
        scoop_lir::ArrayKind::Immutable,
        LirType::Aggregate(vec![LirType::I64, MANAGED_PTR, MANAGED_PTR]),
        24,
        8,
        nested_element_scan,
    );
    let mut temps = Arena::default();
    let array = temps.alloc(Temp { ty: MANAGED_PTR });
    let nested_array = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ArrayAlloc {
                out: array,
                elements: vec![],
                array_type: ref_array_type,
                safepoint: test_safepoint(1),
                live: scoop_lir::StatepointLiveSet::default(),
            },
            Instruction::ArrayAlloc {
                out: nested_array,
                elements: vec![],
                array_type: nested_array_type,
                safepoint: test_safepoint(2),
                live: scoop_lir::StatepointLiveSet::default(),
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Holder".to_string(),
        identity: type_descriptor_identity("Holder"),
        instance_layout: layout_identity(
            "Holder",
            scoop_identity::RepresentationRole::ManagedObject,
        ),
        instance_shape: TypeInstanceShapeV1::fixed_object(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            56,
            8,
            RefScan::References(vec![16, 40, 48]),
        )
        .unwrap(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: vtable("Holder", vec![]),
        itables: vec![],
    });
    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            callable_body: callable_body_at(file!(), line!()),
            safepoints: scoop_lir::SafepointIdentities::default(),
            gc_effect: GcEffect::Managed,
            signature: plain_scoop_signature(vec![], LirType::Void),
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta,
    };
    refresh_module_safepoints(&mut module);
    let scan_symbol = |name: &str| {
        strong_scan_symbol(
            &module,
            type_descriptor(&module, name)
                .instance_layout
                .scan_record()
                .id(),
        )
    };
    let array_ref_scan = scan_symbol("ArrayRef");
    let array_nested_scan = scan_symbol("ArrayNested");
    let holder_scan = scan_symbol("Holder");
    let ir = strong_shape_ir_of(&module);
    assert!(
        ir.contains(&format!(
            "@\"{array_ref_scan}\" = constant [7 x i64] [i64 -1, i64 16, i64 24, i64 8, i64 ptrtoint (ptr getelementptr (i64, ptr @\"{array_ref_scan}\", i64 5) to i64), i64 1, i64 0]"
        )),
        "reference-element array TD must carry SCOOP_REFS_ARRAY:\n{ir}"
    );
    assert!(
        ir.contains(&format!(
            "@\"{array_nested_scan}\" = constant [8 x i64] [i64 -1, i64 16, i64 24, i64 24, i64 ptrtoint (ptr getelementptr (i64, ptr @\"{array_nested_scan}\", i64 5) to i64), i64 2, i64 8, i64 16]"
        )),
        "aggregate array TD must wrap the recursive element scan:\n{ir}"
    );
    assert!(
        ir.contains(&format!(
            "@\"{holder_scan}\" = constant [4 x i64] [i64 3, i64 16, i64 40, i64 48]"
        )),
        "aggregate object scan must use one canonical fixed-offset table:\n{ir}"
    );
}
