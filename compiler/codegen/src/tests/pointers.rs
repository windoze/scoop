use super::*;

mod fixture;

use fixture::*;

#[test]
fn logical_zst_load_results_emit_no_payload_access_or_address_token() {
    let empty = LirType::Aggregate(Vec::new());
    let nested = LirType::Aggregate(vec![empty.clone(), LirType::Aggregate(vec![empty.clone()])]);
    for ty in [empty, nested] {
        let module = zst_module(ty);
        let ir = ir_of(&module);
        let definition = ir
            .lines()
            .skip_while(|line| {
                !line.contains(&format!(
                    "define void {}",
                    llvm_function_symbol(&module.functions[0])
                ))
            })
            .take_while(|line| *line != "}")
            .collect::<Vec<_>>()
            .join("\n");
        assert!(definition.contains("ret void"), "{ir}");
        for operation in ["load ", "store ", "alloca ", "getelementptr "] {
            assert!(!definition.contains(operation), "{definition}");
        }
    }
}

#[test]
fn nonzero_pointer_storage_emits_aligned_load_store_and_scaled_offset() {
    let module = raw_module();
    let machine = host_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    llvm.verify().unwrap();
    let ir = llvm.print_to_string().to_string();
    assert!(
        ir.lines()
            .any(|line| line.contains("load i64") && line.contains("align 8")),
        "{ir}"
    );
    assert!(
        ir.lines()
            .any(|line| line.contains("store i64") && line.contains("align 8")),
        "{ir}"
    );
    assert!(ir.contains("mul i64") && ir.contains(", 8"), "{ir}");
    assert!(ir.contains("getelementptr i8"), "{ir}");
    let object = machine
        .write_to_memory_buffer(&llvm, FileType::Object)
        .unwrap();
    assert!(!object.as_slice().is_empty());
}

#[test]
fn zst_pointer_results_reject_nonzero_layout_proofs_and_wrong_temporaries() {
    let mut module = zst_module(LirType::Aggregate(Vec::new()));
    let function = &mut module.functions[0];
    let Instruction::MakeZstValue { value, .. } =
        &mut function.blocks[function.entry].instructions[0]
    else {
        panic!("ZST fixture must create a logical result");
    };
    *value = unit_zst(
        scoop_lir::AbiZst::new(LirType::I64, scoop_lir::AbiZeroSizedLayout::new(8).unwrap())
            .unwrap(),
    );
    rejected(&module, "layout");

    let mut module = zst_module(LirType::Aggregate(Vec::new()));
    let function = &mut module.functions[0];
    let (_, temp) = function.temps.iter_mut().next().unwrap();
    temp.ty = LirType::I64;
    rejected(
        &module,
        "pointer storage result temporary has the wrong type",
    );
}

#[test]
fn raw_pointer_storage_replays_size_alignment_scan_and_zero_size_classification() {
    let forged = [
        abi_value_with_layout(LirType::I64, 16, 8, RefScan::None),
        abi_value_with_layout(LirType::I64, 8, 4, RefScan::None),
        abi_value_with_layout(LirType::I64, 8, 8, RefScan::References(vec![0])),
        abi_value_with_layout(LirType::Aggregate(Vec::new()), 1, 1, RefScan::None),
    ];
    for value in forged {
        for index in [1, 2] {
            let mut module = raw_module();
            let function = &mut module.functions[0];
            match &mut function.blocks[function.entry].instructions[index] {
                Instruction::RawLoad { pointee, .. } | Instruction::RawStore { pointee, .. } => {
                    *pointee = value.clone()
                }
                _ => panic!("raw fixture must contain a load and a store"),
            }
            rejected(&module, "layout");
        }
    }
}

#[test]
fn raw_pointer_storage_rejects_mismatched_load_and_store_value_types() {
    let mut module = raw_module();
    let function = &mut module.functions[0];
    let Instruction::RawLoad { out, .. } = function.blocks[function.entry].instructions[1] else {
        panic!("raw fixture must contain a load");
    };
    function.temps[out].ty = LirType::I32;
    rejected(
        &module,
        "pointer storage result temporary has the wrong type",
    );

    let mut module = raw_module();
    let function = &mut module.functions[0];
    let Instruction::RawStore { value, .. } = &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("raw fixture must contain a store");
    };
    *value = Value::BoolConst(true);
    rejected(
        &module,
        "raw store value disagrees with its pointee storage",
    );
}
