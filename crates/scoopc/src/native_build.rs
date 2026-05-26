//! Cone-local native C/C++ object compilation for `build-single-cone`.
//!
//! The facade driver must not compile cone sources itself. This module compiles
//! only the cone currently being built and returns object payloads that become
//! part of that cone's artifact manifest.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use miette::Diagnostic;
use scoop_project_model::SourceConeNode;
use thiserror::Error;

use crate::cone::ConeArtifactObject;

#[derive(Debug)]
pub struct NativeBuildObjects {
    pub objects: Vec<ConeArtifactObject>,
    pub use_cxx_linker_driver: bool,
}

#[derive(Debug, Error, Diagnostic)]
pub enum CompileCError {
    #[error("找不到 C 编译器 `{compiler}`（需要安装并确保在 PATH 中）")]
    #[diagnostic(code(scoopc::native_build::c_compiler_not_found))]
    CompilerNotFound { compiler: String },
    #[error("无法读取 C 源文件：{path}")]
    #[diagnostic(code(scoopc::native_build::c_source_unreadable))]
    SourceUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("找不到 C 源文件：{path}")]
    #[diagnostic(code(scoopc::native_build::c_source_missing))]
    SourceMissing { path: PathBuf },
    #[error("运行 C 编译器 `{compiler}` 失败：{source}")]
    #[diagnostic(code(scoopc::native_build::c_compile_spawn_failed))]
    CompileSpawnFailed {
        compiler: String,
        #[source]
        source: std::io::Error,
    },
    #[error("编译失败（退出码：{status}）\n命令：{command}\nstdout：{stdout}\nstderr：{stderr}")]
    #[diagnostic(code(scoopc::native_build::c_compile_failed))]
    CompileFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },
}

#[derive(Debug, Error, Diagnostic)]
pub enum CompileCxxError {
    #[error("找不到 C++ 编译器 `{compiler}`（需要安装并确保在 PATH 中）")]
    #[diagnostic(code(scoopc::native_build::cxx_compiler_not_found))]
    CompilerNotFound { compiler: String },
    #[error("无法读取 C++ 源文件：{path}")]
    #[diagnostic(code(scoopc::native_build::cxx_source_unreadable))]
    SourceUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("找不到 C++ 源文件：{path}")]
    #[diagnostic(code(scoopc::native_build::cxx_source_missing))]
    SourceMissing { path: PathBuf },
    #[error("运行 C++ 编译器 `{compiler}` 失败：{source}")]
    #[diagnostic(code(scoopc::native_build::cxx_compile_spawn_failed))]
    CompileSpawnFailed {
        compiler: String,
        #[source]
        source: std::io::Error,
    },
    #[error("编译失败（退出码：{status}）\n命令：{command}\nstdout：{stdout}\nstderr：{stderr}")]
    #[diagnostic(code(scoopc::native_build::cxx_compile_failed))]
    CompileFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },
}

pub fn compile_native_build_objects(
    node: &SourceConeNode,
    objs_dir: &Path,
) -> miette::Result<NativeBuildObjects> {
    std::fs::create_dir_all(objs_dir).map_err(miette::Report::from_err)?;

    let native_build = &node.native_build;
    let prefix = native_obj_prefix(node);
    let mut objects =
        Vec::with_capacity(native_build.c_sources.len() + native_build.cxx_sources.len());
    for (idx, rel) in native_build.c_sources.iter().enumerate() {
        let file_name = format!("{prefix}_c_{idx}.o");
        let src = node.root.join(rel);
        let out_obj = objs_dir.join(&file_name);
        compile_c_source_to_obj(&node.root, &src, &out_obj, &native_build.c_flags)?;
        let bytes = std::fs::read(&out_obj).map_err(miette::Report::from_err)?;
        objects.push(ConeArtifactObject::new(file_name, bytes).map_err(miette::Report::from_err)?);
    }

    for (idx, rel) in native_build.cxx_sources.iter().enumerate() {
        let file_name = format!("{prefix}_cxx_{idx}.o");
        let src = node.root.join(rel);
        let out_obj = objs_dir.join(&file_name);
        compile_cxx_source_to_obj(&node.root, &src, &out_obj, &native_build.cxx_flags)?;
        let bytes = std::fs::read(out_obj).map_err(miette::Report::from_err)?;
        objects.push(ConeArtifactObject::new(file_name, bytes).map_err(miette::Report::from_err)?);
    }

    Ok(NativeBuildObjects {
        objects,
        use_cxx_linker_driver: !native_build.cxx_sources.is_empty(),
    })
}

