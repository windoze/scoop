//! Native link and runtime-C build support for Scoop cone builds.
//!
//! `scoopld` owns the host linker invocation plus runtime C object production.
//! It intentionally has no dependency on `scoop`, `scoopc`, stage crates, or the
//! LLVM backend; callers pass already-produced object files through this narrow
//! API or through `scoopc link-cone`.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use miette::Diagnostic;
use scoop_project_model::ConeKind;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

const LINK_INPUTS_FINGERPRINT_DOMAIN: &str = "scoop.link.inputs.v1";
const LINK_INPUTS_FINGERPRINT_FILE_NAME: &str = "inputs.fingerprint";
const RUNTIME_GC_BACKEND_ENV: &str = "SCOOP_RUNTIME_GC_BACKEND";

/// GC backend used when compiling the C runtime for linked user programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGcBackend {
    Baseline,
    Minimal,
    Immix,
    Hosted,
}

impl RuntimeGcBackend {
    fn c_define_value(self) -> &'static str {
        match self {
            RuntimeGcBackend::Baseline => "1",
            RuntimeGcBackend::Minimal => "2",
            RuntimeGcBackend::Immix => "3",
            RuntimeGcBackend::Hosted => "4",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "baseline" | "gc-baseline" | "1" => Some(RuntimeGcBackend::Baseline),
            "minimal" | "gc-minimal" | "2" => Some(RuntimeGcBackend::Minimal),
            "immix" | "gc-immix" | "3" => Some(RuntimeGcBackend::Immix),
            "hosted" | "gc-hosted" | "4" => Some(RuntimeGcBackend::Hosted),
            _ => None,
        }
    }
}

/// Linker invocation request for one consumer cone.
#[derive(Debug, Clone)]
pub struct LinkRequest {
    /// Consumer cone kind. Only [`ConeKind::Bin`] is executable today.
    pub kind: ConeKind,
    /// Object file produced for the consumer cone.
    pub consumer_obj: PathBuf,
    /// Object files produced by dependency cones or cone-local native build.
    pub dep_objs: Vec<PathBuf>,
    /// Directory where runtime C objects are materialized for this link.
    pub runtime_artifact_dir: PathBuf,
    /// Link-cache directory, e.g. `build/<profile>/link/<consumer>@<version>/`.
    pub output_dir: PathBuf,
    /// Final binary path visible to the user/driver.
    pub binary_path: PathBuf,
    /// Extra native libraries collected from `@Extern(lib = ...)` declarations.
    pub extern_libs: Vec<String>,
    /// Additional linker flags from loaded cone manifests.
    pub link_flags: Vec<String>,
    /// Optional linker driver (`clang`, `clang++`, or a configured path/name).
    pub linker: Option<PathBuf>,
    /// Driver-provided build input fingerprint, folded into the link digest.
    pub parent_inputs_fingerprint: Vec<u8>,
    /// Optional stable cone id, used only for diagnostics and cache binary naming.
    pub cone_id: Option<String>,
}

/// Result of linking one consumer cone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkResponse {
    pub binary_path: PathBuf,
    pub fingerprint_hex: String,
    pub cache_hit: bool,
}

/// Runtime C object build options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBuildOptions {
    /// Optional C compiler path/name; defaults to `clang`.
    pub compiler: Option<PathBuf>,
    /// Runtime GC backend. Defaults to Immix to preserve the current driver behavior.
    pub gc_backend: RuntimeGcBackend,
    /// Additional C flags used only for runtime compilation.
    pub c_flags: Vec<String>,
}

impl Default for RuntimeBuildOptions {
    fn default() -> Self {
        Self {
            compiler: None,
            gc_backend: RuntimeGcBackend::Immix,
            c_flags: Vec::new(),
        }
    }
}

/// Runtime C object build output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeArtifact {
    pub dir: PathBuf,
    pub object_files: Vec<PathBuf>,
    pub fingerprint_hex: String,
}

/// Link-stage errors.
#[derive(Debug, Error, Diagnostic)]
pub enum LinkError {
    #[error("link kind `{kind}` is not supported by scoopld yet")]
    #[diagnostic(code(scoopld::link_kind_not_supported))]
    KindNotSupported { kind: ConeKind },

