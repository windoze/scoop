use std::path::Path;
use std::process::{Command, Stdio};

use scoop_lir::OptimizationMode;
use scoop_manifest::{NativeCompileFlag, NativeConfig, NativeSourceLanguage};
use scoop_process::CommandExt;

use super::{NativeSourceInput, NativeToolchain, ToolchainError, error};

pub(super) fn base(
    compiler: &NativeToolchain,
    language: NativeSourceLanguage,
    optimization: OptimizationMode,
) -> Result<Command, ToolchainError> {
    let mut command = compiler.command(language)?;
    command.env("SOURCE_DATE_EPOCH", "0").args([
        match language {
            NativeSourceLanguage::C => "-std=c11",
            NativeSourceLanguage::Cxx => "-std=c++20",
        },
        "-fPIC",
        match optimization {
            OptimizationMode::Debug => "-O0",
            OptimizationMode::Release => "-O2",
        },
    ]);
    Ok(command)
}

pub(super) fn configure_preprocessor(
    command: &mut Command,
    config: &NativeConfig,
    language: NativeSourceLanguage,
    root: &Path,
) {
    for include in config.include() {
        command.arg("-I").arg(root.join(include.as_path()));
    }
    for flag in config.flags(language) {
        match flag {
            NativeCompileFlag::Argument(argument) => {
                command.arg(argument);
            }
            NativeCompileFlag::Include { kind, path } => {
                command.arg(kind.argument()).arg(root.join(path.as_path()));
            }
        }
    }
}

pub(super) fn run(
    command: &mut Command,
    source: &str,
) -> Result<std::process::Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output)
}

pub(super) fn system_roots(
    compiler: &NativeToolchain,
    language: NativeSourceLanguage,
) -> Result<Vec<std::path::PathBuf>, ToolchainError> {
    let output = run(
        compiler
            .command(language)?
            .args(["-E", "-v", "-x", language.input_name(), "-"])
            .stdin(Stdio::null()),
        "system include search",
    )?;
    let mut roots = Vec::new();
    let mut reading = false;
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        if line.contains("#include <...> search starts here:") {
            reading = true;
        } else if line.contains("End of search list.") {
            break;
        } else if reading {
            let path = line.trim().trim_end_matches(" (framework directory)");
            roots.push(std::fs::canonicalize(path).map_err(error)?);
        }
    }
    if roots.is_empty() {
        return Err(error(
            "selected C compiler did not report its system include directories",
        ));
    }
    // Apple Clang records SDKSettings.json alongside included headers.
    if let Ok(sdk) = compiler.c().sdk_root() {
        roots.push(sdk.canonicalize().map_err(error)?);
    }
    Ok(roots)
}

pub(super) fn compile(
    input: &NativeSourceInput,
    config: &NativeConfig,
    compiler: &NativeToolchain,
    optimization: OptimizationMode,
    output: &Path,
) -> Result<(), ToolchainError> {
    let directory = tempfile::Builder::new()
        .prefix("scoop-native-")
        .tempdir()
        .map_err(error)?;
    // Debug compilation directories use the physical path, including on macOS.
    let directory_path = directory.path().canonicalize().map_err(error)?;
    let language = input.language();
    let source = directory_path.join(match language {
        NativeSourceLanguage::C => "unit.i",
        NativeSourceLanguage::Cxx => "unit.ii",
    });
    std::fs::write(&source, input.preprocessed()).map_err(error)?;
    let mut command = base(compiler, language, optimization)?;
    for flag in config.flags(language) {
        if let NativeCompileFlag::Argument(argument) = flag
            && !argument.starts_with("-D")
            && !argument.starts_with("-U")
        {
            command.arg(argument);
        }
    }
    command
        .current_dir(&directory_path)
        .arg("-fPIC")
        .arg(format!(
            "-ffile-prefix-map={}=/scoop-native",
            directory_path.display()
        ))
        .args(["-x", language.preprocessed_name(), "-c"])
        .arg(&source)
        .arg("-o")
        .arg(output);
    run(&mut command, input.path().as_str())?;
    Ok(())
}
