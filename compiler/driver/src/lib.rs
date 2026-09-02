//! Compiler driver library: pipeline orchestration entry points.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8 and
//! `docs/milestone4/DESIGN.md` section 1 for the sysroot model: the
//! compilation unit is every `src/*.scoop` of the sysroot's `scoop.core`
//! Cone followed by the user file, all compiled together.

use std::path::{Path, PathBuf};
use std::process::Command;

use scoop_ast::Diagnostic;

/// Text dumps of every pipeline stage, for golden-dump testing and
/// `scoopc build --emit`.
pub struct StageDumps {
    pub ast: String,
    pub hir: String,
    pub mir: String,
    pub lir: String,
}

/// Result of a successful [`compile_file`] run.
pub struct CompileSuccess {
    pub dumps: StageDumps,
    /// Path to the linked executable.
    pub binary: PathBuf,
}

#[derive(Debug, Default)]
pub struct CompileOptions {
    pub library_paths: Vec<PathBuf>,
    /// Internal compiler/test authority switch. It is deliberately absent
    /// from the stable CLI, environment, manifest, and source language.
    pub intrinsic_declaration_policy: scoop_hir_lower::IntrinsicDeclarationPolicy,
}

/// One input file of the compilation unit: a display name (for
/// diagnostics) plus the source text.
pub struct SourceFileInput {
    /// Display name used when rendering diagnostics. Core library files
    /// are named relative to the sysroot (`scoop.core/src/option.scoop`)
    /// so rendered output stays portable; the user file keeps the path
    /// it was given as.
    pub name: String,
    pub source: String,
}

/// Compile one source file through the full pipeline: locate the sysroot,
/// load the `scoop.core` Cone sources, then parse → HIR → MIR → LIR →
/// object file → link with the C runtime.
///
/// All failures — malformed sysroot, unreadable file, stage diagnostics,
/// codegen or linker errors — are reported as diagnostics. Driver-level
/// failures lack a span; stage diagnostics keep their original spans and
/// carry the index of their input file (core files first, the user file
/// last). Render them via [`render_diagnostics`] with [`load_inputs`].
pub fn compile_file(path: &Path, out_dir: &Path) -> Result<CompileSuccess, Vec<Diagnostic>> {
    compile_file_with_options(path, out_dir, &CompileOptions::default())
}

