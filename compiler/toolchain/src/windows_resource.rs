//! Compile executable resources with the selected Windows preprocessing environment.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use scoop_manifest::{ConeRelativePath, LoadedConeManifest};
use scoop_process::CommandExt;
use scoop_protocol::{CompilationPurpose, HostPathCarrier};
use scoop_wire::Digest256;
use serde::{Deserialize, Serialize};

use crate::build_record::{Input, Record, cached, fingerprint};
use crate::{ResolvedTargetProfile, ToolchainError};

mod assets;

#[cfg(all(test, windows))]
mod tests;

pub(crate) struct PreparedResource {
    pub(crate) key: Digest256,
    pub(crate) object: PathBuf,
    pub(crate) inputs: Vec<Input>,
}

#[derive(Serialize, Deserialize)]
struct ResourceOutput {
    object: HostPathCarrier,
}

pub(crate) fn source<'a>(
    manifest: &'a LoadedConeManifest,
    target: &ResolvedTargetProfile,
    purpose: CompilationPurpose,
) -> Option<&'a ConeRelativePath> {
    if purpose != CompilationPurpose::Production
        || !target
            .c_bridge_toolchain()
            .native_environment()
            .is_windows()
    {
        return None;
    }
    manifest.parsed().semantic().windows().rc()
}

pub(crate) fn prepare(
    manifest: &LoadedConeManifest,
    target: &ResolvedTargetProfile,
    purpose: CompilationPurpose,
    sysroot: &Path,
    cache: &Path,
) -> Result<Option<PreparedResource>, ToolchainError> {
    let Some(source) = source(manifest, target, purpose) else {
        return Ok(None);
    };
    let target_root = crate::target_library_root(sysroot, target.lir_target());
    let includes: Vec<_> = manifest
        .parsed()
        .semantic()
        .native()
        .include_paths(target.id(), manifest.real_root(), &target_root)
        .map(|path| path_carrier(&path))
        .collect::<Result<_, _>>()?;
    let search = std::env::var_os("PATH").unwrap_or_default();
    let key = fingerprint(&(
        path_carrier(manifest.real_root())?,
        source,
        includes,
        crate::native_record::ResolvedRecord::from_target(target)?,
        search.as_encoded_bytes(),
    ))?;
    let directory = cache.join("native-resources-v2");
    let record = cached(
        &directory,
        key,
        |output: &ResourceOutput| {
            output
                .object
                .to_path_buf()
                .map_err(error)?
                .try_exists()
                .map_err(error)
        },
        || {
            let temporary = tempfile::Builder::new()
                .prefix("resource-")
                .tempdir_in(&directory)
                .map_err(error)?;
            let (object, paths) =
                compile(manifest, target, source, &target_root, temporary.path())?;
            let inputs = paths
                .iter()
                .map(|path| Input::read(path))
                .collect::<Result<_, _>>()?;
            let result = ResourceOutput {
                object: path_carrier(&object)?,
            };
            let _ = temporary.keep();
            Ok(Record { inputs, result })
        },
    )?;
    Ok(Some(PreparedResource {
        key: fingerprint(&(key.as_array(), &record.inputs))?,
        object: record.result.object.to_path_buf().map_err(error)?,
        inputs: record.inputs,
    }))
}

