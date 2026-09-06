use super::*;

#[test]
fn compiler_runtime_calls_with_results_produce_typed_temps() {
    let mut b = Builder::new();
    let s0 = b.string("a");
    let s1 = b.string("b");
    let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", mir::Type::String));
    let main = b.main(
        locals,
        vec![
            call_value(
                s,
                runtime_call(
                    mir::RuntimeFn::StringConcat,
                    vec![string_expr(s0), string_expr(s1)],
                ),
            ),
            call_stmt(user_call(helper)),
        ],
    );
    let module = lower(&b.finish(main));

    // top_level order: helper first, then main.
    let function = &module.functions[1];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);

    let lir::Instruction::Call { site } = instructions[0] else {
        panic!("string concat must produce a value")
    };
    let concat_out = site.direct_out().expect("string concat result");
    assert_eq!(
        call_symbol(&module, site.destination(&function.call_targets)),
        "scoop_rt_string_concat"
    );
    assert_eq!(function.temps[concat_out].ty, lir::MANAGED_PTR);
    assert!(matches!(instructions[1], lir::Instruction::Store { .. }));

    // User calls return void; the Unit value is a fresh empty
    // aggregate.
    let lir::Instruction::Call { site } = instructions[2] else {
        panic!("user calls must return void")
    };
    assert_eq!(site.result(), lir::TypedCallResult::Void);
    assert_eq!(
        call_symbol(&module, site.destination(&function.call_targets)),
        "scoop.helper"
    );
    let lir::Instruction::MakeAggregate { out, elements } = instructions[3] else {
        panic!("a void call's Unit value must be an empty aggregate")
    };
    assert!(elements.is_empty());
    assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));
}

#[test]
fn string_compare_uses_the_closed_signed_64_runtime_abi() {
    let mut builder = Builder::new();
    let left = builder.string("left");
    let right = builder.string("right");
    let mut locals = Arena::new();
    let result = locals.alloc(local("runtime_result", LONG));
    let main = builder.main(
        locals,
        vec![call_value(
            result,
            runtime_call(
                mir::RuntimeFn::StringCompare,
                vec![string_expr(left), string_expr(right)],
            ),
        )],
    );
    let module = lower(&builder.finish(main));
    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::Call { site } = instructions[0] else {
        panic!("string compare must produce a value")
    };
    let out = site.direct_out().expect("string compare result");

    assert_eq!(
        call_symbol(&module, site.destination(&function.call_targets)),
        "scoop_rt_string_compare"
    );
    assert_eq!(function.temps[out].ty, lir::LirType::I64);
    let (_, result_local) = function.locals.iter().next().expect("one result local");
    assert_eq!(result_local.ty, lir::LirType::I64);
}

#[test]
fn pointer_nulls_preserve_raw_and_code_provenance_in_lir() {
    assert!(matches!(
        lower_constant_image(
            &mir::MirConstantImage::PointerNull(mir::MirPointerNull::Data),
            &lir::EnumDefs::default(),
            &HashMap::new()
        ),
        lir::LirConstantImage::NullPointer(lir::PointerKind::Raw)
    ));
    assert!(matches!(
        lower_constant_image(
            &mir::MirConstantImage::PointerNull(mir::MirPointerNull::Code),
            &lir::EnumDefs::default(),
            &HashMap::new()
        ),
        lir::LirConstantImage::NullPointer(lir::PointerKind::Code)
    ));

    let mut blocks = Arena::new();
    let entry = blocks.alloc(lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: lir::Terminator::Return { value: None },
    });
    let function = lir::Function {
        gc_effect: lir::GcEffect::NoGc,
        symbol: "null_provenance".to_string(),
        signature: lir::ScoopAbiSignature::new(
            Vec::new(),
            lir::AbiReturn::UnitVoid,
            lir::CallingConvention::Cdecl,
        ),
        call_targets: lir::CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    let globals = Arena::new();
    for kind in [
        lir::PointerKind::Managed,
        lir::PointerKind::Raw,
        lir::PointerKind::Code,
        lir::PointerKind::Metadata,
    ] {
        assert_eq!(
            function.value_ty(&globals, lir::Value::NullPointer(kind)),
            lir::LirType::Ptr(kind)
        );
    }
}

