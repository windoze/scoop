use super::*;

pub(super) fn link(
    profile: &ValidatedFinalLinkProfile,
    inputs: &ProgramInputs<'_>,
    startup: &StartupObject,
    directory: &Path,
    paths: &[PathBuf],
    candidate: &Path,
    link_map: &Path,
) -> Result<(), LinkError> {
    let namespace = inputs.namespace.darwin()?;
    let sdk = directory.join("sdk");
    std::fs::create_dir(&sdk).map_err(error)?;
    let mut stubs = std::collections::BTreeMap::new();
    for (id, bytes) in &namespace.providers.stubs {
        let path = directory.join(format!("dynamic-{id}.tbd"));
        write_new(&path, bytes)?;
        stubs.insert(path, namespace.providers.providers[id].install_name.clone());
    }
    let ValidatedFinalLinkProfile::Darwin(darwin) = profile else {
        return Err(error("Darwin linking requires its selected target profile"));
    };
    let mut command = darwin.command(&sdk, candidate, link_map).map_err(error)?;
    response::write_objects(directory, paths)?;
    let mut argument = std::ffi::OsString::from("@");
    argument.push(directory.join(response::FILE_NAME));
    command.arg(argument);
    for argument in ["-alias", &inputs.string_target, "_scoop_td_String"] {
        darwin.linker_argument(&mut command, argument);
    }
    for path in &namespace.providers.rpaths {
        darwin.linker_argument(&mut command, "-rpath");
        darwin.linker_argument(&mut command, path);
    }
    command.args(stubs.keys());
    let result = command
        .scoop_output()
        .map_err(|err| error(format!("cannot start system linker: {err}")))?;
    if !result.status.success() {
        return Err(error(format!(
            "system linker failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    let object_origins = inputs
        .objects
        .iter()
        .zip(&paths[1..])
        .map(|(input, path)| (path.clone(), input.origin))
        .collect();
    let map = map::check(
        &std::fs::read_to_string(link_map).map_err(error)?,
        paths,
        &stubs,
        &object_origins,
    )?;
    map::trace(
        std::str::from_utf8(&result.stdout).map_err(error)?,
        paths,
        &stubs,
    )?;
    let bytes = std::fs::read(candidate).map_err(error)?;
    crate::final_image::verify(&bytes, inputs, startup, profile, &map)
        .map_err(|err| error(format!("final Mach-O validation: {err}")))?;
    Ok(())
}