    #[error(transparent)]
    #[diagnostic(transparent)]
    Runtime(#[from] RuntimeObjError),

    #[error("invalid {env} value `{value}`; expected one of baseline, immix, minimal, or hosted")]
    #[diagnostic(code(scoopld::invalid_runtime_gc_backend))]
    InvalidRuntimeGcBackend { env: &'static str, value: String },

    #[error("failed to access link path `{}`", path.display())]
    #[diagnostic(code(scoopld::io))]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "link cache fingerprint mismatch for `{}` (expected parent-provided value, computed different link inputs)",
        path.display()
    )]
    #[diagnostic(code(scoopld::fingerprint_mismatch))]
    FingerprintMismatch { path: PathBuf },

    #[error("linker `{linker}` was not found")]
    #[diagnostic(code(scoopld::linker_not_found))]
    LinkerNotFound { linker: String },

    #[error("failed to spawn linker `{linker}`: {source}")]
    #[diagnostic(code(scoopld::linker_spawn_failed))]
    LinkerSpawnFailed {
        linker: String,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "link failed (status: {status})\ncommand: {command}\nstdout: {stdout}\nstderr: {stderr}"
    )]
    #[diagnostic(code(scoopld::link_failed))]
    LinkFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },
}

/// Runtime C object compilation errors.
#[derive(Debug, Error, Diagnostic)]
pub enum RuntimeObjError {
    #[error("runtime C source is missing: {}", path.display())]
    #[diagnostic(code(scoopld::runtime_source_missing))]
    RuntimeSourceMissing { path: PathBuf },

    #[error(transparent)]
    #[diagnostic(transparent)]
    CompileC(#[from] CompileCError),

    #[error("failed to access runtime artifact path `{}`", path.display())]
    #[diagnostic(code(scoopld::runtime_io))]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// C compiler errors used while building runtime objects.
#[derive(Debug, Error, Diagnostic)]
pub enum CompileCError {
    #[error("C compiler `{compiler}` was not found")]
    #[diagnostic(code(scoopld::c_compiler_not_found))]
    CompilerNotFound { compiler: String },

    #[error("failed to read C source `{}`", path.display())]
    #[diagnostic(code(scoopld::c_source_unreadable))]
    SourceUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("C source is missing: {}", path.display())]
    #[diagnostic(code(scoopld::c_source_missing))]
    SourceMissing { path: PathBuf },

    #[error("failed to spawn C compiler `{compiler}`: {source}")]
    #[diagnostic(code(scoopld::c_compile_spawn_failed))]
    CompileSpawnFailed {
        compiler: String,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "C compilation failed (status: {status})\ncommand: {command}\nstdout: {stdout}\nstderr: {stderr}"
    )]
    #[diagnostic(code(scoopld::c_compile_failed))]
    CompileFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },
}

/// Link a binary cone and update/reuse its independent link cache.
pub fn link(req: LinkRequest) -> Result<LinkResponse, LinkError> {
    if req.kind != ConeKind::Bin {
        return Err(LinkError::KindNotSupported { kind: req.kind });
    }

    create_dir_all(&req.output_dir)?;
    if let Some(parent) = req.binary_path.parent() {
        create_dir_all(parent)?;
    }

    let runtime_opts = runtime_build_options_from_env()?;
    let runtime = compile_runtime_to_obj_dir(&req.runtime_artifact_dir, &runtime_opts)?;
    let fingerprint = compute_link_inputs_fingerprint(&req, &runtime)?;
    let fingerprint_path = req.output_dir.join(LINK_INPUTS_FINGERPRINT_FILE_NAME);
    let cache_binary = cache_binary_path(&req);

    if read_file_if_exists(&fingerprint_path)?.as_deref() == Some(fingerprint.as_slice())
        && cache_binary.is_file()
    {
        copy_cache_binary_to_final_output(&cache_binary, &req.binary_path)?;
        return Ok(LinkResponse {
            binary_path: req.binary_path,
            fingerprint_hex: hex_lower(&fingerprint),
            cache_hit: true,
        });
    }

    let mut objs = Vec::with_capacity(1 + req.dep_objs.len() + runtime.object_files.len());
    objs.push(req.consumer_obj.clone());
    objs.extend(req.dep_objs.iter().cloned());
    objs.extend(runtime.object_files.iter().cloned());

    link_objs(
        &objs,
        &cache_binary,
        &req.extern_libs,
        req.linker.as_deref(),
        &req.link_flags,
    )?;
    copy_cache_binary_to_final_output(&cache_binary, &req.binary_path)?;
    write_file(&fingerprint_path, &fingerprint)?;

    Ok(LinkResponse {
        binary_path: req.binary_path,
        fingerprint_hex: hex_lower(&fingerprint),
        cache_hit: false,
    })
}

