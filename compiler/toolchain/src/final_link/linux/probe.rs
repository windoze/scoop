use super::*;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::process::Output;

pub(super) fn driver_program(
    startup: &ValidatedCBridgeToolchainInvocation,
    name: &str,
) -> Result<PathBuf, ToolchainError> {
    let mut command = startup.driver_command();
    let path = command
        .get_envs()
        .find(|(key, _)| *key == OsStr::new("PATH"))
        .and_then(|(_, value)| value)
        .ok_or_else(|| error("Linux C invocation is missing PATH"))?
        .to_owned();
    let output = run(command.arg(format!("-print-prog-name={name}")))?;
    let program = String::from_utf8(output.stdout).map_err(error)?;
    crate::linux_c::find_program(OsStr::new(program.trim()), &path)
}

pub(super) fn check(profile: &LinuxFinalLinkProfile) -> Result<Vec<SystemInput>, ToolchainError> {
    let directory = tempfile::Builder::new()
        .prefix("scoop-elf-link-")
        .tempdir()
        .map_err(error)?;
    let source = directory.path().join("probe.c");
    let object = directory.path().join("probe.o");
    std::fs::write(&source, SOURCE).map_err(error)?;
    let mut compile = profile.startup.object_compilation_command(&source, &object);
    compile.args(["-funwind-tables", "-fno-omit-frame-pointer"]);
    if let Some(prefix) = profile.unwind_prefix() {
        compile.arg("-I").arg(prefix.join("include"));
    }
    run(&mut compile)?;
    let binary = directory.path().join("probe");
    let map = directory.path().join("probe.map");
    let mut command = profile.command(directory.path(), &binary, &map)?;
    command.arg(object);
    profile.append_system_libraries(&mut command);
    let result = run(&mut command)?;
    let bytes = std::fs::read(&binary).map_err(error)?;
    profile.check_image(&bytes)?;
    let trace = String::from_utf8(result.stdout).map_err(error)?;
    let map = std::fs::read_to_string(map).map_err(error)?;
    profile.check_unwind_map(&map)?;
    let mut paths = BTreeSet::from([
        profile.linker.clone(),
        driver_program(&profile.startup, "collect2")?,
    ]);
    if let Some(prefix) = profile.unwind_prefix() {
        paths.insert(prefix.join("lib/libunwind.a"));
    }
    for line in trace.lines() {
        let path = Path::new(line.trim());
        if path.is_absolute() && !path.starts_with(directory.path()) {
            if !path.is_file() {
                return Err(error(format!(
                    "linker trace names a missing input {}",
                    path.display()
                )));
            }
            paths.insert(path.to_owned());
        }
    }
    if !paths.iter().any(|path| {
        path.file_name()
            .is_some_and(|name| name == "libc.so" || name == "libc.a")
    }) {
        return Err(error(
            "linker trace does not identify the selected libc development input",
        ));
    }
    paths
        .into_iter()
        .map(|path| {
            let digest = sha256(&std::fs::read(&path).map_err(error)?);
            Ok(SystemInput { path, digest })
        })
        .collect()
}

pub(super) fn run(command: &mut Command) -> Result<Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "Linux native tool {command:?} failed ({}): {}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

pub(super) const SOURCE: &str = "#include <stdio.h>\n#include <unwind.h>\n\
static _Unwind_Reason_Code trace(struct _Unwind_Context *context, void *data) {\n\
    (void)context; (*(int *)data)++; return _URC_NO_REASON;\n}\n\
static __attribute__((noinline)) int count_frames(void) {\n\
    int frames = 0; _Unwind_Backtrace(trace, &frames); return frames;\n}\n\
int main(void) { return count_frames() < 2 ||\n\
    puts(\"LLVM unwinder and target libc passed\") < 0; }\n";
