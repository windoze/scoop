use super::*;

fn module_with_function(
    signature: scoop_lir::ScoopAbiSignature,
    temps: Arena<Temp>,
    instructions: Vec<Instruction>,
    result: Value,
) -> Module {
    let mut module = values_module();
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions,
        terminator: Terminator::Return {
            value: Some(result),
        },
    });
    module.functions = vec![Function {
        callable_body: callable_body("pointerStorage"),
        gc_effect: GcEffect::NoGc,
        signature,
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals: Arena::new(),
        temps,
        blocks,
        entry,
    }];
    module.output = scoop_lir::LirOutput::Library;
    module
}

pub(super) fn zst_module(ty: LirType) -> Module {
    let representation =
        scoop_lir::AbiZst::new(ty.clone(), scoop_lir::AbiZeroSizedLayout::new(1).unwrap()).unwrap();
    let mut temps = Arena::new();
    let out = temps.alloc(Temp { ty: ty.clone() });
    module_with_function(
        plain_scoop_signature(Vec::new(), ty),
        temps,
        vec![Instruction::MakeZstValue {
            out,
            value: scoop_lir::LogicalZstValue::new(test_exact_type("PointerZst"), representation),
        }],
        Value::Temp(out),
    )
}

pub(super) fn raw_module() -> Module {
    let mut temps = Arena::new();
    let pointer = temps.alloc(Temp { ty: RAW_PTR });
    let out = temps.alloc(Temp { ty: LirType::I64 });
    let pointee = abi_value_with_layout(LirType::I64, 8, 8, RefScan::None);
    module_with_function(
        plain_scoop_signature(vec![RAW_PTR, LirType::I64], LirType::I64),
        temps,
        vec![
            Instruction::PtrOffset {
                out: pointer,
                pointer: Value::Param(0),
                element_offset: Value::Param(1),
                element_size: std::num::NonZeroU64::new(8).unwrap(),
                subtract: false,
            },
            Instruction::RawLoad {
                out,
                pointer: Value::Temp(pointer),
                pointee: pointee.clone(),
            },
            Instruction::RawStore {
                pointer: Value::Param(0),
                value: Value::Temp(out),
                pointee,
            },
        ],
        Value::Temp(out),
    )
}

pub(super) fn rejected(module: &Module, expected: &str) {
    let machine = host_target_machine().unwrap();
    let context = Context::create();
    let error = emit_llvm_module(&context, module, &machine, host_profile()).unwrap_err();
    assert!(error.0.contains(expected), "{error}");
}
