use super::*;

#[test]
fn emits_non_empty_object_file() {
    let module = values_module();
    let output = std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
    emit_object(&module, &output, host_profile()).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

#[test]
fn emits_unsigned_division_remainder_and_three_way_comparisons() {
    let mut module = values_module();
    let function = &mut module.functions[0];
    let entry = function.entry;
    let input = function.locals.iter().next().expect("integer local").0;
    for op in [
        BinOp::UDiv,
        BinOp::URem,
        BinOp::SCompareTo,
        BinOp::UCompareTo,
    ] {
        let out = function.temps.alloc(Temp { ty: LirType::I64 });
        function.blocks[entry]
            .instructions
            .push(Instruction::BinOp {
                out,
                op,
                lhs: Value::Local(input),
                rhs: Value::IntConst(2),
            });
    }
    let ir = ir_of(&module);
    for instruction in ["udiv i64", "urem i64", "icmp slt i64", "icmp ult i64"] {
        assert!(
            ir.contains(instruction),
            "missing {instruction:?} in:\n{ir}"
        );
    }
    assert!(ir.matches("select i1").count() >= 4, "{ir}");
}