#[test]
fn enum_unit_constant_maps_between_checked_stage_local_refs() {
    let mut mir_enums = Arena::new();
    let mir_enum = mir_enums.alloc(mir::EnumDef {
        name: "Flag".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![mir::VariantDef {
            name: "Off".to_string(),
            gc_free: true,
            fields: Vec::new(),
        }],
    });
    let source = mir::MirVariantRef::new(&mir_enums, mir_enum, 0).expect("unit variant");
    let mut lir_enums = lir::EnumDefs::default();
    let lir_enum = lir_enums.alloc(lir::EnumDef {
        name: "Flag".to_string(),
        repr: lir::EnumRepr::Tagged {
            variants: vec![lir::EnumVariantRepr {
                fields: Vec::new(),
                slot_offset: 8,
                slot_size: 0,
                slot_align: 1,
                gc_free: true,
            }],
            size: 8,
            align: 8,
        },
        scan: lir::RefScan::None,
    });
    let lowered = lower_constant_image(
        &mir::MirConstantImage::EnumUnit { variant: source },
        &lir_enums,
        &HashMap::new(),
    );
    let lir::LirConstantImage::EnumUnit { variant } = lowered else {
        panic!("unit enum constant stays a typed unit enum constant")
    };
    assert!(lir_enums.contains_variant(variant));
    assert_eq!(variant.definition(), lir_enum);
    assert_eq!(variant.index(), source.variant_index());
}

#[test]
fn static_initial_state_and_all_integer_constant_variants_lower_exhaustively() {
    let cases = [
        (
            mir::MirIntegerConstant::Signed8(0x80),
            lir::LirIntegerConstant::Signed8(0x80),
        ),
        (
            mir::MirIntegerConstant::Signed16(0x8000),
            lir::LirIntegerConstant::Signed16(0x8000),
        ),
        (
            mir::MirIntegerConstant::Signed32(0x8000_0000),
            lir::LirIntegerConstant::Signed32(0x8000_0000),
        ),
        (
            mir::MirIntegerConstant::Signed64(0x8000_0000_0000_0000),
            lir::LirIntegerConstant::Signed64(0x8000_0000_0000_0000),
        ),
        (
            mir::MirIntegerConstant::Unsigned8(u8::MAX),
            lir::LirIntegerConstant::Unsigned8(u8::MAX),
        ),
        (
            mir::MirIntegerConstant::Unsigned16(u16::MAX),
            lir::LirIntegerConstant::Unsigned16(u16::MAX),
        ),
        (
            mir::MirIntegerConstant::Unsigned32(u32::MAX),
            lir::LirIntegerConstant::Unsigned32(u32::MAX),
        ),
        (
            mir::MirIntegerConstant::Unsigned64(u64::MAX),
            lir::LirIntegerConstant::Unsigned64(u64::MAX),
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(
            lower_constant_image(
                &mir::MirConstantImage::Integer(source),
                &lir::EnumDefs::default(),
                &HashMap::new(),
            ),
            lir::LirConstantImage::Integer(expected)
        );
    }
    assert!(matches!(
        lower_static_initial_state(
            &mir::MirStaticInitialState::ZeroedForRuntimeUnit,
            &lir::EnumDefs::default(),
            &HashMap::new(),
        ),
        lir::LirStaticInitialState::ZeroedForRuntimeUnit
    ));
    assert!(matches!(
        lower_static_initial_state(
            &mir::MirStaticInitialState::EncodedStaticValue {
                payload: mir::MirConstantImage::Integer(mir::MirIntegerConstant::Unsigned16(0),),
            },
            &lir::EnumDefs::default(),
            &HashMap::new(),
        ),
        lir::LirStaticInitialState::EncodedStaticValue {
            payload: lir::LirConstantImage::Integer(lir::LirIntegerConstant::Unsigned16(0)),
        }
    ));
}
