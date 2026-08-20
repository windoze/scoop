use super::*;

pub(super) fn module(target: TargetProfileId) -> Module {
    Module {
        release_hooks: Default::default(),
        cone: ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: Default::default(),
        enums: Default::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: AtomicValueKind::ALL.iter().copied().map(function).collect(),
        output: scoop_lir::LirOutput::Library,
        meta: string_metadata_for(scoop_lir::LirTargetProfile::from_id(target)),
    }
}

fn function(kind: AtomicValueKind) -> Function {
    let location = AtomicLocation {
        object: Value::Param(0),
        offset: 16,
        kind,
    };
    let value_type = location.value_type();
    let mut temps = Arena::default();
    let mut instructions = Vec::new();
    for &order in AtomicLoadOrder::ALL {
        let out = temps.alloc(Temp {
            ty: value_type.clone(),
        });
        instructions.push(Instruction::AtomicLoad {
            out,
            location,
            order,
        });
    }
    for &order in AtomicStoreOrder::ALL {
        instructions.push(Instruction::AtomicStore {
            location,
            value: Value::Param(2),
            order,
        });
    }
    for &operation in AtomicRmwOperation::ALL {
        if !operation.accepts(kind) {
            continue;
        }
        for &order in AtomicMemoryOrder::ALL {
            let out = temps.alloc(Temp {
                ty: value_type.clone(),
            });
            instructions.push(Instruction::AtomicRmw {
                out,
                location,
                value: Value::Param(2),
                operation,
                order,
            });
        }
    }
    for &order in AtomicCompareExchangeOrder::ALL {
        for &result in AtomicCompareExchangeResult::ALL {
            let ty = match result {
                AtomicCompareExchangeResult::ObservedValue => value_type.clone(),
                AtomicCompareExchangeResult::Success => LirType::I1,
            };
            let out = temps.alloc(Temp { ty });
            instructions.push(Instruction::AtomicCmpXchg {
                out,
                location,
                expected: Value::Param(1),
                replacement: Value::Param(2),
                result,
                order,
            });
        }
    }
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions,
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body: callable_body(kind.registry_key()),
        safepoints: Default::default(),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(
            vec![MANAGED_PTR, value_type.clone(), value_type],
            LirType::Void,
        ),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    }
}
