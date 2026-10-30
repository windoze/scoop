use super::*;

pub(super) fn paired_driver(
    c: &ValidatedCBridgeToolchainInvocation,
) -> Result<PathBuf, ToolchainError> {
    let path = c.compiler_driver();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let paired = match c.native_environment() {
        scoop_target_data::NativeEnvironment::Darwin => "clang++".to_owned(),
        scoop_target_data::NativeEnvironment::LinuxGnu
            if c.compiler_family() == NativeCompilerFamily::Clang =>
        {
            match name.rsplit_once("clang") {
                Some((prefix, version)) => format!("{prefix}clang++{version}"),
                None if name == "cc" => "c++".to_owned(),
                None => {
                    return Err(error(format!(
                        "cannot locate a paired Clang C++ driver for {}",
                        path.display()
                    )));
                }
            }
        }
        scoop_target_data::NativeEnvironment::LinuxGnu => match name.rsplit_once("gcc") {
            Some((prefix, version)) => format!("{prefix}g++{version}"),
            None if name == "cc" => "c++".to_owned(),
            None => {
                return Err(error(format!(
                    "cannot locate a paired C++ driver for {}",
                    path.display()
                )));
            }
        },
        scoop_target_data::NativeEnvironment::LinuxMusl => {
            return Err(error("C++ is not supported for Linux musl"));
        }
        _ => return Err(error("unsupported target for C++ driver selection")),
    };
    // Preserve clang++'s spelling: its argv[0] selects C++ link defaults.
    let driver = path.with_file_name(paired);
    if !driver.is_file() {
        return Err(error(format!(
            "missing paired C++ driver {}",
            driver.display()
        )));
    }
    Ok(driver)
}

pub(super) fn inputs(
    c: &ValidatedCBridgeToolchainInvocation,
    driver: &Path,
    family: NativeCompilerFamily,
    inputs: &mut std::collections::BTreeSet<PathBuf>,
) -> Result<(), ToolchainError> {
    if c.native_environment() == scoop_target_data::NativeEnvironment::Darwin {
        let sdk = c.sdk_root().map_err(error)?;
        inputs.extend([
            sdk.join("usr/lib/libc++.tbd"),
            sdk.join("usr/lib/libc++.1.tbd"),
        ]);
        let library = ["usr/lib/libc++.tbd", "usr/lib/libc++.1.tbd"]
            .into_iter()
            .map(|name| sdk.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| error("selected SDK has no libc++ interface"))?;
        inputs.insert(library);
        return Ok(());
    }
    let command = c.driver_command();
    let search = command
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value)
        .ok_or_else(|| error("selected GNU toolchain has no PATH"))?;
    if family == NativeCompilerFamily::Gcc {
        let frontend = text(
            c.driver_command_for(driver, family)
                .arg("-print-prog-name=cc1plus"),
        )?;
        crate::native_record::program_candidates(std::ffi::OsStr::new(&frontend), search, inputs);
        let frontend = crate::paths::find_program(std::ffi::OsStr::new(&frontend), search)?;
        inputs.insert(frontend);
    }
    let library = PathBuf::from(text(
        c.driver_command_for(driver, family)
            .arg("-print-file-name=libstdc++.so"),
    )?);
    if !library.is_absolute() || !library.is_file() {
        return Err(error(
            "selected GNU toolchain has no libstdc++ development library",
        ));
    }
    inputs.insert(library);
    Ok(())
}
