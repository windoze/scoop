use super::*;
use scoop_toolchain::{DebugFileKind, NativeDebugTools};
use std::process::Command;

#[cfg(all(test, unix))]
mod tests;

#[derive(Clone, Debug)]
pub struct DebugFile {
    pub kind: DebugFileKind,
    pub path: PathBuf,
}

pub(super) fn generate(
    tools: &NativeDebugTools,
    executable: &Path,
    keep_symbols: bool,
) -> Result<DebugFile, LinkError> {
    let kind = tools.kind();
    let path = kind.output_path(executable);
    match tools {
        NativeDebugTools::Windows => {
            // The PE linker produces the PDB in the same action as the executable.
            return Ok(DebugFile { kind, path });
        }
        NativeDebugTools::Darwin {
            dsymutil,
            strip,
            codesign,
            ..
        } => {
            run(Command::new(dsymutil).arg(executable).arg("-o").arg(&path))?;
            let mut command = Command::new(strip);
            command.arg("-S");
            if !keep_symbols {
                command.arg("-x");
            }
            run(command.arg(executable))?;
            run(Command::new(codesign)
                .args(["--force", "--sign", "-"])
                .arg(executable))?;
        }
        NativeDebugTools::Elf { objcopy, .. } => {
            run(Command::new(objcopy)
                .arg("--only-keep-debug")
                .arg(executable)
                .arg(&path))?;
            run(Command::new(objcopy)
                .arg(if keep_symbols {
                    "--strip-debug"
                } else {
                    "--strip-all"
                })
                .arg(executable))?;
            let mut debuglink = std::ffi::OsString::from("--add-gnu-debuglink=");
            debuglink.push(&path);
            run(Command::new(objcopy).arg(debuglink).arg(executable))?;
        }
    }
    if !path.try_exists().map_err(error)? {
        return Err(error(format!(
            "debug tool did not produce {}",
            path.display()
        )));
    }
    Ok(DebugFile { kind, path })
}

fn run(command: &mut Command) -> Result<(), LinkError> {
    let result = command.scoop_output().map_err(|source| {
        error(format!(
            "cannot start debug tool {}: {source}",
            Path::new(command.get_program()).display()
        ))
    })?;
    if !result.status.success() {
        return Err(error(format!(
            "debug tool {} failed: {}",
            Path::new(command.get_program()).display(),
            String::from_utf8_lossy(&result.stderr).trim()
        )));
    }
    Ok(())
}

/// Replace a completed dSYM, retaining the previous bundle if rename fails.
pub fn replace_debug_bundle(candidate: &Path, output: &Path) -> std::io::Result<()> {
    let parent = output.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "debug bundle has no parent",
        )
    })?;
    let previous = tempfile::Builder::new()
        .prefix(".scoop-dsym-")
        .tempdir_in(parent)?;
    let backup = previous.path().join("previous");
    let had_previous = match std::fs::rename(output, &backup) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if let Err(error) = std::fs::rename(candidate, output) {
        if had_previous {
            std::fs::rename(&backup, output)?;
        }
        return Err(error);
    }
    Ok(())
}
