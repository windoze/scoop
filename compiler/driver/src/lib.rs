//! Compiler driver library: pipeline orchestration entry points.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8 and
//! `docs/milestone4/DESIGN.md` section 1 for the sysroot model: the
//! compilation unit is every `src/*.scoop` of the sysroot's `scoop.core`
//! Cone followed by the user file, all compiled together.

use scoop_ast::{Diagnostic, DiagnosticSeverity};
use std::path::{Path, PathBuf};

mod inputs;
mod linking;

pub use inputs::{load_inputs, render_diagnostics};
use linking::{LinkRequest, build_runtime, compile_c_bridge, link};

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
    /// Non-fatal source diagnostics. Their presence does not change success.
    pub warnings: Vec<Diagnostic>,
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
    /// and the user file relative to its Cone or compilation directory,
    /// so diagnostics and private initialization identities stay portable.
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
    let target_profile = scoop_codegen::TargetProfile::resolve_host()
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
            .zip(&inputs[..user_index])
            .map(|(source, input)| scoop_hir_lower::ProviderSource {
                source,
                provider: core_provider,
                name: &input.name,
                source_text: &input.source,
            })
            .collect(),
        user: scoop_hir_lower::ProviderSource {
            source: &files[user_index],
            provider: user_provider,
            name: &inputs[user_index].name,
            source_text: &inputs[user_index].source,
        },
    };
    let hir = scoop_hir_lower::lower_compilation_unit(
        &hir_unit,
        options.intrinsic_declaration_policy.clone(),
    )?;
    let hir_dump = scoop_hir::dump(&hir.export);
    let warnings = hir.warnings;

    let mir = scoop_mir_lower::lower(&hir.local);
    let mir_dump = scoop_mir::dump(&mir);

    let lir = scoop_lir_lower::lower(&mir, target_profile.lir_target_profile());
    let lir_dump = scoop_lir::dump(&lir);

    let build = (|| -> Result<(StageDumps, PathBuf), Vec<Diagnostic>> {
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

        Ok((
            StageDumps {
                ast: ast_dump,
                hir: hir_dump,
                mir: mir_dump,
                lir: lir_dump,
            },
            binary,
        ))
    })();
    match build {
        Ok((dumps, binary)) => Ok(CompileSuccess {
            dumps,
            binary,
            warnings,
        }),
        Err(mut diagnostics) => {
            diagnostics.extend(warnings);
            Err(diagnostics)
        }
    }
}

/// A driver-level diagnostic without a source span, attributed to the
/// input file at index `file`.
fn no_span(file: usize, message: String) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Error,
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
