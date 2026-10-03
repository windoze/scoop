use super::*;
use object::{Architecture, Object, ObjectKind, macho, read::macho::MachOFile64};
use scoop_process::CommandExt;

pub(super) fn check(profile: &ValidatedFinalLinkProfile) -> Result<(), ToolchainError> {
    for symbol in profile.linker_system_requirements() {
        if !profile.system.exports().contains_key(*symbol) {
            return Err(error(format!(
                "SDK is missing linker support symbol {symbol}"
            )));
        }
    }
    let directory = tempfile::Builder::new()
        .prefix("scoop-link-probe-")
        .tempdir()
        .map_err(error)?;
    let source = directory.path().join("main.c");
    let object = directory.path().join("main.o");
    std::fs::write(
        &source,
        b"#include <stdio.h>\nint main(void) { return puts(\"scoop\") < 0; }\n",
    )
    .map_err(error)?;
    let output = profile
        .startup
        .object_compilation_command(&source, &object)
        .scoop_output()
        .map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "startup C probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let sdk = directory.path().join("sdk");
    let stub = profile.system.write_to(&sdk)?;
    let executable = directory.path().join("program");
    let output = profile
        .command(&sdk, &executable, &directory.path().join("program.map"))
        .arg(&object)
        .arg(stub)
        .scoop_output()
        .map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "ld probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let bytes = std::fs::read(executable).map_err(error)?;
    let file: MachOFile64<'_> = MachOFile64::parse(bytes.as_slice()).map_err(error)?;
    if file.kind() != ObjectKind::Executable
        || file.architecture() != Architecture::Aarch64
        || !file.is_little_endian()
    {
        return Err(error("ld probe did not produce an arm64 executable"));
    }
    let mut commands = file.macho_load_commands().map_err(error)?;
    let mut classic_fixups = false;
    while let Some(command) = commands.next().map_err(error)? {
        if command.cmd() == macho::LC_DYLD_CHAINED_FIXUPS {
            return Err(error("ld ignored -no_fixup_chains"));
        }
        classic_fixups |= command.cmd() == macho::LC_DYLD_INFO_ONLY;
    }
    if !classic_fixups
        || file
            .imports()
            .map_err(error)?
            .iter()
            .any(|import| import.library() != crate::LIBSYSTEM_INSTALL_NAME.as_bytes())
    {
        return Err(error("ld probe has unexpected fixups or dynamic providers"));
    }
    Ok(())
}
