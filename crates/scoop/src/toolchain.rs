//! Cone-local native source compilation helpers for the `scoop` driver.
//!
//! Final linking and runtime C object production live in the standalone
//! `scoopld` crate. This module intentionally keeps only the C/C++ object
//! helpers that are still consumed by the current in-process consumer build
//! path; P10-T06-c will move cone-local native build into `scoopc`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use miette::Diagnostic;
use thiserror::Error;

/// C 源码编译阶段错误（T1115：cone native build 的 `c-sources`）。
#[derive(Debug, Error, Diagnostic)]
pub enum CompileCError {
    #[error("找不到 C 编译器 `{compiler}`（需要安装并确保在 PATH 中）")]
    #[diagnostic(code(scoop::toolchain::c_compiler_not_found))]
    CompilerNotFound { compiler: String },

    #[error("无法读取 C 源文件：{path}")]
    #[diagnostic(code(scoop::toolchain::c_source_unreadable))]
    SourceUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("找不到 C 源文件：{path}")]
    #[diagnostic(code(scoop::toolchain::c_source_missing))]
    SourceMissing { path: PathBuf },

    #[error("运行 C 编译器 `{compiler}` 失败：{source}")]
    #[diagnostic(code(scoop::toolchain::c_compile_spawn_failed))]
    CompileSpawnFailed {
        compiler: String,
        #[source]
        source: std::io::Error,
    },

    #[error("编译失败（退出码：{status}）\n命令：{command}\nstdout：{stdout}\nstderr：{stderr}")]
    #[diagnostic(code(scoop::toolchain::c_compile_failed))]
    CompileFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },
}

/// C++ 源码编译阶段错误（T1116：cone native build 的 `cxx-sources`）。
#[derive(Debug, Error, Diagnostic)]
pub enum CompileCxxError {
    #[error("找不到 C++ 编译器 `{compiler}`（需要安装并确保在 PATH 中）")]
    #[diagnostic(code(scoop::toolchain::cxx_compiler_not_found))]
    CompilerNotFound { compiler: String },

    #[error("无法读取 C++ 源文件：{path}")]
    #[diagnostic(code(scoop::toolchain::cxx_source_unreadable))]
    SourceUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("找不到 C++ 源文件：{path}")]
    #[diagnostic(code(scoop::toolchain::cxx_source_missing))]
    SourceMissing { path: PathBuf },

    #[error("运行 C++ 编译器 `{compiler}` 失败：{source}")]
    #[diagnostic(code(scoop::toolchain::cxx_compile_spawn_failed))]
    CompileSpawnFailed {
        compiler: String,
        #[source]
        source: std::io::Error,
    },

    #[error("编译失败（退出码：{status}）\n命令：{command}\nstdout：{stdout}\nstderr：{stderr}")]
    #[diagnostic(code(scoop::toolchain::cxx_compile_failed))]
    CompileFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },
}

/// 将 cone 额外 `c-sources` 的单个 C 源文件编译为 object。
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

