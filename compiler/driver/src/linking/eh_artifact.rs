//! Final Mach-O exception dependency qualification for generated executables.
//!
//! Object-level LSDA checks live in codegen. This module owns the distinct
//! post-link contract: no C++ EH runtime dependency may survive, and every
//! imported Level-I unwind entry must be one of the operations qualified by
//! the Darwin/AArch64 EH profile and resolve through libSystem.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

pub(crate) const ALLOWED_UNWIND_SYMBOLS: &[&str] = &[
    "_Unwind_DeleteException",
    "_Unwind_GetIP",
    "_Unwind_GetLanguageSpecificData",
    "_Unwind_GetRegionStart",
    "_Unwind_RaiseException",
    "_Unwind_Resume",
    "_Unwind_SetGR",
    "_Unwind_SetIP",
];

pub(crate) const FORBIDDEN_EXACT_SYMBOLS: &[&str] = &[
    "__clang_call_terminate",
    "__gcc_personality_v0",
    "__gxx_personality_v0",
    "_ZSt13get_terminatev",
    "_ZSt13set_terminatePFvvE",
    "_ZSt9terminatev",
];

pub(crate) const FORBIDDEN_SYMBOL_PREFIXES: &[&str] = &["__cxa_"];
pub(crate) const FORBIDDEN_EH_LINKER_ARGUMENTS: &[&str] = &["-lc++abi", "-lunwind"];

#[derive(Debug, Default, PartialEq, Eq)]
struct ArtifactImports {
    undefined_symbols: BTreeSet<String>,
    providers: BTreeMap<String, BTreeSet<String>>,
    dynamic_libraries: BTreeSet<String>,
}

pub(super) fn verify_linker_arguments(
    libraries: &[String],
    profile_arguments: &[&str],
) -> Result<(), Vec<String>> {
    let mut violations = BTreeSet::new();

    for library in libraries {
        inspect_linker_argument(&format!("-l{library}"), &mut violations);
    }
    for argument in profile_arguments {
        inspect_linker_argument(argument, &mut violations);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.into_iter().collect())
    }
}

pub(super) fn verify_executable(binary: &Path) -> Result<(), Vec<String>> {
    let nm = run_tool("nm", &["-u"], binary)?;
    let dyld_info = run_tool("xcrun", &["dyld_info", "-imports"], binary)?;
    let otool = run_tool("otool", &["-L"], binary)?;
    qualify_outputs(&nm, &dyld_info, &otool)
}

