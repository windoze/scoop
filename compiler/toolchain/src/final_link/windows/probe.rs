use super::*;
use object::{Object, ObjectSection};
use scoop_process::CommandExt;
use scoop_wire::sha256;

pub(super) fn check(
    profile: &WindowsFinalLinkProfile,
    observed: &mut BTreeSet<PathBuf>,
) -> Result<Vec<SystemInput>, ToolchainError> {
    let directory = tempfile::Builder::new()
        .prefix("scoop-pe-link-")
        .tempdir()
        .map_err(error)?;
    let source = directory.path().join("probe.c");
    let object = directory.path().join("probe.obj");
    let depfile = directory.path().join("probe.d");
    std::fs::write(&source, SOURCE).map_err(error)?;
    let mut compile = profile.startup.object_compilation_command(&source, &object);
    compile
        .args(["-v", "-MD", "-MT", "link-probe", "-MF"])
        .arg(&depfile)
        .arg("-I")
        .arg(profile.unwind_prefix().join("include"));
    let compiled = run(&mut compile)?;
    observed.extend(super::super::dependencies::read(
        &depfile,
        &source,
        &compiled.stderr,
    )?);
    let mut objects = vec![object];
    if let Some(cxx) = &profile.data.cxx {
        let source = directory.path().join("cpp.cpp");
        let object = directory.path().join("cpp.obj");
        std::fs::write(&source, CPP_SOURCE).map_err(error)?;
        let mut compile = cxx.command(&profile.startup);
        if cxx.kind().microsoft() {
            compile
                .args(["/c", "/Z7"])
                .arg(&source)
                .arg(format!("/Fo{}", object.display()));
        } else {
            compile
                .args(["-c", "-g", "-gcodeview"])
                .arg(&source)
                .arg("-o")
                .arg(&object);
        }
        run(&mut compile)?;
        objects.push(object);
    }
    let output = directory.path().join("probe.exe");
    let mut command = profile.command(directory.path(), &output)?;
    command.args(objects);
    if profile.cxx() {
        profile.linker_arg(
            &mut command,
            if profile.microsoft() {
                "/include:scoop_cpp_probe"
            } else {
                "--undefined=scoop_cpp_probe"
            },
        );
    }
    profile.append_system_libraries(&mut command);
    profile.trace(&mut command);
    let result = run(&mut command)?;
    let mut inputs: BTreeSet<_> = profile
        .traced_inputs(&result, directory.path())
        .into_iter()
        .collect();
    inputs.insert(profile.data.unwind_prefix.join("lib/libunwind.a"));
    let bytes = std::fs::read(&output).map_err(error)?;
    check_image(&bytes, profile.startup.target().id())?;
    inputs
        .into_iter()
        .map(|path| {
            Ok(SystemInput {
                digest: sha256(&std::fs::read(&path).map_err(error)?),
                path,
            })
        })
        .collect()
}

pub(super) fn check_image(
    bytes: &[u8],
    target: scoop_lir::TargetProfileId,
) -> Result<(), ToolchainError> {
    let file = object::File::parse(bytes).map_err(error)?;
    if file.format() != object::BinaryFormat::Pe
        || file.architecture() != object::Architecture::X86_64
        || file.kind() != object::ObjectKind::Executable
        || !file.is_64()
    {
        return Err(error(
            "Windows link probe did not produce a PE32+ AMD64 executable",
        ));
    }
    if file
        .section_by_name(".pdata")
        .is_none_or(|section| section.size() == 0)
    {
        return Err(error("Windows link probe has no SEH function table"));
    }
    let imports = file.imports().map_err(error)?;
    let libraries: BTreeSet<_> = imports
        .iter()
        .map(|import| String::from_utf8_lossy(import.library()).to_ascii_lowercase())
        .collect();
    let legacy = libraries.contains("msvcrt.dll");
    let ucrt = libraries
        .iter()
        .any(|name| name == "ucrtbase.dll" || name.starts_with("api-ms-win-crt-"));
    if target == scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID {
        if !legacy || ucrt {
            return Err(error(format!(
                "MSVCRT target linked incompatible CRT imports: {libraries:?}"
            )));
        }
    } else if !ucrt || legacy {
        return Err(error(format!(
            "UCRT target linked incompatible CRT imports: {libraries:?}"
        )));
    }
    Ok(())
}

pub(super) fn run(command: &mut Command) -> Result<std::process::Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "Windows native tool {command:?} failed ({}): {}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

pub(super) const SOURCE: &str = r#"
#include <stdio.h>
#include <stdint.h>
#include <unwind.h>
static _Unwind_Reason_Code trace(struct _Unwind_Context *context, void *data) {
    (void)context; (*(int *)data)++; return _URC_NO_REASON;
}
int wmain(int argc, uint16_t **argv) {
    int count = 0;
    _Unwind_Backtrace(trace, &count);
    return count < 2 || argc < 1 || argv[0][0] == 0 || puts("PE CRT and LLVM unwind passed") < 0;
}
"#;

pub(super) const CPP_SOURCE: &str = r#"
#include <string>
#include <stdexcept>
struct Base { virtual ~Base() = default; };
struct Derived : Base { std::string value = "native C++"; };
extern "C" int scoop_cpp_probe(int n) {
    Derived derived;
    Base *base = &derived;
    try {
        if (n) throw std::runtime_error(derived.value);
        return dynamic_cast<Derived *>(base)->value.size();
    } catch (const std::exception &error) { return std::string(error.what()).size(); }
}
"#;