fn runtime_build_options_from_env() -> Result<RuntimeBuildOptions, LinkError> {
    let mut opts = RuntimeBuildOptions::default();
    if let Ok(value) = std::env::var(RUNTIME_GC_BACKEND_ENV) {
        opts.gc_backend =
            RuntimeGcBackend::parse(&value).ok_or(LinkError::InvalidRuntimeGcBackend {
                env: RUNTIME_GC_BACKEND_ENV,
                value,
            })?;
    }
    Ok(opts)
}

/// Compile `runtime/c/*.c` into object files under `output_dir`.
pub fn compile_runtime_to_obj_dir(
    output_dir: &Path,
    opts: &RuntimeBuildOptions,
) -> Result<RuntimeArtifact, RuntimeObjError> {
    create_runtime_dir_all(output_dir)?;
    let sources = runtime_c_sources()?;
    let runtime_dir = runtime_c_dir();

    let mut object_files = Vec::with_capacity(sources.len());
    for (idx, src) in sources.iter().enumerate() {
        let stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("runtime");
        let obj_name = if cfg!(windows) {
            format!("rt_{stem}_{idx}.obj")
        } else {
            format!("rt_{stem}_{idx}.o")
        };

        let mut flags = vec![
            format!("-DSCOOP_GC_BACKEND={}", opts.gc_backend.c_define_value()),
            String::from("-DSCOOP_RUNTIME_NO_GC_TEST_HELPERS=1"),
        ];
        flags.extend(opts.c_flags.iter().cloned());
        let out_obj = output_dir.join(obj_name);
        compile_c_source_to_obj(
            &runtime_dir,
            src,
            &out_obj,
            opts.compiler.as_deref(),
            &flags,
        )?;
        object_files.push(out_obj);
    }

    let fingerprint_hex =
        fingerprint_files(&object_files).map_err(|source| RuntimeObjError::Io {
            path: output_dir.to_path_buf(),
            source,
        })?;

    Ok(RuntimeArtifact {
        dir: output_dir.to_path_buf(),
        object_files,
        fingerprint_hex,
    })
}

/// Development-tree location of `runtime/c`.
pub fn runtime_c_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/c")
}

/// Public include directory for runtime headers used by cone-local native code.
pub fn runtime_public_include_dir() -> PathBuf {
    runtime_c_dir().join("include")
}

fn compute_link_inputs_fingerprint(
    req: &LinkRequest,
    runtime: &RuntimeArtifact,
) -> Result<Vec<u8>, LinkError> {
    let mut hasher = Sha256::new();
    hasher.update(LINK_INPUTS_FINGERPRINT_DOMAIN.as_bytes());
    hasher.update(b"\nkind=");
    hasher.update(req.kind.as_str().as_bytes());
    hasher.update(b"\nparent=");
    hasher.update(&req.parent_inputs_fingerprint);
    hasher.update(b"\nconsumer=");
    hash_named_file(&mut hasher, &req.consumer_obj)?;

    let mut dep_objs = req.dep_objs.clone();
    dep_objs.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    for dep_obj in dep_objs {
        hasher.update(b"\ndep=");
        hash_named_file(&mut hasher, &dep_obj)?;
    }

    let mut runtime_objs = runtime.object_files.clone();
    runtime_objs.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    for runtime_obj in runtime_objs {
        hasher.update(b"\nruntime=");
        hash_named_file(&mut hasher, &runtime_obj)?;
    }

    hasher.update(b"\nruntime-artifact=");
    hasher.update(runtime.fingerprint_hex.as_bytes());
    hasher.update(b"\nlinker=");
    let linker = linker_name(req.linker.as_deref());
    hasher.update(linker.as_bytes());
    hasher.update(b"\nlinker-version=");
    hasher.update(linker_version_fingerprint(&linker).as_bytes());
    for lib in &req.extern_libs {
        hasher.update(b"\nlib=");
        hasher.update(lib.as_bytes());
    }
    for flag in &req.link_flags {
        hasher.update(b"\nflag=");
        hasher.update(flag.as_bytes());
    }
    for flag in implicit_link_flags() {
        hasher.update(b"\nimplicit-flag=");
        hasher.update(flag.as_bytes());
    }

    Ok(hasher.finalize().to_vec())
}

