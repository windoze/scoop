//! Compiler driver library: pipeline orchestration entry points.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8 and
//! `docs/milestone1/DESIGN.md` section 2.7.

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

/// Compile one source file through the full pipeline
/// (parse → HIR → MIR → LIR → object file → link with the C runtime).
///
/// All failures — unreadable file, stage diagnostics, codegen or linker
/// errors — are reported as diagnostics. Only driver-level failures lack a
/// span; stage diagnostics keep their original spans and are rendered by
/// the caller via `Diagnostic::render`.
pub fn compile_file(path: &Path, out_dir: &Path) -> Result<CompileSuccess, Vec<Diagnostic>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| vec![no_span(format!("cannot read {}: {e}", path.display()))])?;

    let ast = scoop_parser::parse(&source)?;
    let ast_dump = scoop_ast::dump(&ast);

    let hir = scoop_hir_lower::lower(&ast)?;
    let hir_dump = scoop_hir::dump(&hir);

    let mir = scoop_mir_lower::lower(&hir);
    let mir_dump = scoop_mir::dump(&mir);

    let lir = scoop_lir_lower::lower(&mir);
    let lir_dump = scoop_lir::dump(&lir);

    std::fs::create_dir_all(out_dir).map_err(|e| {
        vec![no_span(format!(
            "cannot create output directory {}: {e}",
            out_dir.display()
        ))]
    })?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("scoop-out");
    let object = out_dir.join(format!("{stem}.o"));
    let binary = out_dir.join(stem);

    scoop_codegen::emit_object(&lir, &object)
        .map_err(|e| vec![no_span(format!("codegen failed: {e}"))])?;

    let runtime_lib = build_runtime()?;
    link(&object, &runtime_lib, &binary)?;

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

/// A driver-level diagnostic without a source span.
fn no_span(message: String) -> Diagnostic {
    Diagnostic {
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

/// Compile the C runtime into a static library cached under
/// `target/scoop-rt/`. M1 has a single C file, so rebuilding every time is
/// cheap enough (see `docs/milestone1/DESIGN.md` section 2.7).
fn build_runtime() -> Result<PathBuf, Vec<Diagnostic>> {
    let root = workspace_root();
    let out_dir = root.join("target/scoop-rt");
    std::fs::create_dir_all(&out_dir).map_err(|e| {
        vec![no_span(format!(
            "cannot create runtime build directory {}: {e}",
            out_dir.display()
        ))]
    })?;
    let triple = host_triple();
    cc::Build::new()
        .file(root.join("runtime/src/rt.c"))
        .include(root.join("runtime/include"))
        .out_dir(&out_dir)
        // The driver is not a build script: cargo does not provide
        // TARGET/HOST here, so set them explicitly and silence cargo
        // metadata output.
        .target(&triple)
        .host(&triple)
        .cargo_metadata(false)
        // Outside a build script there is no OPT_LEVEL/DEBUG either.
        .opt_level(0)
        .debug(false)
        .try_compile("scoop_rt")
        .map_err(|e| vec![no_span(format!("failed to compile the C runtime: {e}"))])?;
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
fn link(object: &Path, runtime_lib: &Path, binary: &Path) -> Result<(), Vec<Diagnostic>> {
    let output = Command::new("cc")
        .arg(object)
        .arg(runtime_lib)
        .arg("-o")
        .arg(binary)
        .output()
        .map_err(|e| vec![no_span(format!("failed to run the linker `cc`: {e}"))])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(vec![no_span(format!(
            "linker failed (status {}): {}",
            output.status,
            stderr.trim()
        ))]);
    }
    Ok(())
}
