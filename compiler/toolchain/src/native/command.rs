use std::path::Path;
use std::process::{Command, Stdio};

use scoop_lir::{OptimizationMode, TargetProfileId};
use scoop_manifest::{NativeCompileFlag, NativeConfig, NativeSourceLanguage};
use scoop_process::CommandExt;

use super::{NativeSourceInput, NativeToolchain, ToolchainError, error};
use crate::native_driver::NativeDriverKind;

pub(super) fn base(
    compiler: &NativeToolchain,
    language: NativeSourceLanguage,
    optimization: OptimizationMode,
    preprocessed: bool,
) -> Result<Command, ToolchainError> {
    let mut command = compiler.command(language)?;
    let kind = compiler.kind(language);
    if preprocessed && crate::windows_native::is_windows(compiler.c()) {
        command = crate::windows_native::command(
            compiler.c(),
            Path::new(command.get_program()),
            kind,
            language,
            true,
        );
    }
    if kind.microsoft() {
        command.arg(match language {
            NativeSourceLanguage::C => "/std:c11",
            NativeSourceLanguage::Cxx => "/std:c++20",
            NativeSourceLanguage::ObjectiveC => {
                return Err(error("Objective-C sources require a Darwin target"));
            }
        });
        command.arg(match optimization {
            OptimizationMode::Debug => "/Od",
            OptimizationMode::Release => "/O2",
        });
        if optimization == OptimizationMode::Debug {
            command.arg("/Z7");
        }
        return Ok(command);
    }
    command.env("SOURCE_DATE_EPOCH", "0").args([
        match language {
            NativeSourceLanguage::C | NativeSourceLanguage::ObjectiveC => "-std=c11",
            NativeSourceLanguage::Cxx => "-std=c++20",
        },
        match optimization {
            OptimizationMode::Debug => "-O0",
            OptimizationMode::Release => "-O2",
        },
    ]);
    if language == NativeSourceLanguage::ObjectiveC {
        command.arg("-fobjc-arc");
    }
    if crate::windows_native::is_windows(compiler.c()) {
        if optimization == OptimizationMode::Debug {
            command.arg("-gcodeview");
        }
    } else {
        command.arg("-fPIC");
    }
    Ok(command)
}

pub(super) fn configure_preprocessor(
    command: &mut Command,
    config: &NativeConfig,
    target: TargetProfileId,
    language: NativeSourceLanguage,
    root: &Path,
    target_root: &Path,
    kind: NativeDriverKind,
) -> Result<(), ToolchainError> {
    for include in config.include_paths(target, root, target_root) {
        command
            .arg(if kind.microsoft() { "/I" } else { "-I" })
            .arg(dunce::simplified(&include));
    }
    for flag in config.flags(language) {
        match flag {
            NativeCompileFlag::Argument(argument) => {
                command.arg(argument);
            }
            NativeCompileFlag::Include {
                kind: include,
                path,
            } => {
                let joined = root.join(path.as_path());
                let path = dunce::simplified(&joined);
                match (kind, include) {
                    (
                        NativeDriverKind::ClangCl | NativeDriverKind::Msvc,
                        scoop_manifest::NativeIncludeFlag::Search,
                    ) => {
                        command.arg("/I").arg(path);
                    }
                    (
                        NativeDriverKind::ClangCl | NativeDriverKind::Msvc,
                        scoop_manifest::NativeIncludeFlag::ForcedHeader,
                    ) => {
                        command.arg("/FI").arg(path);
                    }
                    (NativeDriverKind::ClangCl, _) => {
                        command
                            .arg(format!("/clang:{}", include.argument()))
                            .arg(crate::windows_native::joined("/clang:", path));
                    }
                    (NativeDriverKind::Msvc, _) => {
                        return Err(error(format!(
                            "cl does not support native include option {}",
                            include.argument()
                        )));
                    }
                    (NativeDriverKind::Gnu, _) => {
                        command.arg(include.argument()).arg(path);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn run(
    command: &mut Command,
    source: &str,
) -> Result<std::process::Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "{source}: {}",
            String::from_utf8_lossy(if output.stderr.is_empty() {
                &output.stdout
            } else {
                &output.stderr
            })
            .trim()
        )));
    }
    Ok(output)
}

pub(super) fn system_roots(
    compiler: &NativeToolchain,
    language: NativeSourceLanguage,
) -> Result<Vec<std::path::PathBuf>, ToolchainError> {
    if compiler.kind(language).microsoft() {
        return compiler
            .c()
            .windows_environment()
            .map_err(error)?
            .includes
            .iter()
            .map(|p| p.canonicalize().map_err(error))
            .collect();
    }
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
        NativeSourceLanguage::ObjectiveC => "unit.mi",
    });
    std::fs::write(&source, input.preprocessed()).map_err(error)?;
    let mut command = base(compiler, language, optimization, true)?;
    let kind = compiler.kind(language);
    for flag in config.flags(language) {
        if let NativeCompileFlag::Argument(argument) = flag
            && !argument.starts_with("-D")
            && !argument.starts_with("-U")
            && !argument.starts_with("/D")
            && !argument.starts_with("/U")
        {
            command.arg(argument);
        }
    }
    command.current_dir(dunce::simplified(&directory_path));
    remap(&mut command, kind, &directory_path, "/scoop-native");
    if kind.microsoft() {
        if kind == NativeDriverKind::Msvc {
            command.arg(match language {
                NativeSourceLanguage::C => "/TC",
                NativeSourceLanguage::Cxx => "/TP",
                NativeSourceLanguage::ObjectiveC => {
                    return Err(error("Objective-C sources require a Darwin target"));
                }
            });
        }
        // clang-cl recognizes .i/.ii as preprocessed input. /clang:-x would
        // be forwarded after all inputs and trigger an unused-option warning.
        command
            .arg("/c")
            .arg(crate::windows_native::joined("/Fo", output));
    } else {
        command
            .args(["-x", language.preprocessed_name(), "-c"])
            .arg("-o")
            .arg(output);
    }
    crate::windows_native::input(&mut command, kind, &source);
    run(&mut command, input.path().as_str())?;
    Ok(())
}

pub(super) fn remap(command: &mut Command, kind: NativeDriverKind, root: &Path, logical: &str) {
    let prefix = match kind {
        NativeDriverKind::Gnu => "-ffile-prefix-map=",
        NativeDriverKind::ClangCl => "/clang:-ffile-prefix-map=",
        NativeDriverKind::Msvc => "/pathmap:",
    };
    let mut argument = crate::windows_native::joined(prefix, dunce::simplified(root));
    argument.push("=");
    argument.push(logical);
    command.arg(argument);
}
