//! GCC discovery for ELF targets. C compilation does not require an unwinder.
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use object::{Architecture, BinaryFormat, Object, ObjectKind};
use scoop_lir::{
    CBridgeToolchainProfileV1, GccCompilerIdentityV1, LirTargetProfile, TargetProfileId,
    ValidatedCBridgeToolchainInvocation,
};
use scoop_process::{CommandExt, parse_make_dependencies};
use scoop_wire::{Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::ToolchainError;

pub fn resolve_linux_c_toolchain(
    target: LirTargetProfile,
    compiler: Option<&Path>,
    native_sysroot: Option<&Path>,
) -> Result<ValidatedCBridgeToolchainInvocation, ToolchainError> {
    let default_driver = match target.id() {
        TargetProfileId::LinuxX86_64Gnu => "gcc",
        TargetProfileId::LinuxX86_64Musl => "musl-gcc",
        TargetProfileId::DarwinAarch64 => {
            return Err(error("GCC discovery requires a Linux target"));
        }
    };
    let search_path = std::env::var_os("PATH").ok_or_else(|| error("PATH is not set"))?;
    let driver = find_program(
        compiler
            .unwrap_or_else(|| Path::new(default_driver))
            .as_os_str(),
        &search_path,
    )?;
    let real_gcc = find_program(
        &std::env::var_os("REALGCC").unwrap_or_else(|| OsString::from("x86_64-linux-gnu-gcc")),
        &search_path,
    )?;
    let sysroot = native_sysroot
        .map(std::fs::canonicalize)
        .transpose()
        .map_err(error)?;
    let directory = tempfile::Builder::new()
        .prefix("scoop-linux-c-")
        .tempdir()
        .map_err(error)?;
    let paths = CompilerPaths {
        driver,
        real_gcc,
        search_path,
        sysroot,
        temporary: directory.path().to_owned(),
    };
    let source = directory.path().join("probe.c");
    let object = directory.path().join("probe.o");
    let libc_check = if target.id() == TargetProfileId::LinuxX86_64Gnu {
        "#ifndef __GLIBC__\n#error glibc target requires glibc headers\n#endif\n"
    } else {
        "#ifdef __GLIBC__\n#error musl target cannot use glibc headers\n#endif\n"
    };
    std::fs::write(
        &source,
        format!("{PROBE_PREFIX}{libc_check}\nint scoop_linux_c_probe(void) {{ return 0; }}\n"),
    )
    .map_err(error)?;
    let version = command_text(paths.command().arg("-dumpfullversion"))?;
    let dependencies = run(paths
        .command()
        .args(["-v", "-M", "-MT", "c-probe"])
        .arg(&source))?;
    let dependency_text = std::str::from_utf8(&dependencies.stdout).map_err(error)?;
    let mut inputs: BTreeSet<PathBuf> = parse_make_dependencies(dependency_text, "c-probe")
        .ok_or_else(|| error("GCC returned an invalid header dependency list"))?
        .into_iter()
        .filter(|path| path != &source)
        .collect();
    inputs.insert(paths.driver.clone());
    for program in ["cc1", "as"] {
        let resolved = command_text(paths.command().arg(format!("-print-prog-name={program}")))?;
        inputs.insert(find_program(OsStr::new(&resolved), &paths.search_path)?);
    }
    for line in String::from_utf8_lossy(&dependencies.stderr).lines() {
        if let Some(specs) = line.strip_prefix("Reading specs from ") {
            inputs.insert(PathBuf::from(specs));
        }
        if let Some(compiler) = line.strip_prefix("COLLECT_GCC=") {
            inputs.insert(find_program(OsStr::new(compiler), &paths.search_path)?);
        }
    }
    let fingerprints = inputs
        .iter()
        .map(|path| std::fs::read(path).map(|bytes| sha256(&bytes)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    let compiler_inputs =
        domain_separated_cbor_hash("scoop-linux-c-inputs-v1", &InputDigests(&fingerprints))
            .map_err(error)?;
    let compiler = GccCompilerIdentityV1::new(&version, compiler_inputs).map_err(error)?;
    let profile = CBridgeToolchainProfileV1::new_linux_gcc(target, compiler).map_err(error)?;
    let invocation = ValidatedCBridgeToolchainInvocation::new_linux(
        target,
        profile,
        paths.driver,
        paths.sysroot,
        paths.real_gcc,
        paths.search_path,
    )
    .map_err(error)?;
    run(&mut invocation.object_compilation_command(&source, &object))?;
    let bytes = std::fs::read(object).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    if file.architecture() != Architecture::X86_64
        || file.format() != BinaryFormat::Elf
        || file.kind() != ObjectKind::Relocatable
        || !file.is_little_endian()
        || !file.is_64()
        || file.symbol_by_name("scoop_linux_c_probe").is_none()
    {
        return Err(error("GCC did not emit the expected ELF64 amd64 C object"));
    }
    Ok(invocation)
}

const PROBE_PREFIX: &str = "#include <features.h>\n#include <stdint.h>\n#include <stddef.h>\n#include <stdlib.h>\n#include <limits.h>\n\
#if !defined(__GNUC__) || defined(__clang__)\n#error this Linux C driver requires GCC\n#endif\n\
_Static_assert(CHAR_BIT == 8 && sizeof(void *) == 8 && sizeof(long) == 8, \"LP64 required\");\n";

struct CompilerPaths {
    driver: PathBuf,
    real_gcc: PathBuf,
    search_path: OsString,
    sysroot: Option<PathBuf>,
    temporary: PathBuf,
}

impl CompilerPaths {
    fn command(&self) -> Command {
        let mut command = Command::new(&self.driver);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("TZ", "UTC")
            .env("PATH", &self.search_path)
            .env("REALGCC", &self.real_gcc)
            .env("TMPDIR", &self.temporary);
        if let Some(sysroot) = &self.sysroot {
            command.arg("--sysroot").arg(sysroot);
        }
        command
    }
}

pub(crate) fn find_program(
    program: &OsStr,
    search_path: &OsStr,
) -> Result<PathBuf, ToolchainError> {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return std::fs::canonicalize(path).map_err(error);
    }
    for root in std::env::split_paths(search_path) {
        let candidate = root.join(path);
        if candidate.is_file() {
            return std::fs::canonicalize(candidate).map_err(error);
        }
    }
    Err(error(format!(
        "C tool not found in the selected PATH: {}",
        path.display()
    )))
}

fn run(command: &mut Command) -> Result<Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "C tool failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output)
}

fn command_text(command: &mut Command) -> Result<String, ToolchainError> {
    String::from_utf8(run(command)?.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(error)
}

struct InputDigests<'a>(&'a [scoop_wire::Digest256]);
impl WireEncode for InputDigests<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for digest in self.0 {
            digest.encode(encoder)?;
        }
        Ok(())
    }
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("Linux C toolchain: {value}"))
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
