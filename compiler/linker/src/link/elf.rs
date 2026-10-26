use super::*;
use scoop_toolchain::LinuxFinalLinkProfile;

mod shared;

#[cfg(all(test, target_os = "linux"))]
mod tests;

fn append_objects(
    command: &mut std::process::Command,
    directory: &Path,
    paths: &[PathBuf],
) -> Result<(), LinkError> {
    response::write_objects(directory, paths)?;
    // Keep @ away from the driver response-file parser. The relative name
    // also avoids -Wl comma splitting when the scratch path contains commas.
    command
        .current_dir(directory)
        .arg(format!("-Wl,@{}", response::FILE_NAME));
    Ok(())
}

pub(super) fn link(
    profile: &LinuxFinalLinkProfile,
    inputs: &ProgramInputs<'_>,
    directory: &Path,
    paths: &[PathBuf],
    candidate: &Path,
    keep_symbols: bool,
    symbol_map: Option<&Path>,
) -> Result<(), LinkError> {
    let mut command = profile.command(directory, candidate).map_err(error)?;
    if !keep_symbols {
        command.arg("-Xlinker").arg("--strip-all");
    }
    if let Some(symbol_map) = symbol_map {
        command
            .arg("-Xlinker")
            .arg("-Map")
            .arg("-Xlinker")
            .arg(symbol_map);
    }
    append_objects(&mut command, directory, paths)?;
    command.arg("-Xlinker").arg(format!(
        "--defsym={}={}",
        inputs.symbol("scoop_td_String"),
        inputs.string_target
    ));
    shared::append(inputs, directory, &mut command)?;
    profile.append_system_libraries(&mut command);
    let result = command
        .scoop_output()
        .map_err(|err| error(format!("cannot start ELF linker: {err}")))?;
    if !result.status.success() {
        return Err(error(format!(
            "ELF linker failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    Ok(())
}
