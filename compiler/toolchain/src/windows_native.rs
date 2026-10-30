//! Native C/C++ compilers in the selected Windows SDK/CRT environment.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use object::{Object, ObjectSection, ObjectSymbol};
use scoop_manifest::NativeSourceLanguage;
use scoop_process::CommandExt;

use crate::build_record::{Input, Record};
use crate::native_driver::{NativeDriverDiscovery, NativeDriverKind};
use crate::{ToolchainError, ValidatedCBridgeToolchainInvocation};

mod probe;

pub(crate) fn is_windows(c: &ValidatedCBridgeToolchainInvocation) -> bool {
    c.native_environment().is_windows()
}

pub(crate) fn paired_driver(
    c: &ValidatedCBridgeToolchainInvocation,
    language: NativeSourceLanguage,
    selected_c: Option<&Path>,
) -> PathBuf {
    if let Some(driver) = selected_c {
        if language == NativeSourceLanguage::C
            || (c.target().id() == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
                && kind(driver).microsoft())
        {
            return driver.to_owned();
        }
        return driver.with_file_name(crate::host_executable_name("clang++"));
    }
    let name = if c.target().id() == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID {
        "clang-cl"
    } else if language == NativeSourceLanguage::Cxx {
        "clang++"
    } else {
        "clang"
    };
    c.compiler_driver()
        .with_file_name(crate::host_executable_name(name))
}

pub(crate) fn kind(driver: &Path) -> NativeDriverKind {
    let name = driver
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name.eq_ignore_ascii_case("cl") {
        NativeDriverKind::Msvc
    } else if name.eq_ignore_ascii_case("clang-cl") {
        NativeDriverKind::ClangCl
    } else {
        NativeDriverKind::Gnu
    }
}

pub(crate) fn input(command: &mut Command, kind: NativeDriverKind, path: &Path) {
    let path = dunce::simplified(path);
    // Use Clang's GNU argument parser for filenames so /Users is not /U.
    if kind == NativeDriverKind::ClangCl {
        command.arg(joined("/clang:", path));
    } else {
        command.arg(path);
    }
}

pub(crate) fn command(
    c: &ValidatedCBridgeToolchainInvocation,
    driver: &Path,
    kind: NativeDriverKind,
    language: NativeSourceLanguage,
    preprocessed: bool,
) -> Command {
    let environment = c
        .windows_environment()
        .expect("selected Windows native environment");
    if !kind.microsoft() {
        let mut command = environment.bare_command(driver);
        environment.configure(
            &mut command,
            crate::windows_c::machine_triple(c.target().id()).expect("validated Windows target"),
            c.target().id() == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
            preprocessed,
        );
        if !preprocessed
            && language == NativeSourceLanguage::Cxx
            && c.target().id() != scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
        {
            command.arg("-stdlib=libc++");
        }
        return command;
    }
    let mut command = environment.bare_command(driver);
    command.args([
        "/nologo",
        "/X",
        "/MD",
        "/utf-8",
        "/D_WIN32_WINNT=0x0a00",
        "/DWINVER=0x0a00",
    ]);
    if kind == NativeDriverKind::ClangCl {
        command.args(["--target=x86_64-pc-windows-msvc", "/clang:-march=x86-64"]);
    } else {
        command.arg("/Zc:preprocessor");
    }
    if language == NativeSourceLanguage::Cxx {
        command.arg("/EHsc");
    }
    if !preprocessed {
        for include in &environment.includes {
            command
                .arg(if kind == NativeDriverKind::ClangCl {
                    "/imsvc"
                } else {
                    "/I"
                })
                .arg(include);
        }
    }
    command
}

