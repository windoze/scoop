use super::*;

#[test]
fn windows_caller_copies_keep_sret_without_byval_through_statepoints() {
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        let profile =
            ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
                .unwrap();
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let mut module = aggregate_abi_module();
        module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
        let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
        llvm.verify().unwrap();
        let ir = llvm.print_to_string().to_string();
        assert!(!ir.contains("byval("), "{ir}");
        assert!(ir.contains("sret({ i64, i64, i64 }) align 8"), "{ir}");
        assert!(ir.contains("alloca { i64, i64, i64 }, align 16"), "{ir}");
        statepoint::rewrite(
            &llvm,
            &machine,
            &statepoint::expectations(&module).unwrap(),
            profile,
        )
        .unwrap();
        llvm.verify().unwrap();
        let ir = llvm.print_to_string().to_string();
        assert!(!ir.contains("byval("), "{ir}");
        assert!(ir.contains("sret({ i64, i64, i64 }) align 8"), "{ir}");
        machine
            .write_to_memory_buffer(&llvm, FileType::Object)
            .unwrap();
    }
}

#[cfg(windows)]
#[test]
fn windows_aggregate_values_roundtrip_between_scoop_and_native_c() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        let profile =
            ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
                .unwrap()
                .with_optimization(scoop_lir::OptimizationMode::Release);
        let mut module = aggregate_abi_module();
        module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
        module.output = scoop_lir::LirOutput::Library;
        module.functions.truncate(2);
        module
            .functions
            .push(aggregate_identity("windowsNativeIdentity", GcEffect::NoGc));
        let caller = &mut module.functions[1];
        let (locals, argument, result) = aggregate_call_storage();
        caller.locals = locals;
        caller.temps = Arena::new();
        caller.signature = aggregate_signature();
        caller.call_targets = CallTargets::default();
        let site = aggregate_call(
            &mut caller.call_targets,
            TestCallProtocol::NoGc {
                destination: no_gc_local(0),
            },
            argument,
            result,
            &caller.locals,
        );
        caller.blocks[caller.entry].instructions = vec![
            Instruction::Store {
                local: argument,
                value: Value::Param(0),
            },
            Instruction::Call { site },
        ]
        .into();
        caller.blocks[caller.entry].terminator = Terminator::Return {
            value: Some(Value::Local(result)),
        };
        let native_symbol = module.functions[0].symbol().to_string();
        let caller_symbol = module.functions[1].symbol().to_string();
        let identity_symbol = module.functions[2].symbol().to_string();
        let native_body = module.functions[0].callable_body.id();
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let emitted = emit_object_set(
            &input,
            &scoop_lir::ConeCoordinate::reserved_single_file(),
            &[scoop_identity::ConeIdentity::CORE],
            scoop_lir::EntryProductionSourceV1::Library,
            directory.path(),
            profile,
        )
        .unwrap();
        let source = directory.path().join("win64-aggregate.c");
        std::fs::write(
            &source,
            format!(
                r#"
typedef struct {{ long long a, b, c; }} Triple;
extern Triple native_transform(Triple) __asm__("{native_symbol}");
extern Triple scoop_forward(Triple) __asm__("{caller_symbol}");
extern Triple scoop_identity(Triple) __asm__("{identity_symbol}");
Triple native_transform(Triple value) {{
    value.a += 10; value.b += 20; value.c += 30;
    return value;
}}
int main(void) {{
    Triple value = {{1, -2, 30000000000LL}};
    Triple direct = scoop_identity(value);
    Triple forwarded = scoop_forward(value);
    return direct.a != 1 || direct.b != -2 || direct.c != 30000000000LL
        || forwarded.a != 11 || forwarded.b != 18 || forwarded.c != 30000000030LL
        || value.a != 1 || value.b != -2 || value.c != 30000000000LL;
}}
"#
            ),
        )
        .unwrap();
        let executable = directory.path().join("win64-aggregate.exe");
        let result = crate::tests::windows_codegen::native_command(target, profile.llvm_triple())
            .arg("-O2")
            .arg(&source)
            .args(
                emitted
                    .members()
                    .iter()
                    .filter(|member| {
                        matches!(member.units().kind(),
                            ScoopLirObjectKindV1::CallableBody(body) if body != native_body
                        )
                    })
                    .map(|member| member.path()),
            )
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{target:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            std::process::Command::new(executable)
                .status()
                .unwrap()
                .success()
        );
    }
}