fn run_tool(program: &str, arguments: &[&str], binary: &Path) -> Result<String, Vec<String>> {
    let output = Command::new(program)
        .args(arguments)
        .arg(binary)
        .output()
        .map_err(|error| {
            vec![format!(
                "failed to run `{}` for {}: {error}",
                tool_name(program, arguments),
                binary.display()
            )]
        })?;
    if !output.status.success() {
        return Err(vec![format!(
            "`{}` failed for {} (status {}): {}",
            tool_name(program, arguments),
            binary.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )]);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn tool_name(program: &str, arguments: &[&str]) -> String {
    std::iter::once(program)
        .chain(arguments.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

fn inspect_linker_argument(argument: &str, violations: &mut BTreeSet<String>) {
    let forbidden = FORBIDDEN_EH_LINKER_ARGUMENTS
        .iter()
        .copied()
        .find(|forbidden| argument == *forbidden)
        .or_else(|| {
            argument
                .strip_prefix("-Wl,")
                .and_then(|arguments| arguments.split(',').find(is_forbidden_linker_argument))
        });
    if let Some(forbidden) = forbidden {
        violations.insert(format!(
            "explicit exception runtime linker argument `{forbidden}` is forbidden"
        ));
    }
}

fn is_forbidden_linker_argument(argument: &&str) -> bool {
    FORBIDDEN_EH_LINKER_ARGUMENTS.contains(argument)
}

fn qualify_outputs(nm: &str, dyld_info: &str, otool: &str) -> Result<(), Vec<String>> {
    let artifact = ArtifactImports {
        undefined_symbols: parse_nm_undefined_symbols(nm),
        providers: parse_dyld_imports(dyld_info),
        dynamic_libraries: parse_otool_libraries(otool),
    };
    let mut violations = BTreeSet::new();

    let symbols = artifact
        .undefined_symbols
        .iter()
        .chain(artifact.providers.keys())
        .collect::<BTreeSet<_>>();
    for symbol in symbols {
        if is_forbidden_symbol(symbol) {
            violations.insert(format!("forbidden exception symbol `{symbol}` is imported"));
        }
        if symbol.starts_with("_Unwind_") && !ALLOWED_UNWIND_SYMBOLS.contains(&symbol.as_str()) {
            violations.insert(format!("unqualified unwind symbol `{symbol}` is imported"));
        }
    }

    for symbol in artifact
        .undefined_symbols
        .iter()
        .filter(|symbol| symbol.starts_with("_Unwind_"))
    {
        let Some(providers) = artifact.providers.get(symbol) else {
            violations.insert(format!(
                "cannot determine the dynamic provider for unwind symbol `{symbol}`"
            ));
            continue;
        };
        for provider in providers {
            if !is_libsystem_provider(provider) {
                violations.insert(format!(
                    "unwind symbol `{symbol}` resolves from `{provider}` instead of libSystem"
                ));
            }
        }
    }

    for (symbol, providers) in &artifact.providers {
        if symbol.starts_with("_Unwind_") {
            for provider in providers {
                if !is_libsystem_provider(provider) {
                    violations.insert(format!(
                        "unwind symbol `{symbol}` resolves from `{provider}` instead of libSystem"
                    ));
                }
            }
        }
        for provider in providers {
            if is_libcxxabi(provider) {
                violations.insert(format!(
                    "symbol `{symbol}` resolves from forbidden provider `{provider}`"
                ));
            }
        }
    }

    for library in &artifact.dynamic_libraries {
        if is_libcxxabi(library) {
            violations.insert(format!(
                "forbidden dynamic library dependency `{library}` is loaded"
            ));
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.into_iter().collect())
    }
}

fn parse_nm_undefined_symbols(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.ends_with(':') {
                return None;
            }
            let raw = line.split_whitespace().next_back()?;
            raw.starts_with('_')
                .then(|| normalize_macho_symbol(raw).to_string())
        })
        .collect()
}

fn parse_dyld_imports(output: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut imports = BTreeMap::<String, BTreeSet<String>>::new();
    for line in output.lines() {
        let Some((symbol_fields, provider_fields)) = line.rsplit_once("(from ") else {
            continue;
        };
        let Some((provider, _)) = provider_fields.split_once(')') else {
            continue;
        };
        let Some(raw_symbol) = symbol_fields.split_whitespace().next_back() else {
            continue;
        };
        if !raw_symbol.starts_with('_') {
            continue;
        }
        imports
            .entry(normalize_macho_symbol(raw_symbol).to_string())
            .or_default()
            .insert(provider.trim().to_string());
    }
    imports
}

fn parse_otool_libraries(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (library, _) = line.split_once(" (")?;
            Some(library.to_string())
        })
        .collect()
}

fn normalize_macho_symbol(symbol: &str) -> &str {
    symbol.strip_prefix('_').unwrap_or(symbol)
}

fn is_forbidden_symbol(symbol: &str) -> bool {
    FORBIDDEN_EXACT_SYMBOLS.contains(&symbol)
        || FORBIDDEN_SYMBOL_PREFIXES
            .iter()
            .any(|prefix| symbol.starts_with(prefix))
        || (symbol.starts_with("_Z") && symbol.contains("terminate"))
}

fn is_libsystem_provider(provider: &str) -> bool {
    provider == "libSystem"
        || Path::new(provider)
            .file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file == "libSystem.B.dylib")
}