pub(crate) fn discover(
    c: &ValidatedCBridgeToolchainInvocation,
    language: NativeSourceLanguage,
    driver: &Path,
) -> Result<Record<NativeDriverDiscovery>, ToolchainError> {
    let environment = c.windows_environment().map_err(error)?;
    let mut tool_inputs = BTreeSet::new();
    crate::native_record::program_candidates(
        driver.as_os_str(),
        &environment.search_path,
        &mut tool_inputs,
    );
    let driver = crate::paths::find_program(driver.as_os_str(), &environment.search_path)?;
    let kind = kind(&driver);
    if kind.microsoft() && c.target().id() != scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID {
        return Err(error("cl/clang-cl require the Microsoft UCRT target"));
    }
    let directory = tempfile::Builder::new()
        .prefix("scoop-native-probe-")
        .tempdir()
        .map_err(error)?;
    let source = directory
        .path()
        .join(if language == NativeSourceLanguage::Cxx {
            "probe.cpp"
        } else {
            "probe.c"
        });
    let object = directory.path().join("probe.obj");
    let depfile = directory.path().join("dependencies");
    std::fs::write(&source, probe::source(c.target().id(), language)).map_err(error)?;
    let mut compiler = command(c, &driver, kind, language, false);
    compiler.arg(if kind.microsoft() {
        if language == NativeSourceLanguage::Cxx {
            "/std:c++20"
        } else {
            "/std:c11"
        }
    } else if language == NativeSourceLanguage::Cxx {
        "-std=c++20"
    } else {
        "-std=c11"
    });
    dependencies(&mut compiler, kind, &depfile, "native-probe");
    if kind.microsoft() {
        compiler.arg("/c").arg(joined("/Fo", &object));
    } else {
        compiler.arg("-c").arg("-o").arg(&object);
    }
    input(&mut compiler, kind, &source);
    let output = compiler.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let real_source = source.canonicalize().map_err(error)?;
    let mut inputs: BTreeSet<_> = read_dependencies(kind, &depfile, "native-probe")?
        .into_iter()
        .filter(|p| p != &source && p.canonicalize().ok().as_ref() != Some(&real_source))
        .collect();
    inputs.insert(driver.clone());
    inputs.extend(tool_inputs);
    if kind == NativeDriverKind::Msvc {
        let frontend = if language == NativeSourceLanguage::Cxx {
            "c1xx.dll"
        } else {
            "c1.dll"
        };
        inputs.extend([frontend, "c2.dll", "msobj140.dll"].map(|name| driver.with_file_name(name)));
    }
    inputs.extend(environment.includes.iter().cloned());
    for path in inputs.clone() {
        if let Some(parent) = path.parent() {
            inputs.insert(parent.to_owned());
        }
    }
    let bytes = std::fs::read(object).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    if file.architecture() != object::Architecture::X86_64
        || file.format() != object::BinaryFormat::Coff
        || file.kind() != object::ObjectKind::Relocatable
    {
        return Err(error("native compiler must emit AMD64 COFF objects"));
    }
    let symbol = file
        .symbol_by_name("scoop_native_configuration")
        .ok_or_else(|| error("missing native probe configuration"))?;
    let section = file
        .section_by_index(
            symbol
                .section_index()
                .ok_or_else(|| error("missing probe section"))?,
        )
        .map_err(error)?;
    let data = section
        .data_range(symbol.address(), 16)
        .map_err(error)?
        .ok_or_else(|| error("truncated native probe configuration"))?;
    Ok(Record {
        inputs: inputs
            .iter()
            .map(|p| Input::read(p))
            .collect::<Result<_, _>>()?,
        result: NativeDriverDiscovery {
            driver: scoop_protocol::HostPathCarrier::from_path(&driver).map_err(error)?,
            configuration: *scoop_wire::sha256(data).as_array(),
            kind,
            family: if kind == NativeDriverKind::Msvc {
                crate::native_driver::NativeCompilerFamily::Msvc
            } else {
                crate::native_driver::NativeCompilerFamily::Clang
            },
        },
    })
}

pub(crate) fn dependencies(
    command: &mut Command,
    kind: NativeDriverKind,
    path: &Path,
    target: &str,
) {
    match kind {
        NativeDriverKind::Msvc => {
            command.arg("/sourceDependencies").arg(path);
        }
        NativeDriverKind::ClangCl => {
            command
                .args(["/clang:-MD", "/clang:-MT"])
                .arg(format!("/clang:{target}"))
                .arg("/clang:-MF")
                .arg(joined("/clang:", path));
        }
        NativeDriverKind::Gnu => {
            command.args(["-MD", "-MT", target, "-MF"]).arg(path);
        }
    }
}

pub(crate) fn read_dependencies(
    kind: NativeDriverKind,
    path: &Path,
    target: &str,
) -> Result<Vec<PathBuf>, ToolchainError> {
    let text = std::fs::read_to_string(path).map_err(error)?;
    if kind == NativeDriverKind::Msvc {
        #[derive(serde::Deserialize)]
        struct Dependencies {
            #[serde(rename = "Data")]
            data: Source,
        }
        #[derive(serde::Deserialize)]
        struct Source {
            #[serde(rename = "Source")]
            source: PathBuf,
            #[serde(rename = "Includes")]
            includes: Vec<PathBuf>,
        }
        let dependencies: Dependencies =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(error)?;
        Ok(std::iter::once(dependencies.data.source)
            .chain(dependencies.data.includes)
            .collect())
    } else {
        scoop_process::parse_make_dependencies(&text, target)
            .ok_or_else(|| error("invalid native compiler dependency output"))
    }
}

pub(crate) fn joined(prefix: &str, path: &Path) -> std::ffi::OsString {
    let mut value = std::ffi::OsString::from(prefix);
    value.push(path);
    value
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("Windows native compiler: {value}"))
}