pub fn compile_file_with_options(
    path: &Path,
    out_dir: &Path,
    options: &CompileOptions,
) -> Result<CompileSuccess, Vec<Diagnostic>> {
    let target_profile = scoop_codegen::TargetProfile::resolve(&host_triple())
        .map_err(|error| vec![no_span(0, format!("target configuration failed: {error}"))])?;
    let inputs = load_inputs(path)?;
    let user_index = inputs.len() - 1;

    // The parser sees one file at a time and reports file 0; rewrite to
    // the file's index in the compilation unit.
    let mut files = Vec::with_capacity(inputs.len());
    let mut diagnostics = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        match scoop_parser::parse(&input.source) {
            Ok(file) => files.push(file),
            Err(mut parse_diagnostics) => {
                for diagnostic in &mut parse_diagnostics {
                    diagnostic.file = index;
                }
                diagnostics.extend(parse_diagnostics);
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let ast_dump = files.iter().map(scoop_ast::dump).collect::<String>();

    // HIR lowering consumes the whole compilation unit at once; its
    // diagnostics already carry the file index into `files`.
    let core_provider = scoop_hir::IntrinsicProviderId::from_raw(0);
    let user_provider = scoop_hir::IntrinsicProviderId::from_raw(1);
    let hir_unit = scoop_hir_lower::CompilationUnit {
        core: files[..user_index]
            .iter()
            .map(|source| scoop_hir_lower::ProviderSource {
                source,
                provider: core_provider,
            })
            .collect(),
        user: scoop_hir_lower::ProviderSource {
            source: &files[user_index],
            provider: user_provider,
        },
    };
    let hir = scoop_hir_lower::lower_compilation_unit(
        &hir_unit,
        options.intrinsic_declaration_policy.clone(),
    )?;
    let hir_dump = scoop_hir::dump(&hir.export);

    let mir = scoop_mir_lower::lower(&hir.local);
    let mir_dump = scoop_mir::dump(&mir);

    let lir = scoop_lir_lower::lower(&mir);
    let lir_dump = scoop_lir::dump(&lir);

    std::fs::create_dir_all(out_dir).map_err(|e| {
        vec![no_span(
            user_index,
            format!("cannot create output directory {}: {e}", out_dir.display()),
        )]
    })?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("scoop-out");
    let object = out_dir.join(format!("{stem}.o"));
    let binary = out_dir.join(stem);

    scoop_codegen::emit_object(&lir, &object, target_profile)
        .map_err(|e| vec![no_span(user_index, format!("codegen failed: {e}"))])?;

    let bridge_object = match scoop_codegen::c_bridge_source(&lir).map_err(|e| {
        vec![no_span(
            user_index,
            format!("C bridge generation failed: {e}"),
        )]
    })? {
        Some(source) => {
            let source_path = out_dir.join(format!("{stem}.ffi.c"));
            let object_path = out_dir.join(format!("{stem}.ffi.o"));
            std::fs::write(&source_path, source).map_err(|error| {
                vec![no_span(
                    user_index,
                    format!("cannot write C bridge {}: {error}", source_path.display()),
                )]
            })?;
            compile_c_bridge(&source_path, &object_path, target_profile, user_index)?;
            Some(object_path)
        }
        None => None,
    };

    let runtime_lib = build_runtime(user_index, target_profile)?;
    let mut libraries = Vec::new();
    for (_, extern_) in lir.extern_functions.iter() {
        if !extern_.library.is_empty() && !libraries.contains(&extern_.library) {
            libraries.push(extern_.library.clone());
        }
    }
    for (_, global) in lir.native_globals.iter() {
        if !global.library.is_empty() && !libraries.contains(&global.library) {
            libraries.push(global.library.clone());
        }
    }
    link(LinkRequest {
        object: &object,
        bridge_object: bridge_object.as_deref(),
        runtime_lib: &runtime_lib,
        libraries: &libraries,
        library_paths: &options.library_paths,
        binary: &binary,
        target_profile,
        file: user_index,
    })?;

    Ok(CompileSuccess {
        dumps: StageDumps {
            ast: ast_dump,
            hir: hir_dump,
            mir: mir_dump,
            lir: lir_dump,
        },
        binary,
    })
}

fn compile_c_bridge(
    source: &Path,
    object: &Path,
    target_profile: scoop_codegen::TargetProfile,
    file: usize,
) -> Result<(), Vec<Diagnostic>> {
    let output = Command::new("cc")
        .arg("-target")
        .arg(target_profile.canonical_triple())
        .arg("-std=c11")
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(object)
        .output()
        .map_err(|error| {
            vec![no_span(
                file,
                format!("failed to run C bridge compiler `cc`: {error}"),
            )]
        })?;
    if !output.status.success() {
        return Err(vec![no_span(
            file,
            format!(
                "C bridge compilation failed (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        )]);
    }
    Ok(())
}

/// Load the compilation unit: every `src/*.scoop` of the sysroot's
/// `scoop.core` Cone (sorted by file name) followed by the user file.
pub fn load_inputs(user_path: &Path) -> Result<Vec<SourceFileInput>, Vec<Diagnostic>> {
    let mut inputs = load_core_sources()?;
    let user_index = inputs.len();
    let source = std::fs::read_to_string(user_path).map_err(|e| {
        vec![no_span(
            user_index,
            format!("cannot read {}: {e}", user_path.display()),
        )]
    })?;
    inputs.push(SourceFileInput {
        name: user_path.display().to_string(),
        source,
    });
    Ok(inputs)
}

/// Render diagnostics against the loaded inputs, selecting each
/// diagnostic's `(name, source)` by its file index. Driver-level
/// diagnostics emitted before loading finished have no corresponding
/// input; they fall back to the user file's name and source.
pub fn render_diagnostics(
    diagnostics: &[Diagnostic],
    inputs: &[SourceFileInput],
    fallback_name: &str,
    fallback_source: &str,
) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| match inputs.get(diagnostic.file) {
            Some(input) => diagnostic.render(&input.name, &input.source),
            None => diagnostic.render(fallback_name, fallback_source),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A driver-level diagnostic without a source span, attributed to the
/// input file at index `file`.
fn no_span(file: usize, message: String) -> Diagnostic {
    Diagnostic {
        file,
        span: None,
        message,
    }
}

/// Root of the Cargo workspace (the driver crate lives in `compiler/driver`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}

/// Locate the sysroot: `SCOOP_SYSROOT` if set, otherwise the workspace's
/// `sysroot/` directory.
fn sysroot() -> PathBuf {
    match std::env::var_os("SCOOP_SYSROOT") {
        Some(dir) => PathBuf::from(dir),
        None => workspace_root().join("sysroot"),
    }
}

/// Read the `scoop.core` Cone of the located sysroot: validate its
/// `Cone.toml`, then load every `src/*.scoop` sorted by file name.
fn load_core_sources() -> Result<Vec<SourceFileInput>, Vec<Diagnostic>> {
    let core_dir = sysroot().join("lib/scoop.core");
    let manifest_path = core_dir.join("Cone.toml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| {
        vec![no_span(
            0,
            format!("cannot read {}: {e}", manifest_path.display()),
        )]
    })?;
    validate_core_manifest(&manifest, &manifest_path)?;

    let src_dir = core_dir.join("src");
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&src_dir).map_err(|e| {
        vec![no_span(
            0,
            format!("cannot list {}: {e}", src_dir.display()),
        )]
    })? {
        let path = entry
            .map_err(|e| {
                vec![no_span(
                    0,
                    format!("cannot read {}: {e}", src_dir.display()),
                )]
            })?
            .path();
        if path.extension().is_some_and(|ext| ext == "scoop") {
            entries.push(path);
        }
    }
    // All entries share `src_dir`, so path order is file-name order.
    entries.sort();

    let mut inputs = Vec::with_capacity(entries.len());
    for (index, path) in entries.iter().enumerate() {
        let source = std::fs::read_to_string(path).map_err(|e| {
            vec![no_span(
                index,
                format!("cannot read {}: {e}", path.display()),
            )]
        })?;
        let file_name = path
            .file_name()
            .expect("dir entry has a file name")
            .to_string_lossy();
        inputs.push(SourceFileInput {
            name: format!("scoop.core/src/{file_name}"),
            source,
        });
    }
    Ok(inputs)
}

/// Validate the M4 slice of a `Cone.toml`: a `[cone]` table with string
/// `group` / `name` / `version`, and `name` matching the Cone directory.
fn validate_core_manifest(manifest: &str, path: &Path) -> Result<(), Vec<Diagnostic>> {
    let invalid =
        |detail: String| vec![no_span(0, format!("invalid {}: {detail}", path.display()))];
    let table = manifest
        .parse::<toml::Table>()
        .map_err(|e| invalid(format!("not valid TOML: {e}")))?;
    let Some(cone) = table.get("cone").and_then(toml::Value::as_table) else {
        return Err(invalid("missing [cone] section".into()));
    };
    for field in ["group", "name", "version"] {
        if cone.get(field).and_then(toml::Value::as_str).is_none() {
            return Err(invalid(format!("missing [cone].{field}")));
        }
    }
    let name = cone["name"].as_str().expect("checked above");
    if name != "scoop.core" {
        return Err(invalid(format!(
            "[cone].name is {name:?}, expected \"scoop.core\""
        )));
    }
    Ok(())
}

/// Compile the C runtime into a static library cached under
/// `target/scoop-rt/`. The complete source set comes from the selected target
/// profile; rebuilding it is still cheap enough (see
/// `docs/milestone1/DESIGN.md` section 2.7).
fn build_runtime(
    file: usize,
    target_profile: scoop_codegen::TargetProfile,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let root = workspace_root();
    let out_dir = root.join("target/scoop-rt");
    std::fs::create_dir_all(&out_dir).map_err(|e| {
        vec![no_span(
            file,
            format!(
                "cannot create runtime build directory {}: {e}",
                out_dir.display()
            ),
        )]
    })?;
    let mut build = cc::Build::new();
    for source in target_profile.runtime_sources() {
        build.file(root.join(source));
    }
    build.include(root.join("runtime/include"));
    for flag in target_profile.runtime_c_flags() {
        build.flag(flag);
    }
    build
        .out_dir(&out_dir)
        // The driver is not a build script: cargo does not provide
        // TARGET/HOST here, so set them explicitly and silence cargo
        // metadata output.
        .target(target_profile.canonical_triple())
        .host(target_profile.canonical_triple())
        .cargo_metadata(false)
        // Outside a build script there is no OPT_LEVEL/DEBUG either.
        .opt_level(0)
        .debug(false)
        .try_compile("scoop_rt")
        .map_err(|e| {
            vec![no_span(
                file,
                format!("failed to compile the C runtime: {e}"),
            )]
        })?;
    Ok(out_dir.join("libscoop_rt.a"))
}

/// Host target triple, derived from the platform the driver runs on.
/// Cross-compilation is out of scope for M1.
fn host_triple() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        other => format!("{arch}-unknown-{other}"),
    }
}

/// Link the object file and the runtime static library into an executable
/// using the system `cc` driver.
struct LinkRequest<'a> {
    object: &'a Path,
    bridge_object: Option<&'a Path>,
    runtime_lib: &'a Path,
    libraries: &'a [String],
    library_paths: &'a [PathBuf],
    binary: &'a Path,
    target_profile: scoop_codegen::TargetProfile,
    file: usize,
}

fn link(request: LinkRequest<'_>) -> Result<(), Vec<Diagnostic>> {
    let LinkRequest {
        object,
        bridge_object,
        runtime_lib,
        libraries,
        library_paths,
        binary,
        target_profile,
        file,
    } = request;
    let mut command = Command::new("cc");
    command
        .arg("-target")
        .arg(target_profile.canonical_triple());
    command.arg(object);
    if let Some(bridge_object) = bridge_object {
        command.arg(bridge_object);
    }
    command.arg(runtime_lib);
    for path in library_paths {
        command.arg("-L").arg(path);
    }
    for library in libraries {
        command.arg(format!("-l{library}"));
    }
    command.args(target_profile.linker_args());
    let output = command
        .arg("-o")
        .arg(binary)
        .output()
        .map_err(|e| vec![no_span(file, format!("failed to run the linker `cc`: {e}"))])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(vec![no_span(
            file,
            format!(
                "linker failed (status {}): {}",
                output.status,
                stderr.trim()
            ),
        )]);
    }
    Ok(())
}