fn hash_named_file(hasher: &mut Sha256, path: &Path) -> Result<(), LinkError> {
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(b"=");
    let bytes = read_file(path)?;
    hasher.update(Sha256::digest(&bytes));
    Ok(())
}

fn link_objs(
    objs: &[PathBuf],
    output: &Path,
    libs: &[String],
    linker: Option<&Path>,
    link_flags: &[String],
) -> Result<(), LinkError> {
    let mut cmd = link_command(objs, output, libs, linker, link_flags);
    let linker_for_error = cmd.get_program().to_string_lossy().to_string();

    let output_res = cmd.output();
    let output_res = match output_res {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(LinkError::LinkerNotFound {
                linker: linker_for_error,
            });
        }
        Err(e) => {
            return Err(LinkError::LinkerSpawnFailed {
                linker: linker_for_error,
                source: e,
            });
        }
    };

    if !output_res.status.success() {
        return Err(LinkError::LinkFailed {
            status: output_res.status,
            command: format_command_for_debug(&cmd),
            stdout: String::from_utf8_lossy(&output_res.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output_res.stderr).to_string(),
        });
    }

    Ok(())
}

fn link_command(
    objs: &[PathBuf],
    output: &Path,
    libs: &[String],
    linker: Option<&Path>,
    link_flags: &[String],
) -> Command {
    let linker = linker_name(linker);
    let mut cmd = Command::new(&linker);
    for obj in objs {
        cmd.arg(obj);
    }
    for lib in libs {
        if lib.trim().is_empty() {
            continue;
        }
        cmd.arg(format!("-l{}", lib.trim()));
    }
    for flag in link_flags {
        if flag.trim().is_empty() {
            continue;
        }
        cmd.arg(flag);
    }
    for flag in implicit_link_flags() {
        cmd.arg(flag);
    }
    cmd.arg("-o").arg(output);
    cmd
}

fn implicit_link_flags() -> &'static [&'static str] {
    implicit_link_flags_for_target()
}

#[cfg(target_os = "linux")]
fn implicit_link_flags_for_target() -> &'static [&'static str] {
    &["-no-pie"]
}

#[cfg(not(target_os = "linux"))]
fn implicit_link_flags_for_target() -> &'static [&'static str] {
    &[]
}

fn compile_c_source_to_obj(
    cone_root: &Path,
    source: &Path,
    output_obj: &Path,
    compiler: Option<&Path>,
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

    let mut cmd = compile_c_command_to_obj(cone_root, source, output_obj, compiler, c_flags);
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
    compiler: Option<&Path>,
    c_flags: &[String],
) -> Command {
    let compiler = compiler
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clang".to_string());
    let mut cmd = Command::new(compiler);
    cmd.current_dir(cone_root);
    cmd.arg("-c");
    cmd.arg("-I").arg(runtime_public_include_dir());
    for flag in c_flags {
        if flag.trim().is_empty() {
            continue;
        }
        cmd.arg(flag);
    }
    push_build_profile_and_target_defines(&mut cmd);
    cmd.arg(source);
    cmd.arg("-o").arg(output_obj);
    cmd
}

