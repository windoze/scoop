use super::*;
use scoop_toolchain::WindowsFinalLinkProfile;

#[allow(clippy::too_many_arguments)]
pub(super) fn link(
    profile: &WindowsFinalLinkProfile,
    inputs: &ProgramInputs<'_>,
    directory: &Path,
    paths: &[PathBuf],
    candidate: &Path,
    pdb: Option<&Path>,
    keep_symbols: bool,
) -> Result<Vec<PathBuf>, LinkError> {
    let crate::namespace::NativeNamespace::Coff(namespace) = &inputs.namespace else {
        return Err(error("PE link requires a COFF namespace"));
    };
    let mut paths = paths.to_vec();
    let selected = namespace.selected();
    for file in inputs
        .native
        .ordered_files()
        .into_iter()
        .filter(|file| selected.contains(&file.id))
    {
        let id = file.id;
        let path = directory.join(format!("{id}.lib"));
        write_new(&path, &file.bytes[file.slice.clone()])?;
        paths.push(path);
    }
    response::write_pe_objects(directory, &paths, profile.microsoft())?;
    let mut command = profile
        .command_with_subsystem(directory, candidate, inputs.windows_subsystem)
        .map_err(error)?;
    profile.append_system_libraries(&mut command);
    profile.configure_outputs(&mut command, pdb, keep_symbols);
    if profile.microsoft() {
        profile.linker_arg(&mut command, format!("@{}", response::FILE_NAME));
    } else {
        // Keep the response file for LLD's host quoting rules. A standalone
        // @ argument is expanded earlier by Clang's GNU response parser.
        command.arg(format!("-Wl,@{}", response::FILE_NAME));
    }
    for argument in [
        format!(
            "/alternatename:{}={}",
            inputs.symbol("scoop_td_String"),
            inputs.string_target
        ),
        "/merge:.llvm_stackmaps=.smap".into(),
        "/section:.smap,R".into(),
    ] {
        profile.linker_arg(
            &mut command,
            if profile.microsoft() {
                argument
            } else {
                format!("--Xlink={argument}")
            },
        );
    }
    profile.trace(&mut command);
    let output = command
        .scoop_output()
        .map_err(|err| error(format!("cannot start PE linker: {err}")))?;
    if !output.status.success() {
        return Err(error(format!(
            "PE linker failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(profile.traced_inputs(&output, directory))
}
