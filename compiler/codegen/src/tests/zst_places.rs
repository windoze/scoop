use super::*;

fn places_module(address_logical: bool) -> Module {
    let mut module = values_module();
    let mut locals = Arena::new();
    let first = locals.alloc(test_zst_place("first"));
    let second = locals.alloc(test_zst_place("second"));
    let logical = locals.alloc(test_local("logical", LirType::Aggregate(Vec::new())));
    let mut temps = Arena::new();
    let first_address = temps.alloc(Temp { ty: RAW_PTR });
    let repeated_address = temps.alloc(Temp { ty: RAW_PTR });
    let second_address = temps.alloc(Temp { ty: RAW_PTR });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Store {
                local: first,
                value: Value::Param(0),
            },
            Instruction::Store {
                local: second,
                value: Value::Local(first),
            },
            Instruction::Store {
                local: logical,
                value: Value::Local(second),
            },
            Instruction::LocalAddress {
                out: first_address,
                local: first,
            },
            Instruction::LocalAddress {
                out: repeated_address,
                local: first,
            },
            Instruction::LocalAddress {
                out: second_address,
                local: if address_logical { logical } else { second },
            },
        ]
        .into(),
        terminator: Terminator::Return { value: None },
    });
    module.functions = vec![Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_signature: Default::default(),
        debug_origin: Default::default(),
        callable_body: callable_body("zstPlaces"),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(vec![LirType::Aggregate(Vec::new())], LirType::Void),
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals,
        temps,
        blocks,
        entry,
    }];
    module.output = scoop_lir::LirOutput::Library;
    module
}

#[test]
fn only_address_observed_zst_locals_allocate_tokens_and_never_copy_payload() {
    let module = places_module(false);
    let ir = ir_of(&module);
    assert!(ir.contains("%first = alloca i8, align 1"), "{ir}");
    assert!(ir.contains("%second = alloca i8, align 1"), "{ir}");
    assert!(!ir.contains("%logical = alloca"), "{ir}");
    assert!(!ir.contains("load {}") && !ir.contains("store {}"), "{ir}");
    assert!(ir.contains(&format!(
        "define void {}()",
        llvm_function_symbol(&module.functions[0])
    )));
}

#[test]
fn codegen_rejects_address_of_a_logical_zst_without_a_lir_token_plan() {
    let module = places_module(true);
    let machine = host_target_machine().unwrap();
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap_err();
    assert!(
        error
            .0
            .contains("logical ZST local %logical without a place token"),
        "{error}"
    );
}
