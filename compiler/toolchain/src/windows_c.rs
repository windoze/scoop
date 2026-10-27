//! Generated-C discovery for the three Windows native environments.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use object::{Object, ObjectKind};
use scoop_lir::{LirTargetProfile, TargetProfileId, WindowsClangCompilerIdentityV1};
use scoop_process::CommandExt;
use scoop_wire::sha256;

use crate::{ToolchainError, ValidatedCBridgeToolchainInvocation};

mod environment;
pub(crate) use environment::WindowsCEnvironment;

pub(crate) fn machine_triple(target: TargetProfileId) -> Result<&'static str, ToolchainError> {
    match target {
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID => Ok("x86_64-pc-windows-msvc"),
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID
        | scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID => Ok("x86_64-w64-windows-gnu"),
        _ => Err(error("a registered Windows target is required")),
    }
}

pub fn resolve_windows_c_toolchain(
    target: LirTargetProfile,
    compiler: &Path,
    native_sysroot: Option<&Path>,
) -> Result<ValidatedCBridgeToolchainInvocation, ToolchainError> {
    probe(target, compiler, native_sysroot).map(|(invocation, _)| invocation)
}

pub(crate) fn probe(
    target: LirTargetProfile,
    compiler: &Path,
    native_sysroot: Option<&Path>,
) -> Result<(ValidatedCBridgeToolchainInvocation, Vec<PathBuf>), ToolchainError> {
    let triple = machine_triple(target.id())?;
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let compiler = crate::linux_c::find_program(compiler.as_os_str(), &search_path)?;
    let (environment, paths) = WindowsCEnvironment::resolve(target, &compiler, native_sysroot)?;
    let mut inputs: BTreeSet<_> = paths.into_iter().collect();
    inputs.insert(compiler.clone());
    let directory = tempfile::Builder::new()
        .prefix("scoop-windows-c-")
        .tempdir()
        .map_err(error)?;
    let source = directory.path().join("probe.c");
    let object = directory.path().join("probe.obj");
    let microsoft = target.id() == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID;
    let ucrt = target.id() != scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID;
    std::fs::write(&source, format!("{PROBE}\n#if defined(__MINGW32__) == {microsoft}\n#error wrong Windows native environment\n#endif\n#if defined(_UCRT) != {ucrt}\n#error wrong Windows CRT headers\n#endif\nvoid *scoop_windows_c_probe(void) {{ return (void *)&_errno; }}\n",
        microsoft = u8::from(microsoft), ucrt = u8::from(ucrt))).map_err(error)?;
    let make_command = || environment.command(&compiler, triple, microsoft);
    let dependencies = run(make_command()
        .args(["-v", "-M", "-MT", "c-probe"])
        .arg(&source))?;
    let text = std::str::from_utf8(&dependencies.stdout).map_err(error)?;
    inputs.extend(
        scoop_process::parse_make_dependencies(text, "c-probe")
            .ok_or_else(|| error("Clang returned an invalid header dependency list"))?
            .into_iter()
            .filter(|path| path != &source),
    );
    crate::native_record::header_candidates(&dependencies.stderr, &mut inputs);
    let macros = run(make_command().args(["-dM", "-E"]).arg(&source))?;
    let macros_text = std::str::from_utf8(&macros.stdout).map_err(error)?;
    let version = [
        macro_number(macros_text, "__clang_major__")?,
        macro_number(macros_text, "__clang_minor__")?,
        macro_number(macros_text, "__clang_patchlevel__")?,
    ];
    let identity = WindowsClangCompilerIdentityV1::new(version, sha256(&macros.stdout));
    let profile = scoop_target_data::windows_c_bridge_profile(target, identity).map_err(error)?;
    let invocation =
        ValidatedCBridgeToolchainInvocation::new_windows(target, profile, compiler, environment)
            .map_err(error)?;
    run(&mut invocation.object_compilation_command(&source, &object))?;
    let bytes = std::fs::read(object).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    if file.architecture() != object::Architecture::X86_64
        || file.format() != object::BinaryFormat::Coff
        || file.kind() != ObjectKind::Relocatable
        || file.symbol_by_name("scoop_windows_c_probe").is_none()
    {
        return Err(error("Clang did not emit the selected AMD64 COFF C object"));
    }
    Ok((invocation, inputs.into_iter().collect()))
}

fn macro_number(macros: &str, name: &str) -> Result<u32, ToolchainError> {
    let prefix = format!("#define {name} ");
    macros
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| error(format!("Clang did not define numeric {name}")))
}

const PROBE: &str = r#"
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <errno.h>
#include <windows.h>
#if !defined(__clang__) || !defined(_WIN64) || !defined(__x86_64__)
#error the Windows generated-C producer requires x64 Clang
#endif
_Static_assert(sizeof(void *) == 8 && sizeof(long) == 4 && sizeof(wchar_t) == 2,
    "Windows LLP64 required");
"#;

fn run(command: &mut Command) -> Result<Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "{}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output)
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("Windows C toolchain: {value}"))
}