fn native_obj_prefix(node: &SourceConeNode) -> String {
    let mut name = String::with_capacity(node.manifest.cone.name.len());
    for ch in node.manifest.cone.name.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    if name.is_empty() {
        name.push_str("anonymous");
    }
    format!("native_cone{}_{}", node.id.as_u32(), name)
}

pub fn compile_c_source_to_obj(
    cone_root: &Path,
    source: &Path,
    output_obj: &Path,
    c_flags: &[String],
) -> Result<(), CompileCError> {
    let meta = std::fs::metadata(source).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CompileCError::SourceMissing {
            path: source.to_path_buf(),
        },
        _ => CompileCError::SourceUnreadable {
            path: source.to_path_buf(),
            source: e,
        },
    })?;
    if !meta.is_file() {
        return Err(CompileCError::SourceMissing {
            path: source.to_path_buf(),
        });
    }

    let mut cmd = compile_c_command_to_obj(cone_root, source, output_obj, c_flags);
    let compiler_for_error = cmd.get_program().to_string_lossy().to_string();
    let output_res = cmd.output();
    let output_res = match output_res {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CompileCError::CompilerNotFound {
                compiler: compiler_for_error,
            });
        }
        Err(e) => {
            return Err(CompileCError::CompileSpawnFailed {
                compiler: compiler_for_error,
                source: e,
            });
        }
    };

    if !output_res.status.success() {
        return Err(CompileCError::CompileFailed {
            status: output_res.status,
            command: format_command_for_debug(&cmd),
            stdout: String::from_utf8_lossy(&output_res.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output_res.stderr).to_string(),
        });
    }
    Ok(())
}

fn compile_c_command_to_obj(
    cone_root: &Path,
    source: &Path,
    output_obj: &Path,
    c_flags: &[String],
) -> Command {
    let mut cmd = Command::new("clang");
    cmd.current_dir(cone_root);
    cmd.arg("-c");
    cmd.arg("-I").arg(scoopld::runtime_public_include_dir());
    for flag in c_flags {
        if !flag.trim().is_empty() {
            cmd.arg(flag);
        }
    }
    push_build_profile_and_target_defines(&mut cmd);
    cmd.arg(source);
    cmd.arg("-o").arg(output_obj);
    cmd
}

pub fn compile_cxx_source_to_obj(
    cone_root: &Path,
    source: &Path,
    output_obj: &Path,
    cxx_flags: &[String],
) -> Result<(), CompileCxxError> {
    let meta = std::fs::metadata(source).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CompileCxxError::SourceMissing {
            path: source.to_path_buf(),
        },
        _ => CompileCxxError::SourceUnreadable {
            path: source.to_path_buf(),
            source: e,
        },
    })?;
    if !meta.is_file() {
        return Err(CompileCxxError::SourceMissing {
            path: source.to_path_buf(),
        });
    }

    let mut cmd = compile_cxx_command_to_obj(cone_root, source, output_obj, cxx_flags);
    let compiler_for_error = cmd.get_program().to_string_lossy().to_string();
    let output_res = cmd.output();
    let output_res = match output_res {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CompileCxxError::CompilerNotFound {
                compiler: compiler_for_error,
            });
        }
        Err(e) => {
            return Err(CompileCxxError::CompileSpawnFailed {
                compiler: compiler_for_error,
                source: e,
            });
        }
    };

    if !output_res.status.success() {
        return Err(CompileCxxError::CompileFailed {
            status: output_res.status,
            command: format_command_for_debug(&cmd),
            stdout: String::from_utf8_lossy(&output_res.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output_res.stderr).to_string(),
        });
    }
    Ok(())
}