fn runtime_c_sources() -> Result<Vec<PathBuf>, RuntimeObjError> {
    let dir = runtime_c_dir();
    let runtime_main = dir.join("scoop_runtime.c");
    if !runtime_main.is_file() {
        return Err(RuntimeObjError::RuntimeSourceMissing { path: runtime_main });
    }

    let mut extra = Vec::<PathBuf>::new();
    let entries = std::fs::read_dir(&dir).map_err(|source| RuntimeObjError::Io {
        path: dir.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RuntimeObjError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_file() || path == runtime_main {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("c") {
            extra.push(path);
        }
    }
    extra.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));

    let mut all = Vec::with_capacity(1 + extra.len());
    all.push(runtime_main);
    all.extend(extra);
    Ok(all)
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

fn linker_name(linker: Option<&Path>) -> String {
    linker
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clang".to_string())
}

fn linker_version_fingerprint(linker: &str) -> String {
    match Command::new(linker).arg("--version").output() {
        Ok(output) => {
            let mut hasher = Sha256::new();
            hasher.update(output.status.to_string().as_bytes());
            hasher.update(&output.stdout);
            hasher.update(&output.stderr);
            hex_lower(&hasher.finalize())
        }
        Err(err) => format!("unavailable:{err}"),
    }
}

fn cache_binary_path(req: &LinkRequest) -> PathBuf {
    let file_name = req
        .binary_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(ToOwned::to_owned)
        .or_else(|| req.cone_id.as_ref().map(|id| sanitize_file_name(id)))
        .unwrap_or_else(|| {
            if std::env::consts::EXE_EXTENSION.is_empty() {
                "a.out".to_string()
            } else {
                format!("a.{}", std::env::consts::EXE_EXTENSION)
            }
        });
    req.output_dir.join(file_name)
}

fn sanitize_file_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("a.out");
    }
    out
}