/// 将 cone 额外 `cxx-sources` 的单个 C++ 源文件编译为 object。
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
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>();
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_profile_and_target_define_args() -> Vec<String> {
        let build_profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let is_debug = build_profile == "debug";
        let arch = option_env!("CARGO_CFG_TARGET_ARCH").unwrap_or(std::env::consts::ARCH);
        let os_cfg = option_env!("CARGO_CFG_TARGET_OS").unwrap_or(std::env::consts::OS);
        let os = normalize_target_os_for_llvm_triple(os_cfg);
        let vendor = option_env!("CARGO_CFG_TARGET_VENDOR").unwrap_or(match os {
            "darwin" => "apple",
            "windows" => "pc",
            _ => "unknown",
        });
        let env = option_env!("CARGO_CFG_TARGET_ENV").unwrap_or("");
        let triple = if env.is_empty() {
            format!("{arch}-{vendor}-{os}")
        } else {
            format!("{arch}-{vendor}-{os}-{env}")
        };
        let pointer_width = usize::BITS;
        let endianness = if cfg!(target_endian = "little") {
            "little"
        } else {
            "big"
        };

        let mut expected = Vec::new();
        expected.push(format!("-DSCOOP_BUILD_PROFILE=\"{build_profile}\""));
        if is_debug {
            expected.push("-DSCOOP_DEBUG".to_string());
        }
        expected.push(format!("-DSCOOP_TARGET_TRIPLE=\"{triple}\""));
        expected.push(format!("-DSCOOP_TARGET_ARCH=\"{arch}\""));
        expected.push(format!("-DSCOOP_TARGET_OS=\"{os}\""));
        expected.push(format!("-DSCOOP_TARGET_POINTER_WIDTH={pointer_width}"));
        expected.push(format!("-DSCOOP_TARGET_ENDIANNESS=\"{endianness}\""));
        expected
    }

    #[test]
    fn compile_c_command_includes_build_profile_and_target_defines() {
        let dir = tempfile::tempdir().unwrap();
        let cone_root = dir.path();
        let source = cone_root.join("main.c");
        let output_obj = cone_root.join("main.o");
        std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();

        let cmd = compile_c_command_to_obj(cone_root, &source, &output_obj, &[]);
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        for expected in expected_profile_and_target_define_args() {
            assert!(args.iter().any(|a| a == &expected));
        }
    }

    #[test]
    fn compile_cxx_command_includes_build_profile_and_target_defines() {
        let dir = tempfile::tempdir().unwrap();
        let cone_root = dir.path();
        let source = cone_root.join("main.cpp");
        let output_obj = cone_root.join("main.o");
        std::fs::write(&source, "int main() { return 0; }\n").unwrap();

        let cmd = compile_cxx_command_to_obj(cone_root, &source, &output_obj, &[]);
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        for expected in expected_profile_and_target_define_args() {
            assert!(args.iter().any(|a| a == &expected));
        }
    }

    #[test]
    fn native_compile_commands_include_public_runtime_header_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cone_root = dir.path();
        let c_source = cone_root.join("main.c");
        let cxx_source = cone_root.join("main.cpp");
        let output_obj = cone_root.join("main.o");
        std::fs::write(&c_source, "int f(void) { return 0; }\n").unwrap();
        std::fs::write(&cxx_source, "int f() { return 0; }\n").unwrap();

        let include_dir = scoopld::runtime_public_include_dir()
            .to_string_lossy()
            .to_string();
        for cmd in [
            compile_c_command_to_obj(cone_root, &c_source, &output_obj, &[]),
            compile_cxx_command_to_obj(cone_root, &cxx_source, &output_obj, &[]),
        ] {
            let args = cmd
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            assert!(
                args.windows(2)
                    .any(|pair| pair[0] == "-I" && pair[1] == include_dir),
                "native compile command should include public runtime header dir {include_dir}, actual: {args:?}"
            );
        }
    }

    #[test]
    fn compile_c_source_missing_has_stable_error_code() {
        let dir = tempfile::tempdir().unwrap();
        let cone_root = dir.path();
        let missing = cone_root.join("missing.c");
        let out = cone_root.join("missing.o");

        let err = compile_c_source_to_obj(cone_root, &missing, &out, &[]).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::toolchain::c_source_missing"
        );
        assert!(err.to_string().contains("missing.c"));
    }

    #[test]
    fn compile_cxx_source_missing_has_stable_error_code() {
        let dir = tempfile::tempdir().unwrap();
        let cone_root = dir.path();
        let missing = cone_root.join("missing.cpp");
        let out = cone_root.join("missing.o");

        let err = compile_cxx_source_to_obj(cone_root, &missing, &out, &[]).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::toolchain::cxx_source_missing"
        );
        assert!(err.to_string().contains("missing.cpp"));
    }
}