fn is_libcxxabi(name: &str) -> bool {
    name.to_ascii_lowercase().contains("libc++abi")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const CLEAN_NM: &str = "\
/tmp/program (for architecture arm64):\n\
__Unwind_DeleteException\n\
__Unwind_Resume\n\
_abort\n";

    const CLEAN_DYLD_INFO: &str = "\
/tmp/program [arm64]:\n\
    -imports:\n\
      0x0000  __Unwind_DeleteException  (from libSystem)\n\
      0x0001  __Unwind_Resume  (from libSystem)\n\
      0x0002  _abort  (from libSystem)\n";

    const CLEAN_OTOOL: &str = "\
/tmp/program:\n\
\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)\n";

    struct HarnessBinary(PathBuf);

    impl Drop for HarnessBinary {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("driver crate is nested below the workspace root")
            .to_path_buf()
    }

    fn qualify_runtime_harness(
        name: &str,
        arguments: &[&str],
        include_directories: &[&str],
        sources: &[&str],
        expected_stdout: &[u8],
    ) {
        verify_linker_arguments(&[], arguments).unwrap_or_else(|violations| {
            panic!("{name} uses forbidden linker arguments: {violations:#?}")
        });

        let workspace = workspace_root();
        let binary = HarnessBinary(std::env::temp_dir().join(format!(
            "scoop_driver_{name}_{}_self_audit",
            std::process::id()
        )));
        let mut command = Command::new("cc");
        command.args(arguments);
        for include in include_directories {
            command.arg("-I").arg(workspace.join(include));
        }
        for source in sources {
            command.arg(workspace.join(source));
        }
        let compile = command
            .arg("-o")
            .arg(&binary.0)
            .output()
            .unwrap_or_else(|error| panic!("compile {name}: {error}"));
        assert!(
            compile.status.success(),
            "{name} must compile cleanly:\n{}",
            String::from_utf8_lossy(&compile.stderr)
        );

        verify_executable(&binary.0)
            .unwrap_or_else(|violations| panic!("{name} failed final EH gate: {violations:#?}"));
        let output = Command::new(&binary.0)
            .output()
            .unwrap_or_else(|error| panic!("run {name}: {error}"));
        assert!(
            output.status.success(),
            "{name} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected_stdout, "unexpected {name} output");
    }

    #[test]
    fn macho_normalization_removes_exactly_the_platform_prefix() {
        assert_eq!(normalize_macho_symbol("___cxa_throw"), "__cxa_throw");
        assert_eq!(normalize_macho_symbol("__Unwind_Resume"), "_Unwind_Resume");
        assert_eq!(normalize_macho_symbol("_abort"), "abort");
        assert_eq!(normalize_macho_symbol("plain"), "plain");
    }

    #[test]
    fn parsers_collect_normalized_symbols_providers_and_dependencies() {
        let undefined = parse_nm_undefined_symbols(CLEAN_NM);
        assert!(undefined.contains("_Unwind_DeleteException"));
        assert!(undefined.contains("_Unwind_Resume"));
        assert!(!undefined.contains("/tmp/program"));

        let providers = parse_dyld_imports(CLEAN_DYLD_INFO);
        assert_eq!(
            providers.get("_Unwind_Resume"),
            Some(&BTreeSet::from(["libSystem".to_string()]))
        );
        assert_eq!(
            providers.get("abort"),
            Some(&BTreeSet::from(["libSystem".to_string()]))
        );

        assert_eq!(
            parse_otool_libraries(CLEAN_OTOOL),
            BTreeSet::from(["/usr/lib/libSystem.B.dylib".to_string()])
        );
    }

    #[test]
    fn closed_level_one_import_set_from_libsystem_is_accepted() {
        assert_eq!(
            qualify_outputs(CLEAN_NM, CLEAN_DYLD_INFO, CLEAN_OTOOL),
            Ok(())
        );
    }

    #[test]
    fn cxx_abi_symbols_and_dependency_are_rejected() {
        let nm = format!("{CLEAN_NM}___cxa_throw\n___gxx_personality_v0\n");
        let imports = format!(
            "{CLEAN_DYLD_INFO}      0x0003  ___cxa_throw  (from libc++abi)\n\
             0x0004  ___gxx_personality_v0  (from libc++abi)\n"
        );
        let libraries = format!(
            "{CLEAN_OTOOL}\t/usr/lib/libc++abi.dylib \
             (compatibility version 1.0.0, current version 2100.43.0)\n"
        );
        let violations =
            qualify_outputs(&nm, &imports, &libraries).expect_err("must reject C++ EH");

        assert!(
            violations
                .iter()
                .any(|message| message.contains("forbidden exception symbol `__cxa_throw`"))
        );
        assert!(
            violations.iter().any(
                |message| message.contains("forbidden exception symbol `__gxx_personality_v0`")
            )
        );
        assert!(
            violations
                .iter()
                .any(|message| message.contains("libc++abi.dylib"))
        );
    }

    #[test]
    fn complete_pre_m25_import_baseline_is_rejected() {
        let nm = "\
__Unwind_Resume\n\
__ZSt13set_terminatePFvvE\n\
___cxa_allocate_exception\n\
___cxa_begin_catch\n\
___cxa_current_primary_exception\n\
___cxa_end_catch\n\
___cxa_rethrow\n\
___cxa_throw\n\
___gxx_personality_v0\n";
        let imports = "\
      0x0000  __ZSt13set_terminatePFvvE  (from libc++abi)\n\
      0x0001  ___cxa_allocate_exception  (from libc++abi)\n\
      0x0002  ___cxa_begin_catch  (from libc++abi)\n\
      0x0003  ___cxa_current_primary_exception  (from libc++abi)\n\
      0x0004  ___cxa_end_catch  (from libc++abi)\n\
      0x0005  ___cxa_rethrow  (from libc++abi)\n\
      0x0006  ___cxa_throw  (from libc++abi)\n\
      0x0007  ___gxx_personality_v0  (from libc++abi)\n\
      0x0008  __Unwind_Resume  (from libSystem)\n";
        let libraries = "\
/tmp/program:\n\
\t/usr/lib/libc++abi.dylib (compatibility version 1.0.0, current version 2100.43.0)\n\
\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)\n";
        let violations =
            qualify_outputs(nm, imports, libraries).expect_err("pre-M25 artifact must fail");

        for symbol in [
            "_ZSt13set_terminatePFvvE",
            "__cxa_allocate_exception",
            "__cxa_begin_catch",
            "__cxa_current_primary_exception",
            "__cxa_end_catch",
            "__cxa_rethrow",
            "__cxa_throw",
            "__gxx_personality_v0",
        ] {
            assert!(
                violations.iter().any(|message| message.contains(symbol)),
                "missing rejection for {symbol}: {violations:#?}"
            );
        }
    }

    #[test]
    fn cxx_terminate_variants_are_rejected() {
        for symbol in [
            "__clang_call_terminate",
            "_ZSt13set_terminatePFvvE",
            "_ZSt9terminatev",
            "_ZN10__cxxabiv111__terminateEPFvvE",
        ] {
            assert!(is_forbidden_symbol(symbol), "did not reject {symbol}");
        }
    }

    #[test]
    fn unwind_superset_and_non_system_provider_are_rejected() {
        let nm = format!("{CLEAN_NM}__Unwind_Resume_or_Rethrow\n");
        let imports = CLEAN_DYLD_INFO.replace(
            "__Unwind_Resume  (from libSystem)",
            "__Unwind_Resume  (from libunwind)\n\
             0x0003  __Unwind_Resume_or_Rethrow  (from libSystem)",
        );
        let violations = qualify_outputs(&nm, &imports, CLEAN_OTOOL)
            .expect_err("must reject provider and unwind superset");

        assert!(violations.iter().any(|message| {
            message.contains("unqualified unwind symbol `_Unwind_Resume_or_Rethrow`")
        }));
        assert!(violations.iter().any(|message| {
            message.contains("`_Unwind_Resume` resolves from `libunwind` instead of libSystem")
        }));
    }

    #[test]
    fn missing_unwind_provider_is_rejected() {
        let violations = qualify_outputs(CLEAN_NM, "", CLEAN_OTOOL)
            .expect_err("undefined unwind imports require provider evidence");
        assert!(violations.iter().any(|message| {
            message.contains("cannot determine the dynamic provider for unwind symbol")
        }));
    }

    #[test]
    fn explicit_exception_runtime_linker_arguments_are_rejected() {
        assert_eq!(verify_linker_arguments(&[], &["-pthread"]), Ok(()));
        let violations = verify_linker_arguments(
            &["unwind".to_string()],
            &["-pthread", "-Wl,-dead_strip,-lc++abi"],
        )
        .expect_err("must reject both explicit exception runtime libraries");
        assert_eq!(violations.len(), 2);
        assert!(
            violations
                .iter()
                .any(|message| message.contains("-lc++abi"))
        );
        assert!(
            violations
                .iter()
                .any(|message| message.contains("-lunwind"))
        );
    }

    #[test]
    fn lifecycle_harness_passes_the_production_final_eh_gate() {
        qualify_runtime_harness(
            "eh_lifecycle",
            &["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread"],
            &["runtime/include", "runtime/src"],
            &["runtime/src/eh.c", "runtime/tests/eh_lifecycle_test.c"],
            b"EH lifecycle tests passed\n",
        );
    }

    #[test]
    fn personality_harness_passes_the_production_final_eh_gate() {
        qualify_runtime_harness(
            "eh_personality",
            &["-std=c11", "-Wall", "-Wextra", "-Werror"],
            &["runtime/src"],
            &[
                "runtime/src/eh_personality.c",
                "runtime/tests/eh_personality_test.c",
            ],
            b"EH personality tests passed\n",
        );
    }
}
