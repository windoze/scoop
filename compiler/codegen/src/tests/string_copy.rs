use super::*;
use scoop_lir::{StringCopySource, StringCopyTarget, TypeDescriptorRef};

#[test]
fn string_copy_reloads_its_only_input_after_allocation() {
    let mut module = arrays::arrays_module();
    module.output = scoop_lir::LirOutput::Library;
    let descriptor = module
        .meta
        .type_descriptors
        .iter()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "String")
        .unwrap()
        .0;
    let overflow_message = module.globals.iter().next().unwrap().0;
    let mut temps = Arena::default();
    let copy = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![Instruction::StringCopy {
            out: copy,
            source: StringCopySource::Slice {
                string: Value::Param(0),
                start: signed64(1),
                end: signed64(5),
            },
            target: StringCopyTarget::String(TypeDescriptorRef::Local(descriptor)),
            overflow_message,
            safepoint: test_safepoint(1),
            live: statepoint_live(vec![statepoint_value(
                scoop_lir::CallerRootSource::Param(0),
                MANAGED_PTR,
                &[0],
            )]),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(copy)),
        },
    });
    module.functions[0] = Function {
        callable_body: callable_body("scoop_main"),
        safepoints: test_safepoints("scoop_main", &blocks, entry),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };
    let ir = ir_of(&module);
    let after = ir.split("alloc.continue.0:").nth(1).unwrap();
    let root = after
        .find("load ptr addrspace(1), ptr %managed_root_storage")
        .unwrap();
    let address = after
        .find("copy.source = getelementptr i8, ptr addrspace(1) %root_param")
        .unwrap();
    let memcpy = after.find("@llvm.memcpy.p1.p1.i64").unwrap();
    assert!(root < address && address < memcpy, "{ir}");
    assert!(!ir.contains("@scoop_rt_string_slice_bytes"), "{ir}");
    assert!(!ir.contains("@scoop_rt_push_native_roots"), "{ir}");
    assert!(!ir.contains("@scoop_rt_gc_write_barrier"), "{ir}");
    let rewritten = rewritten_ir_of(&module);
    assert!(rewritten.contains("gc.relocate"), "{rewritten}");
    assert!(rewritten.contains("@llvm.memcpy.p1.p1.i64"), "{rewritten}");
}