fn copy_cache_binary_to_final_output(
    cache_binary: &Path,
    final_output: &Path,
) -> Result<(), LinkError> {
    if cache_binary == final_output {
        return Ok(());
    }
    if let Some(parent) = final_output.parent() {
        create_dir_all(parent)?;
    }
    std::fs::copy(cache_binary, final_output).map_err(|source| LinkError::Io {
        path: final_output.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn create_dir_all(path: &Path) -> Result<(), LinkError> {
    std::fs::create_dir_all(path).map_err(|source| LinkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn create_runtime_dir_all(path: &Path) -> Result<(), RuntimeObjError> {
    std::fs::create_dir_all(path).map_err(|source| RuntimeObjError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_file(path: &Path) -> Result<Vec<u8>, LinkError> {
    std::fs::read(path).map_err(|source| LinkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_file_if_exists(path: &Path) -> Result<Option<Vec<u8>>, LinkError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LinkError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), LinkError> {
    std::fs::write(path, bytes).map_err(|source| LinkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn fingerprint_files(files: &[PathBuf]) -> std::io::Result<String> {
    let mut entries = files.to_vec();
    entries.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    let mut hasher = Sha256::new();
    for path in entries {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"\n");
        hasher.update(Sha256::digest(std::fs::read(path)?));
        hasher.update(b"\n");
    }
    Ok(hex_lower(&hasher.finalize()))
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

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_test_obj(dir: &Path, name: &str, ret: i32) -> PathBuf {
        let src = dir.join(format!("{name}.c"));
        let obj = dir.join(format!("{name}.o"));
        std::fs::write(&src, format!("int main(void) {{ return {ret}; }}\n")).unwrap();
        let status = Command::new("clang")
            .arg("-c")
            .arg(&src)
            .arg("-o")
            .arg(&obj)
            .status()
            .unwrap();
        assert!(status.success(), "clang -c should succeed");
        obj
    }

    fn request(root: &Path, obj: PathBuf) -> LinkRequest {
        LinkRequest {
            kind: ConeKind::Bin,
            consumer_obj: obj,
            dep_objs: Vec::new(),
            runtime_artifact_dir: root.join("runtime"),
            output_dir: root.join("link"),
            binary_path: root.join("bin").join("app"),
            extern_libs: Vec::new(),
            link_flags: Vec::new(),
            linker: None,
            parent_inputs_fingerprint: b"parent".to_vec(),
            cone_id: Some("app@0.0.0".to_string()),
        }
    }

    #[test]
    fn runtime_compile_commands_include_build_profile_and_target_defines() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("main.c");
        let output_obj = dir.path().join("main.o");
        std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();

        let cmd = compile_c_command_to_obj(dir.path(), &source, &output_obj, None, &[]);
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.iter().any(|a| a.starts_with("-DSCOOP_BUILD_PROFILE=")));
        assert!(args.iter().any(|a| a.starts_with("-DSCOOP_TARGET_TRIPLE=")));
        assert!(args.iter().any(|a| a.starts_with("-DSCOOP_TARGET_ARCH=")));
        assert!(args.iter().any(|a| a.starts_with("-DSCOOP_TARGET_OS=")));
        assert!(
            args.iter()
                .any(|a| a.starts_with("-DSCOOP_TARGET_POINTER_WIDTH="))
        );
        assert!(
            args.iter()
                .any(|a| a.starts_with("-DSCOOP_TARGET_ENDIANNESS="))
        );
    }

    #[test]
    fn runtime_compile_command_includes_public_include_dir() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("main.c");
        let output_obj = dir.path().join("main.o");
        std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();

        let cmd = compile_c_command_to_obj(dir.path(), &source, &output_obj, None, &[]);
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let include_dir = runtime_public_include_dir().to_string_lossy().to_string();

        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "-I" && pair[1] == include_dir),
            "runtime include dir missing from args: {args:?}"
        );
    }

    #[test]
    fn runtime_gc_backend_parser_accepts_fixture_matrix_names() {
        assert_eq!(
            RuntimeGcBackend::parse("baseline"),
            Some(RuntimeGcBackend::Baseline)
        );
        assert_eq!(
            RuntimeGcBackend::parse("gc-minimal"),
            Some(RuntimeGcBackend::Minimal)
        );
        assert_eq!(
            RuntimeGcBackend::parse("immix"),
            Some(RuntimeGcBackend::Immix)
        );
        assert_eq!(
            RuntimeGcBackend::parse("hosted"),
            Some(RuntimeGcBackend::Hosted)
        );
        assert_eq!(RuntimeGcBackend::parse("4"), Some(RuntimeGcBackend::Hosted));
        assert_eq!(RuntimeGcBackend::parse("unknown"), None);
    }

    #[test]
    fn bin_link_cache_hits_on_second_identical_request() {
        let dir = tempfile::tempdir().unwrap();
        let obj = compile_test_obj(dir.path(), "main", 0);
        let req = request(dir.path(), obj);

        let first = link(req.clone()).unwrap();
        let second = link(req).unwrap();

        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(first.fingerprint_hex, second.fingerprint_hex);
        assert!(second.binary_path.is_file());
    }

    #[test]
    fn bin_link_cache_misses_when_consumer_object_changes() {
        let dir = tempfile::tempdir().unwrap();
        let obj = compile_test_obj(dir.path(), "main", 0);
        let req = request(dir.path(), obj.clone());
        let first = link(req.clone()).unwrap();

        let changed = compile_test_obj(dir.path(), "main", 1);
        assert_eq!(obj, changed, "test rewrites the same object path");
        let second = link(req).unwrap();

        assert!(!first.cache_hit);
        assert!(!second.cache_hit);
        assert_ne!(first.fingerprint_hex, second.fingerprint_hex);
    }

    #[test]
    fn lib_and_syslib_return_explicit_kind_not_supported() {
        let dir = tempfile::tempdir().unwrap();
        let obj = dir.path().join("missing.o");

        for kind in [ConeKind::Lib, ConeKind::Syslib] {
            let mut req = request(dir.path(), obj.clone());
            req.kind = kind;
            let err = link(req).unwrap_err();
            assert!(matches!(err, LinkError::KindNotSupported { kind: found } if found == kind));
        }
    }

    #[test]
    fn link_command_preserves_libs_flags_and_output_order() {
        let dir = tempfile::tempdir().unwrap();
        let obj = dir.path().join("main.o");
        let out = dir.path().join("app");
        let libs = vec!["m".to_string()];
        let flags = vec![
            "-Wl,--gc-sections".to_string(),
            "-Wl,-dead_strip".to_string(),
        ];
        let cmd = link_command(&[obj], &out, &libs, Some(Path::new("my-linker")), &flags);
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(cmd.get_program().to_string_lossy(), "my-linker");
        let idx_lib = args.iter().position(|a| a == "-lm").unwrap();
        let idx_flag1 = args.iter().position(|a| a == "-Wl,--gc-sections").unwrap();
        let idx_flag2 = args.iter().position(|a| a == "-Wl,-dead_strip").unwrap();
        let idx_o = args.iter().position(|a| a == "-o").unwrap();
        assert!(idx_lib < idx_flag1 && idx_flag1 < idx_flag2 && idx_flag2 < idx_o);
    }
}
