use super::*;

fn constructed_projection() -> (Module, usize, scoop_lir::LocalId) {
    let mut module = enum_module();
    let shape = module.enums.iter().next().unwrap().0;
    let variant = module.enums.variant_ref(shape, 1).unwrap();
    let field = module.enums.variant_field_ref(variant, 0).unwrap();
    let index = append_variant_projection_function(
        &mut module,
        "known.variant.copy",
        LirType::Enum(shape),
        field,
        LirType::I64,
    );
    let first = test_local_with_types(
        &module.structs,
        &module.enums,
        "first",
        LirType::Enum(shape),
    );
    let second = test_local_with_types(
        &module.structs,
        &module.enums,
        "second",
        LirType::Enum(shape),
    );
    let function = &mut module.functions[index];
    let value = function.temps.alloc(Temp {
        ty: LirType::Enum(shape),
    });
    let first = function.locals.alloc(first);
    let second = function.locals.alloc(second);
    let matched = function.blocks.iter().nth(1).unwrap().0;
    function.blocks[function.entry].instructions = vec![
        Instruction::EnumWrap {
            out: value,
            variant,
            fields: vec![signed64(12)],
        },
        Instruction::Store {
            local: first,
            value: Value::Temp(value),
        },
        Instruction::Store {
            local: second,
            value: Value::Local(first),
        },
    ];
    function.blocks[function.entry].terminator = Terminator::Br(matched);
    let Instruction::VariantPayloadProject { operand, .. } =
        &mut function.blocks[matched].instructions[0]
    else {
        panic!("projection block");
    };
    *operand = Value::Local(second);
    let mut blocks = Arena::default();
    for (_, block) in std::mem::take(&mut function.blocks).into_iter().take(2) {
        blocks.alloc(block);
    }
    function.blocks = blocks;
    (module, index, second)
}

#[test]
fn known_constructor_and_copies_allow_projection_after_dead_edge_removal() {
    let (module, _, _) = constructed_projection();
    let ir = ir_of(&module);
    assert!(ir.contains("variant_payload_field_ptr"));
}

#[test]
fn overwriting_a_copy_removes_its_known_variant() {
    let (mut module, index, local) = constructed_projection();
    let function = &mut module.functions[index];
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Store {
            local,
            value: Value::Param(0),
        });
    let error = enum_codegen_error(&module);
    assert!(error.0.contains("not dominated"), "{error}");
}

#[test]
fn an_unknown_pointer_store_invalidates_an_escaped_variant() {
    let (mut module, index, local) = constructed_projection();
    let function = &mut module.functions[index];
    let scoop_lir::LocalStorage::NonZero(pointee) = function.locals[local].storage() else {
        panic!("nonempty enum storage");
    };
    let pointee = pointee.clone();
    let pointer = function.temps.alloc(Temp { ty: RAW_PTR });
    function.blocks[function.entry].instructions.extend([
        Instruction::LocalAddress {
            out: pointer,
            local,
        },
        Instruction::RawStore {
            pointer: Value::Temp(pointer),
            value: Value::Param(0),
            pointee,
        },
    ]);
    let error = enum_codegen_error(&module);
    assert!(error.0.contains("not dominated"), "{error}");
}
