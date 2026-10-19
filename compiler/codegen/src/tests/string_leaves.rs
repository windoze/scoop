use super::*;

#[test]
fn string_reads_keep_as1_and_do_not_enter_native() {
    let mut module = values_module();
    let text = module.globals.iter().next().unwrap().0;
    let function = &mut module.functions[0];
    for (operation, ty, indexed) in [
        (
            scoop_lir::NoGcRuntimeFunction::StringByteLength,
            LirType::I64,
            false,
        ),
        (
            scoop_lir::NoGcRuntimeFunction::StringByteAt,
            LirType::I8,
            true,
        ),
        (
            scoop_lir::NoGcRuntimeFunction::StringCharacterAtByte,
            LirType::I32,
            true,
        ),
    ] {
        let mut params = vec![MANAGED_PTR];
        let mut args = vec![Value::Global(text)];
        if indexed {
            params.push(LirType::I64);
            args.push(Value::IntegerConst(
                scoop_lir::LirIntegerConstant::Signed64(0),
            ));
        }
        let out = function.temps.alloc(Temp { ty: ty.clone() });
        let site = direct_site(
            &mut function.call_targets,
            TestCallProtocol::NoGc {
                destination: no_gc_runtime(operation),
            },
            params,
            (ty, RefScan::None),
            out,
            args,
        );
        function.blocks[function.entry]
            .instructions
            .push(Instruction::Call { site });
    }
    let ir = ir_of(&module);
    for declaration in [
        "declare i64 @scoop_rt_string_byte_length_leaf(ptr addrspace(1))",
        "declare i8 @scoop_rt_string_byte_at_leaf(ptr addrspace(1), i64)",
        "declare i32 @scoop_rt_string_character_at_byte_leaf(ptr addrspace(1), i64)",
    ] {
        assert!(ir.contains(declaration), "missing {declaration}:\n{ir}");
    }
    assert!(
        !ir.lines()
            .any(|line| line.contains("call ") && line.contains("enter_native")),
        "{ir}"
    );
}
