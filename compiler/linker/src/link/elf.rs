use super::*;
use scoop_toolchain::LinuxFinalLinkProfile;
use std::collections::BTreeSet;

mod shared;

pub(super) fn link(
    profile: &LinuxFinalLinkProfile,
    inputs: &ProgramInputs<'_>,
    directory: &Path,
    paths: &[PathBuf],
    candidate: &Path,
    link_map: &Path,
) -> Result<(), LinkError> {
    let mut command = profile
        .command(directory, candidate, link_map)
        .map_err(error)?;
    command.args(paths).arg("-Xlinker").arg(format!(
        "--defsym={}={}",
        inputs.symbol("scoop_td_String"),
        inputs.string_target
    ));
    let shared = shared::append(inputs, directory, &mut command)?;
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
    let allowed: BTreeSet<_> = paths
        .iter()
        .chain(&shared)
        .map(PathBuf::as_path)
        .chain(profile.input_paths())
        .map(|path| std::fs::canonicalize(path).map_err(error))
        .collect::<Result<_, _>>()?;
    let mut seen = BTreeSet::new();
    for line in std::str::from_utf8(&result.stdout).map_err(error)?.lines() {
        let path = Path::new(line.trim());
        if path.is_absolute() {
            let path = std::fs::canonicalize(path).map_err(error)?;
            if !allowed.contains(&path) {
                return Err(error(format!(
                    "ELF linker selected unexpected input {}",
                    path.display()
                )));
            }
            seen.insert(path);
        }
    }
    for path in paths {
        if !seen.contains(&std::fs::canonicalize(path).map_err(error)?) {
            return Err(error(format!(
                "ELF linker omitted object {}",
                path.display()
            )));
        }
    }
    let map = std::fs::read_to_string(link_map).map_err(error)?;
    profile.check_unwind_map(&map).map_err(error)?;
    let bytes = std::fs::read(candidate).map_err(error)?;
    crate::final_image::elf::verify(&bytes, inputs, profile)
        .map_err(|err| error(format!("final ELF validation: {err}")))
}
