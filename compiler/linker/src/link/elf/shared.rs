use super::*;

pub(super) fn append(
    inputs: &ProgramInputs<'_>,
    directory: &Path,
    command: &mut std::process::Command,
) -> Result<Vec<PathBuf>, LinkError> {
    let crate::namespace::NativeNamespace::Elf(namespace) = &inputs.namespace else {
        return Err(error("ELF link requires an ELF namespace"));
    };
    let scratch = directory.join("shared");
    std::fs::create_dir(&scratch).map_err(error)?;
    let mut paths = Vec::new();
    command.arg("-Wl,--push-state,--no-as-needed");
    for id in namespace.selected_order() {
        let file = &inputs.native.files[&id];
        let provider = &namespace.providers[&id];
        if provider.interface.soname.is_some() {
            let path = scratch.join(format!("{id}.so"));
            write_new(&path, &file.bytes[file.slice.clone()])?;
            command.arg(&path);
            paths.push(path);
        } else {
            // -l:filename retains a normal DT_NEEDED name for SONAME-less DSOs.
            let path = scratch.join(&provider.name);
            write_new(&path, &file.bytes[file.slice.clone()])?;
            command
                .arg("-L")
                .arg(&scratch)
                .arg(format!("-l:{}", provider.name));
            paths.push(path);
        }
    }
    command.arg("-Wl,--pop-state");
    for path in &namespace.rpaths {
        command.args(["-Xlinker", "-rpath", "-Xlinker"]).arg(path);
    }
    Ok(paths)
}
