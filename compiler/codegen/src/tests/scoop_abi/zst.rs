use super::*;

fn prefix_zst_arguments<Destination>(call: &mut scoop_lir::TypedCall<Destination>, value: Value) {
    let scoop_lir::TypedCall::IndirectResult { args, .. } = call else {
        panic!("aggregate fixture contains only indirect-result calls");
    };
    args.splice(
        0..0,
        [
            scoop_lir::AbiCallArgument::ElidedZst(value),
            scoop_lir::AbiCallArgument::Direct(signed64(11)),
            scoop_lir::AbiCallArgument::ElidedZst(value),
        ],
    );
}

fn mixed_abi_module() -> Module {
    let mut module = aggregate_abi_module();
    let zst = LirType::Aggregate(Vec::new());
    let signature = plain_scoop_signature(
        vec![zst.clone(), LirType::I64, zst.clone(), aggregate_type()],
        aggregate_type(),
    );
    for index in [0, 2] {
        let function = &mut module.functions[index];
        function.signature = signature.clone();
        function.blocks[function.entry].terminator = Terminator::Return {
            value: Some(Value::Param(3)),
        };
    }
    for index in [1, 3, 4, 5] {
        let function = &mut module.functions[index];
        let local = function
            .locals
            .alloc(test_local("elided_argument", zst.clone()));
        for (_, call_signature) in function.call_targets.indirect_result_signatures.iter_mut() {
            *call_signature = IndirectResultCallSignature::scoop_sret(
                signature.arguments().to_vec(),
                aggregate_value(),
                scoop_lir::CallingConvention::Cdecl,
            );
        }
        for (_, block) in function.blocks.iter_mut() {
            for instruction in &mut block.instructions {
                match instruction {
                    Instruction::Call {
                        site: CallSite::Managed(site),
                    } => {
                        prefix_zst_arguments(&mut site.call, Value::Local(local));
                    }
                    Instruction::Call {
                        site: CallSite::NoGc(site),
                    } => {
                        prefix_zst_arguments(&mut site.call, Value::Local(local));
                    }
                    Instruction::Invoke {
                        site: scoop_lir::InvokeSite::Managed(site),
                    } => {
                        prefix_zst_arguments(&mut site.call, Value::Local(local));
                    }
                    _ => {}
                }
            }
        }
    }
    module
}

fn has_mixed_attributes(line: &str) -> bool {
    let Some(sret) = line.find("sret({ i64, i64, i64 }) align 8") else {
        return false;
    };
    let Some(scalar) = line.find("i64 11") else {
        return false;
    };
    let Some(byval) = line.find("byval({ i64, i64, i64 }) align 8") else {
        return false;
    };
    sret < scalar && scalar < byval && !line.contains("{}")
}

#[test]
fn direct_invoke_dispatch_and_statepoints_share_mixed_zst_physical_indexes() {
    let module = mixed_abi_module();
    let ir = ir_of(&module);
    assert_eq!(
        ir.lines()
            .filter(|line| {
                (line.contains("call void") || line.contains("invoke token"))
                    && has_mixed_attributes(line)
            })
            .count(),
        4,
        "every direct, invoke and dispatch call must pass sret/scalar/byval with no ZST slot:\n{ir}"
    );
    assert!(!ir.contains("%elided_argument = alloca"), "{ir}");
    for index in [0, 2] {
        let definition = ir
            .lines()
            .find(|line| {
                line.starts_with("define void")
                    && line.contains(&llvm_function_symbol(&module.functions[index]))
            })
            .unwrap();
        assert!(definition.contains("i64 %1, ptr byval"), "{definition}");
        assert!(!definition.contains("{}"), "{definition}");
    }
    let rewritten = rewritten_ir_of(&module);
    assert_eq!(
        rewritten
            .lines()
            .filter(|line| { line.contains("gc.statepoint") && has_mixed_attributes(line) })
            .count(),
        3,
        "statepoint wrappers must retain the same physical parameter attributes:\n{rewritten}"
    );
}
