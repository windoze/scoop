use super::*;
use inkwell::values::AnyValue;

#[test]
fn gc_leaf_call_preserves_side_effects_without_a_gc_boundary() {
    let mut module = Module {
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
        functions: Vec::new(),
        output: scoop_lir::LirOutput::Library,
        meta: string_metadata(),
    };
    let bridge = outbound_bridge(1);
    let bridge_symbol = bridge.symbol().to_owned();
    let native = module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "leaf".into(),
            native_symbol: "native_leaf".into(),
            library: "fixture".into(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        call_mode: scoop_lir::CAbiCallMode::GcLeaf,
        bridge,
        signature: scoop_lir::CFunctionType {
            params: Vec::new(),
            return_type: scoop_lir::CReturnType::Void,
        },
    });
    let mut targets = CallTargets::default();
    let signature = targets
        .void_signatures
        .alloc(scoop_lir::VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
    let target = targets.c_targets.void.alloc(scoop_lir::CallTarget {
        destination: scoop_lir::CCallDestination::extern_function(native),
        signature,
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![Instruction::Call {
            site: CallSite::NativeGcLeaf(scoop_lir::NativeGcLeafCallSite {
                call: scoop_lir::CTypedCall::Void {
                    target,
                    args: Vec::new(),
                },
            }),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let function = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: Default::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: targets,
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    };
    let symbol = function.symbol().to_owned();
    module.functions = vec![function];
    module.output = scoop_lir::LirOutput::Executable {
        entry: managed_function_ref(0),
    };
    install_test_native_function_contract(&mut module, 1);
    refresh_module_safepoints(&mut module);
    let machine = host_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    statepoint::rewrite(
        &llvm,
        &machine,
        &statepoint::expectations(&module).unwrap(),
        host_profile(),
    )
    .unwrap();
    llvm.verify().unwrap();
    let body = llvm
        .get_function(&symbol)
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        body.contains(&bridge_symbol),
        "the observable native call was removed: {body}"
    );
    for forbidden in [
        "gc.statepoint",
        "gc.relocate",
        "native_safe",
        "caller_root",
        "readnone",
        "readonly",
        "nosync",
    ] {
        assert!(!body.contains(forbidden), "unexpected {forbidden}: {body}");
    }
}
