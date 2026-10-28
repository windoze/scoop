use super::*;

#[test]
fn windows_debug_functions_retain_source_declarations_in_codeview() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source-声明.scoop");
    std::fs::write(
        &source,
        "\n\n\n\n\n\n\n\nfun debugProduct(value: Double): Double = -(value * 1.25 + 7.5)\n",
    )
    .unwrap();
    let source = source.canonicalize().unwrap();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        for optimization in [
            scoop_lir::OptimizationMode::Debug,
            scoop_lir::OptimizationMode::Release,
        ] {
            let mut function = super::constants::floating_function("codeViewValue", false);
            let symbol = function.symbol().to_owned();
            function.debug_source = Some(scoop_lir::FunctionDebugSource {
                name: "debugProduct".to_owned(),
                file: source.to_str().unwrap().to_owned(),
                line: 9,
                column: 1,
            });
            let mut module = values_module();
            module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
            module.output = scoop_lir::LirOutput::Library;
            module.globals = Arena::new();
            module.functions = vec![function];
            let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(optimization);
            let emitted = emit_object_set(
                &input,
                &scoop_lir::ConeCoordinate::reserved_single_file(),
                &[scoop_identity::ConeIdentity::CORE],
                scoop_lir::EntryProductionSourceV1::Library,
                directory.path(),
                profile,
            )
            .unwrap();
            let mut found = false;
            for member in emitted.members() {
                let bytes = std::fs::read(member.path()).unwrap();
                let object = object::File::parse(bytes.as_slice()).unwrap();
                if !object
                    .symbol_by_name(&symbol)
                    .is_some_and(|symbol| symbol.is_definition())
                {
                    continue;
                }
                found = true;
                let debug = object
                    .sections()
                    .filter(|section| section.name().is_ok_and(|name| name == ".debug$S"))
                    .collect::<Vec<_>>();
                if optimization == scoop_lir::OptimizationMode::Release {
                    assert!(debug.is_empty());
                    continue;
                }
                assert!(debug.iter().any(|section| {
                    section
                        .data()
                        .unwrap()
                        .windows(b"debugProduct".len())
                        .any(|value| value == b"debugProduct")
                }));
                #[cfg(windows)]
                link_and_inspect_pdb(
                    directory.path(),
                    target,
                    profile.llvm_triple(),
                    member.path(),
                    &symbol,
                );
            }
            assert!(found);
        }
    }
}

#[cfg(windows)]
fn link_and_inspect_pdb(
    directory: &std::path::Path,
    target: TargetProfileId,
    triple: &str,
    object: &std::path::Path,
    symbol: &str,
) {
    let source = directory.join("debug-consumer.c");
    std::fs::write(&source, format!(
        "extern double value(double) __asm__(\"{symbol}\");\nint main(void) {{ return value(2.0) != -10.0; }}\n"
    )).unwrap();
    let executable = directory.join("debug-consumer.exe");
    let pdb = executable.with_extension("pdb");
    let linker_flags = if target == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID {
        format!("-Wl,/debug:full,/pdb:{}", pdb.display())
    } else {
        format!("-Wl,--pdb={}", pdb.display())
    };
    let output = native_command(target, triple)
        .arg(&source)
        .arg(object)
        .arg(linker_flags)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{target:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(&executable)
            .status()
            .unwrap()
            .success()
    );
    let compiler = std::path::PathBuf::from(std::env::var_os("SCOOP_TEST_MINGW_UCRT").unwrap());
    let output = std::process::Command::new(compiler.parent().unwrap().join("llvm-pdbutil.exe"))
        .args(["dump", "--symbols", "--files", "-l"])
        .arg(pdb)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = String::from_utf8(output.stdout).unwrap();
    assert!(dump.contains("debugProduct"), "{dump}");
    assert!(dump.contains("source-声明.scoop"), "{dump}");
    let source = directory.join("source-声明.scoop").canonicalize().unwrap();
    let expected_path = dunce::simplified(&source)
        .to_str()
        .unwrap()
        .replace('\\', "/");
    let expected_file = format!("- (None: ) {expected_path}");
    assert!(
        dump.lines()
            .any(|line| line.trim().replace('\\', "/") == expected_file),
        "expected {expected_file:?}, found {:?}",
        dump.lines()
            .filter(|line| line.contains(".scoop"))
            .collect::<Vec<_>>()
    );
    assert!(
        dump.lines().any(|line| line.trim_start().starts_with("9 ")),
        "{dump}"
    );
}