fn compile(
    manifest: &LoadedConeManifest,
    target: &ResolvedTargetProfile,
    source: &ConeRelativePath,
    target_root: &Path,
    directory: &Path,
) -> Result<(PathBuf, BTreeSet<PathBuf>), ToolchainError> {
    let invocation = target.c_bridge_toolchain();
    let environment = invocation.windows_environment().map_err(error)?;
    let mut tools = BTreeSet::new();
    let windres = find_tool("llvm-windres", invocation, &mut tools)?;
    let cvtres = find_tool("llvm-cvtres", invocation, &mut tools)?;
    let version = run(environment.bare_command(&windres).arg("--version"))?;
    if !String::from_utf8_lossy(&version.stdout)
        .lines()
        .any(|line| line.trim().starts_with("LLVM version 22.1."))
    {
        return Err(error(
            "resource compilation requires LLVM 22.1 llvm-windres",
        ));
    }
    let input = manifest.real_root().join(source.as_path());
    let dependencies = directory.join("headers.d");
    let resource = directory.join("program.res");
    let object = directory.join("resource.obj");
    let mut command = environment.bare_command(&windres);
    command
        .current_dir(dunce::simplified(manifest.real_root()))
        .args(["--use-temp-file", "--verbose", "--codepage=65001"])
        .arg(joined(
            "--preprocessor=",
            invocation.compiler_driver().as_os_str(),
        ));
    for argument in invocation.driver_command().get_args() {
        command.arg(joined("--preprocessor-arg=", argument));
    }
    for argument in [
        "-E",
        "-xc",
        "-DRC_INVOKED",
        "-v",
        "-MD",
        "-MT",
        "resource-unit",
        "-MF",
    ] {
        command.arg(joined("--preprocessor-arg=", argument.as_ref()));
    }
    command.arg(joined(
        "--preprocessor-arg=",
        dunce::simplified(&dependencies).as_os_str(),
    ));
    let mut includes = vec![manifest.real_root().to_path_buf()];
    includes.extend(manifest.parsed().semantic().native().include_paths(
        target.id(),
        manifest.real_root(),
        target_root,
    ));
    includes.extend(environment.includes.iter().cloned());
    if let Some(prefix) = &environment.native_sysroot {
        if target.id() != scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID {
            includes.push(prefix.join("x86_64-w64-mingw32/include"));
        }
    }
    for include in &includes {
        command.arg("-I").arg(dunce::simplified(include));
    }
    command
        .arg("-i")
        .arg(dunce::simplified(&input))
        .args(["-O", "res", "-o"])
        .arg(dunce::simplified(&resource));
    let output = run(&mut command)?;
    let mut paths: BTreeSet<_> = scoop_process::parse_make_dependencies(
        &std::fs::read_to_string(&dependencies).map_err(error)?,
        "resource-unit",
    )
    .ok_or_else(|| error("Clang returned invalid RC header dependencies"))?
    .into_iter()
    .collect();
    crate::native_record::header_candidates(&output.stderr, &mut paths);
    paths.insert(input.clone());
    assets::observe(
        std::str::from_utf8(&output.stdout).map_err(error)?,
        manifest.real_root(),
        input.parent().expect("resource input has a parent"),
        &includes,
        &mut paths,
    )?;
    paths.extend(tools);
    let mut convert = environment.bare_command(&cvtres);
    convert
        .args(["/nologo", "/machine:x64"])
        .arg(joined("/out:", dunce::simplified(&object).as_os_str()))
        .arg(dunce::simplified(&resource));
    run(&mut convert)?;
    Ok((object, paths))
}

fn find_tool(
    name: &str,
    invocation: &crate::ValidatedCBridgeToolchainInvocation,
    inputs: &mut BTreeSet<PathBuf>,
) -> Result<PathBuf, ToolchainError> {
    let environment = invocation.windows_environment().map_err(error)?;
    let executable = crate::host_executable_name(name);
    let mut candidates = vec![invocation.compiler_driver().with_file_name(&executable)];
    if let Some(prefix) = &environment.native_sysroot {
        candidates.push(prefix.join("bin").join(&executable));
    }
    candidates
        .extend(std::env::split_paths(&environment.search_path).map(|root| root.join(&executable)));
    for candidate in candidates {
        inputs.insert(candidate.clone());
        if candidate.is_file() {
            return std::path::absolute(&candidate).map_err(error);
        }
    }
    Err(error(format!(
        "{name} not found in the selected LLVM tools, native prefix, or PATH"
    )))
}

fn joined(prefix: &str, suffix: &std::ffi::OsStr) -> OsString {
    let mut value = OsString::from(prefix);
    value.push(suffix);
    value
}

fn run(command: &mut Command) -> Result<Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "{} failed: {}{}",
            command.get_program().to_string_lossy(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

fn path_carrier(path: &Path) -> Result<HostPathCarrier, ToolchainError> {
    HostPathCarrier::from_path(path).map_err(error)
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("Windows resources: {value}"))
}