fn compile_cxx_command_to_obj(
    cone_root: &Path,
    source: &Path,
    output_obj: &Path,
    cxx_flags: &[String],
) -> Command {
    let mut cmd = Command::new("clang++");
    cmd.current_dir(cone_root);
    cmd.arg("-c");
    cmd.arg("-I").arg(scoopld::runtime_public_include_dir());
    for flag in cxx_flags {
        if !flag.trim().is_empty() {
            cmd.arg(flag);
        }
    }
    push_build_profile_and_target_defines(&mut cmd);
    cmd.arg(source);
    cmd.arg("-o").arg(output_obj);
    cmd
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostTargetInfo {
    build_profile: &'static str,
    is_debug: bool,
    triple: String,
    arch: String,
    os: String,
    env: String,
    vendor: String,
    pointer_width: u32,
    endianness: &'static str,
}

fn push_build_profile_and_target_defines(cmd: &mut Command) {
    let info = host_target_info();
    cmd.arg(c_define_string("SCOOP_BUILD_PROFILE", info.build_profile));
    if info.is_debug {
        cmd.arg("-DSCOOP_DEBUG");
    }
    cmd.arg(c_define_string("SCOOP_TARGET_TRIPLE", &info.triple));
    cmd.arg(c_define_string("SCOOP_TARGET_ARCH", &info.arch));
    cmd.arg(c_define_string("SCOOP_TARGET_OS", &info.os));
    cmd.arg(c_define_string("SCOOP_TARGET_ENV", &info.env));
    cmd.arg(c_define_string("SCOOP_TARGET_VENDOR", &info.vendor));
    cmd.arg(c_define_u32(
        "SCOOP_TARGET_POINTER_WIDTH",
        info.pointer_width,
    ));
    cmd.arg(c_define_string("SCOOP_TARGET_ENDIANNESS", info.endianness));
}

fn host_target_info() -> HostTargetInfo {
    let build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let is_debug = build_profile == "debug";
    let arch = option_env!("CARGO_CFG_TARGET_ARCH")
        .unwrap_or(std::env::consts::ARCH)
        .to_string();
    let os_cfg = option_env!("CARGO_CFG_TARGET_OS").unwrap_or(std::env::consts::OS);
    let os = normalize_target_os_for_llvm_triple(os_cfg).to_string();
    let vendor = option_env!("CARGO_CFG_TARGET_VENDOR")
        .unwrap_or(match os.as_str() {
            "darwin" => "apple",
            "windows" => "pc",
            _ => "unknown",
        })
        .to_string();
    let env = option_env!("CARGO_CFG_TARGET_ENV")
        .unwrap_or("")
        .to_string();
    let triple = if env.is_empty() {
        format!("{arch}-{vendor}-{os}")
    } else {
        format!("{arch}-{vendor}-{os}-{env}")
    };
    let endianness = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };

    HostTargetInfo {
        build_profile,
        is_debug,
        triple,
        arch,
        os,
        env,
        vendor,
        pointer_width: usize::BITS,
        endianness,
    }
}

fn normalize_target_os_for_llvm_triple(os_cfg: &str) -> &str {
    match os_cfg {
        "macos" => "darwin",
        other => other,
    }
}

fn c_define_string(name: &str, value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("-D{name}=\"{escaped}\"")
}

fn c_define_u32(name: &str, value: u32) -> String {
    format!("-D{name}={value}")
}

fn format_command_for_debug(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy();
    let args = cmd
        .get_args()
        .map(|arg| shell_escape(&arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {args}")
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b':' | b'='))
    {
        value.to_string()
    } else {
        format!("'{escaped}'", escaped = value.replace('\'', "'\\''"))
    }
}
