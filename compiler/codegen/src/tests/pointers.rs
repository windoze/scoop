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
