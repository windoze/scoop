use super::*;

pub(super) fn paired_driver(
    c: &ValidatedCBridgeToolchainInvocation,
) -> Result<PathBuf, ToolchainError> {
    let path = c.compiler_driver();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let paired = match c.profile().contract().target().id() {
        TargetProfileId::DarwinAarch64 => "clang++".to_owned(),
        TargetProfileId::LinuxX86_64Gnu => match name.rsplit_once("gcc") {
            Some((prefix, version)) => format!("{prefix}g++{version}"),
            None if name == "cc" => "c++".to_owned(),
            None => {
                return Err(error(format!(
                    "cannot locate a paired C++ driver for {}",
                    path.display()
                )));
            }
        },
        TargetProfileId::LinuxX86_64Musl => {
            return Err(error("C++ is not supported for Linux musl"));
        }
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
    inputs: &mut Vec<Digest256>,
) -> Result<(), ToolchainError> {
    if c.profile().contract().target().id() == TargetProfileId::DarwinAarch64 {
        let sdk = c.sdk_root().map_err(error)?;
        let library = ["usr/lib/libc++.tbd", "usr/lib/libc++.1.tbd"]
            .into_iter()
            .map(|name| sdk.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| error("selected SDK has no libc++ interface"))?;
        inputs.push(sha256(&std::fs::read(library).map_err(error)?));
        return Ok(());
    }
    for option in ["-dumpmachine", "-dumpfullversion"] {
        let expected = text(c.driver_command().arg(option))?;
        let actual = text(c.driver_command_with(driver).arg(option))?;
        if actual != expected {
            return Err(error(format!(
                "paired C++ driver {option} mismatch: {actual:?}, expected {expected:?}"
            )));
        }
    }
    let command = c.driver_command();
    let search = command
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value)
        .ok_or_else(|| error("selected GNU toolchain has no PATH"))?;
    let frontend = text(
        c.driver_command_with(driver)
            .arg("-print-prog-name=cc1plus"),
    )?;
    let frontend = crate::linux_c::find_program(std::ffi::OsStr::new(&frontend), search)?;
    let library = PathBuf::from(text(
        c.driver_command_with(driver)
            .arg("-print-file-name=libstdc++.so"),
    )?);
    if !library.is_absolute() || !library.is_file() {
        return Err(error(
            "selected GNU toolchain has no libstdc++ development library",
        ));
    }
    for path in [frontend, library] {
        inputs.push(sha256(&std::fs::read(path).map_err(error)?));
    }
    Ok(())
}
